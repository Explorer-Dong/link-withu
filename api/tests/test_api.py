import copy
import csv
import io
from datetime import datetime, time, timedelta, timezone
from pathlib import Path
from zoneinfo import ZoneInfo

import pytest
from sqlalchemy import insert

from app.database import engine
from app.models import AppSetting, Visit
from app.routers.analytics import calendar
from app.schemas.link import SiteSettings


@pytest.mark.parametrize(
    "path",
    [
        "/api/links",
        "/api/links/tags",
        "/api/analytics",
        "/api/admin/settings",
        "/api/admin/backup",
        "/api/analytics/export",
    ],
)
def test_management_requires_auth(client, path):
    assert client.get(path).status_code == 401


def test_auth_rejects_wrong_password_and_bad_tokens(client):
    assert (
        client.post("/api/admin/login", json={"password": "wrong"}).status_code == 401
    )
    for token in ["invalid", "a.b", "abc.😀", "a" * 100 + ".x"]:
        response = client.get(
            "/api/links",
            headers={
                "Authorization": "Bearer " + token.encode("ascii", "ignore").decode()
            },
        )
        assert response.status_code == 401


def test_crud_search_tags_sort_and_public_redirect(client, auth, make_link):
    one = make_link(
        "one", title="博客文章", tags=["博客", "技术", "博客"], note="测试备注"
    )
    two = make_link("two", title="活动")
    assert one["tags"] == ["博客", "技术"]
    assert client.get("/r/one", follow_redirects=False).status_code == 302
    for q in ["博客文章", "备注", "技术", "/path", "one"]:
        result = client.get("/api/links", headers=auth, params={"q": q}).json()
        assert one["id"] in [item["id"] for item in result["items"]]
    assert (
        client.get("/api/links", headers=auth, params={"q": "%"}).json()["total"] == 0
    )
    tagged = client.get("/api/links", headers=auth, params={"tag": "博客"}).json()
    assert [item["short_code"] for item in tagged["items"]] == ["one"]
    ordered = client.get(
        "/api/links", headers=auth, params={"sort": "visits", "limit": 1}
    ).json()
    assert ordered["total"] == 2 and ordered["items"][0]["short_code"] == "one"
    changed = client.put(
        "/api/links/one",
        headers=auth,
        json={"original_url": "new.example.org", "title": "更新", "tags": ["新标签"]},
    )
    assert changed.status_code == 200, changed.text
    assert changed.json()["short_code"] == "one"
    assert changed.json()["tags"] == ["新标签"]
    assert (
        client.get("/r/one", follow_redirects=False).headers["location"]
        == "https://new.example.org"
    )
    assert client.delete("/api/links/one", headers=auth).status_code == 204
    assert client.get("/r/one", follow_redirects=False).status_code == 404
    backup = client.get("/api/admin/backup", headers=auth).json()
    assert backup["visits"] == []
    assert backup["links"][0]["id"] == two["id"]


def test_invalid_url_and_duplicate_code(client, auth, make_link):
    make_link()
    assert (
        client.post(
            "/api/links",
            headers=auth,
            json={"original_url": "https://example.org", "short_code": "example"},
        ).status_code
        == 409
    )
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://",
        "https://user:pass@example.org",
        "https://example.org:invalid",
        "https://example.org/\nLocation: evil",
    ]:
        assert (
            client.post(
                "/api/links", headers=auth, json={"original_url": url}
            ).status_code
            == 422
        )


def test_sources_devices_bots_and_analytics_consistency(client, auth, make_link):
    make_link(tags=["博客", "推广"])
    agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1"
    client.get(
        "/r/example",
        headers={
            "user-agent": agent,
            "referer": "https://blog.example.com/private?secret=hidden",
        },
        follow_redirects=False,
    )
    client.get(
        "/r/example", headers={"user-agent": "Googlebot/2.1"}, follow_redirects=False
    )
    result = client.get("/api/analytics", headers=auth, params={"days": 7}).json()
    assert result["summary"]["total_visits"] == 2
    assert result["summary"]["today"] == 2
    assert sum(p["count"] for p in result["trend"]) == 2
    assert result["summary"]["suspected_bots"] == 1
    assert {"name": "mobile", "count": 1} in result["devices"]
    assert {"name": "Safari", "count": 1} in result["browsers"]
    assert {"name": "blog.example.com", "count": 1} in result["referrers"]
    assert all(t["count"] == 2 for t in result["tags"])
    clean = client.get(
        "/api/analytics", headers=auth, params={"exclude_bots": True}
    ).json()
    assert clean["summary"]["suspected_bots"] == 1
    assert clean["summary"]["total_visits"] == clean["summary"]["period_visits"] == 1
    assert clean["top_links"][0]["count"] == 1
    assert all(t["count"] == 1 for t in clean["tags"])
    tagged = client.get(
        "/api/analytics", headers=auth, params={"tag": "博客", "exclude_bots": True}
    ).json()
    assert tagged["summary"]["period_visits"] == 1
    assert {row["name"] for row in tagged["tags"]} == {"博客", "推广"}
    empty_tag = client.get(
        "/api/analytics", headers=auth, params={"tag": "不存在"}
    ).json()
    assert empty_tag["summary"]["links"] == 0 and empty_tag["tags"] == []
    stats = client.get(
        "/api/links/stats/example", headers=auth, params={"exclude_bots": True}
    ).json()
    assert stats["total_visits"] == 1 and stats["visits"][0]["device"] == "mobile"
    assert stats["visits"][0]["referrer_domain"] == "blog.example.com"
    csv_text = client.get(
        "/api/analytics/export", headers=auth, params={"exclude_bots": True, "days": 7}
    ).text
    assert len(list(csv.reader(io.StringIO(csv_text)))) == 2
    assert "secret=hidden" not in csv_text


def test_aggregates_use_all_history_not_recent_page(client, auth, make_link):
    link = make_link()
    now = datetime.now(timezone.utc).replace(tzinfo=None)

    async def seed():
        async with engine.begin() as conn:
            await conn.execute(
                insert(Visit),
                [
                    {
                        "short_link_id": link["id"],
                        "visited_at": now - timedelta(seconds=i),
                        "device": "desktop",
                        "browser": "Chrome",
                        "is_bot": 0,
                    }
                    for i in range(650)
                ],
            )

    client.portal.call(seed)
    stats = client.get("/api/links/stats/example", headers=auth).json()
    assert stats["total_visits"] == 650 and len(stats["visits"]) == 50
    second = client.get(
        "/api/links/stats/example", headers=auth, params={"skip": 600, "limit": 100}
    ).json()
    assert len(second["visits"]) == 50
    result = client.get("/api/analytics", headers=auth).json()
    assert result["summary"]["total_visits"] == 650
    assert sum(p["count"] for p in result["trend"]) == 650
    assert result["top_links"][0]["count"] == 650


def test_timezone_boundaries_and_dst(client, auth, make_link, monkeypatch):
    from app.config import settings

    monkeypatch.setattr(settings, "tz", "America/New_York")
    link = make_link()
    config = {
        "app_name": "My links",
        "base_url": "https://go.example.com",
        "timezone": "America/New_York",
        "random_code_length": 8,
    }
    assert (
        client.put("/api/admin/settings", headers=auth, json=config).status_code == 200
    )
    now = datetime.now(timezone.utc)
    local_today = now.astimezone(ZoneInfo(config["timezone"])).date()
    beginning = (
        datetime.combine(local_today, time.min, ZoneInfo(config["timezone"]))
        .astimezone(timezone.utc)
        .replace(tzinfo=None)
    )

    async def seed():
        async with engine.begin() as conn:
            await conn.execute(
                insert(Visit),
                [
                    {
                        "short_link_id": link["id"],
                        "visited_at": beginning - timedelta(seconds=1),
                    },
                    {"short_link_id": link["id"], "visited_at": beginning},
                ],
            )

    client.portal.call(seed)
    stats = client.get("/api/analytics", headers=auth, params={"days": 7}).json()
    assert stats["summary"]["today"] == 1
    assert stats["summary"]["last_7_days"] == 2
    assert stats["trend"][-1]["count"] == 1 and stats["trend"][-2]["count"] == 1
    _, _, _, midnight = calendar(SiteSettings(**config), 7)
    from datetime import date

    assert midnight(date(2026, 3, 9)) - midnight(date(2026, 3, 8)) == timedelta(
        hours=23
    )
    assert midnight(date(2026, 11, 2)) - midnight(date(2026, 11, 1)) == timedelta(
        hours=25
    )


def test_settings_persist_and_apply_to_new_links(client, auth):
    settings = {
        "app_name": "私人短链",
        "base_url": "https://go.example.com/",
        "timezone": "UTC",
        "random_code_length": 9,
    }
    assert (
        client.put("/api/admin/settings", headers=auth, json=settings).status_code
        == 200
    )
    assert client.get("/api/site").json()["app_name"] == "私人短链"
    result = client.post(
        "/api/links", headers=auth, json={"original_url": "example.org"}
    ).json()
    assert len(result["short_code"]) == 9
    assert result["short_url"].startswith("https://go.example.com/r/")
    for patch in [
        {"timezone": "Invalid/Time"},
        {"random_code_length": 1},
        {"base_url": "https://example.org/path"},
        {"app_name": "  "},
    ]:
        assert (
            client.put(
                "/api/admin/settings", headers=auth, json={**settings, **patch}
            ).status_code
            == 422
        )
    assert (
        client.get("/api/admin/settings", headers=auth).json()["site"][
            "random_code_length"
        ]
        == 9
    )


def test_deployment_timezone_overrides_saved_settings_and_restored_backup(
    client, auth, monkeypatch
):
    from app.config import settings

    monkeypatch.setattr(settings, "tz", "Asia/Singapore")

    async def seed_old_timezone():
        async with engine.begin() as conn:
            await conn.execute(
                insert(AppSetting),
                [{"key": "timezone", "value": "America/New_York"}],
            )

    client.portal.call(seed_old_timezone)
    assert (
        client.get("/api/admin/settings", headers=auth).json()["site"]["timezone"]
        == "Asia/Singapore"
    )
    updated = client.put("/api/admin/settings", headers=auth, json={"timezone": "UTC"})
    assert updated.status_code == 200
    assert updated.json()["timezone"] == "Asia/Singapore"
    snapshot = client.get("/api/admin/backup", headers=auth).json()
    snapshot["settings"]["timezone"] = "UTC"
    restored = client.post(
        "/api/admin/restore",
        headers={**auth, "X-Confirm-Restore": "RESTORE"},
        json=snapshot,
    )
    assert restored.status_code == 200
    assert (
        client.get("/api/analytics", headers=auth).json()["timezone"]
        == "Asia/Singapore"
    )
    assert (
        client.get("/api/admin/backup", headers=auth).json()["settings"]["timezone"]
        == "Asia/Singapore"
    )


def test_timezone_environment_validation(monkeypatch):
    from pydantic import ValidationError

    from app.config import Settings

    monkeypatch.setenv("TZ", "America/New_York")
    assert Settings(_env_file=None).tz == "America/New_York"
    monkeypatch.setenv("TZ", "Invalid/Time")
    with pytest.raises(ValidationError, match="TZ 必须是有效时区"):
        Settings(_env_file=None)


def test_backup_restore_roundtrip_and_invalid_restore_is_atomic(
    client, auth, make_link
):
    make_link(title="备份测试", tags=["备份"], note="保留备注")
    client.get("/r/example", follow_redirects=False)
    snapshot = client.get("/api/admin/backup", headers=auth)
    assert snapshot.status_code == 200
    assert "attachment" in snapshot.headers["content-disposition"]
    original = snapshot.json()
    assert "admin_password" not in snapshot.text
    make_link("new")
    for mutation in [
        "duplicate_link",
        "orphan_visit",
        "bad_url",
        "duplicate_visit",
        "bad_settings",
    ]:
        data = copy.deepcopy(original)
        if mutation == "duplicate_link":
            data["links"].append(data["links"][0])
        elif mutation == "orphan_visit":
            data["visits"][0]["short_link_id"] = 9999
        elif mutation == "bad_url":
            data["links"][0]["original_url"] = "file:///etc/passwd"
        elif mutation == "duplicate_visit":
            data["visits"].append(data["visits"][0])
        else:
            data["settings"]["timezone"] = "bad"
        res = client.post(
            "/api/admin/restore",
            headers={**auth, "X-Confirm-Restore": "RESTORE"},
            json=data,
        )
        assert res.status_code == 400, res.text
        assert client.get("/api/links", headers=auth).json()["total"] == 2
    assert (
        client.post("/api/admin/restore", headers=auth, json=original).status_code
        == 400
    )
    result = client.post(
        "/api/admin/restore",
        headers={**auth, "X-Confirm-Restore": "RESTORE"},
        json=original,
    )
    assert result.status_code == 200, result.text
    restored = client.get("/api/admin/backup", headers=auth).json()
    assert restored["links"] == original["links"]
    assert restored["visits"] == original["visits"]
    assert restored["settings"] == original["settings"]
    assert client.get("/r/example", follow_redirects=False).status_code == 302


def test_csv_export_protects_spreadsheet_formulas(client, auth, make_link):
    make_link(
        title='=HYPERLINK("https://evil.org")', note="  +SUM(1,2)", tags=["@attack"]
    )
    res = client.get("/api/analytics/export?kind=links", headers=auth)
    assert res.status_code == 200
    rows = list(csv.reader(io.StringIO(res.text.lstrip("\ufeff"))))
    assert rows[1][2].startswith("'=")
    assert rows[1][3].startswith("'+")
    assert rows[1][4].startswith("'@")


def test_restore_rolls_back_if_database_write_fails(
    client, auth, make_link, monkeypatch
):
    make_link(tags=["keep"])
    snapshot = client.get("/api/admin/backup", headers=auth).json()
    make_link("also-keep")
    import app.routers.settings as settings_router

    async def fail(*args):
        raise RuntimeError("simulated write failure")

    monkeypatch.setattr(settings_router, "save_site_settings", fail)
    with pytest.raises(RuntimeError, match="simulated write failure"):
        client.post(
            "/api/admin/restore",
            headers={**auth, "X-Confirm-Restore": "RESTORE"},
            json=snapshot,
        )
    assert client.get("/api/links", headers=auth).json()["total"] == 2
    assert client.get("/api/links/tags", headers=auth).json() == [
        {"name": "keep", "link_count": 1}
    ]


def test_existing_tags_can_be_edited_without_duplicate_rows(client, auth, make_link):
    make_link(tags=["keep", "remove"])
    for _ in range(2):
        res = client.put(
            "/api/links/example",
            headers=auth,
            json={"original_url": "https://example.org", "tags": ["keep", "new"]},
        )
        assert res.status_code == 200, res.text
        assert res.json()["tags"] == ["keep", "new"]


def test_browser_routes_refresh_and_unknown_api_returns_404(client):
    if not (Path(__file__).resolve().parents[2] / "web/dist/index.html").is_file():
        pytest.skip("build frontend before running SPA route checks")
    for path in ["/", "/login", "/links", "/analytics", "/settings"]:
        response = client.get(path)
        assert response.status_code == 200
        assert '<div id="app">' in response.text
    assert client.get("/api/not-a-route").status_code == 404
    assert client.get("/does-not-exist").status_code == 404


def test_all_time_includes_old_visits_in_charts_rankings_tags_and_export(
    client, auth, make_link
):
    link = make_link(tags=["历史"])
    now = datetime.now(timezone.utc).replace(tzinfo=None)

    async def seed():
        async with engine.begin() as conn:
            await conn.execute(
                insert(Visit),
                [
                    {"short_link_id": link["id"], "visited_at": now, "is_bot": 0},
                    {
                        "short_link_id": link["id"],
                        "visited_at": now - timedelta(days=180),
                        "is_bot": 0,
                    },
                    {
                        "short_link_id": link["id"],
                        "visited_at": now - timedelta(days=4000),
                        "is_bot": 0,
                    },
                    {
                        "short_link_id": link["id"],
                        "visited_at": now - timedelta(days=5000),
                        "is_bot": 1,
                    },
                    {
                        "short_link_id": link["id"],
                        "visited_at": now + timedelta(days=5),
                        "is_bot": 0,
                    },
                ],
            )

    client.portal.call(seed)
    response = client.get("/api/analytics", headers=auth)
    assert response.status_code == 200, response.text
    data = response.json()
    assert data["days"] == 0
    assert data["summary"]["period_visits"] == data["summary"]["total_visits"] == 4
    assert sum(point["count"] for point in data["trend"]) == 4
    assert data["top_links"][0]["count"] == data["tags"][0]["count"] == 4
    assert len(data["trend"]) <= 90 and data["bucket_days"] > 1
    assert data["trend"][0]["date"] == data["range_start"]
    assert data["trend"][-1]["end_date"] == data["range_end"]
    assert data["range_start"] < (now - timedelta(days=4000)).date().isoformat()
    filtered = client.get(
        "/api/analytics",
        headers=auth,
        params={"days": 0, "tag": "历史", "exclude_bots": True},
    ).json()
    assert filtered["summary"]["period_visits"] == 3
    assert sum(point["count"] for point in filtered["trend"]) == 3
    assert filtered["summary"]["suspected_bots"] == 1
    recent = client.get("/api/analytics", headers=auth, params={"days": 90}).json()
    assert recent["summary"]["period_visits"] == 1
    assert sum(point["count"] for point in recent["trend"]) == 1
    export = client.get(
        "/api/analytics/export",
        headers=auth,
        params={"days": 0, "tag": "历史", "exclude_bots": True},
    )
    assert export.status_code == 200
    assert len(list(csv.reader(io.StringIO(export.text)))) == 4
    recent_export = client.get(
        "/api/analytics/export", headers=auth, params={"days": 90}
    ).text
    assert len(list(csv.reader(io.StringIO(recent_export)))) == 2


def test_all_time_handles_empty_history(client, auth):
    data = client.get("/api/analytics", headers=auth, params={"days": 0}).json()
    assert data["summary"]["period_visits"] == 0
    assert len(data["trend"]) == 1 and data["trend"][0]["count"] == 0
    assert data["range_start"] == data["range_end"]
