import copy
from concurrent.futures import ThreadPoolExecutor

import pytest
from fastapi.testclient import TestClient
from sqlalchemy import event, insert
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import engine
from app.main import app
from app.models import Visit


@pytest.mark.parametrize(
    "address", ["example.org:8080/path", "localhost:8080/path", "[::1]:8080/path"]
)
def test_scheme_optional_address_with_port(client, auth, address):
    result = client.post("/api/links", headers=auth, json={"original_url": address})
    assert result.status_code == 201, result.text
    assert result.json()["original_url"] == "https://" + address
    assert (
        client.get("/r/" + result.json()["short_code"], follow_redirects=False).headers[
            "location"
        ]
        == "https://" + address
    )


def test_public_address_with_port(client, auth):
    response = client.put(
        "/api/admin/settings", headers=auth, json={"base_url": "localhost:8080"}
    )
    assert response.status_code == 200
    assert response.json()["base_url"] == "https://localhost:8080"


@pytest.mark.parametrize("field", ["link_id", "visit_id"])
def test_restore_rejects_sqlite_id_overflow_without_modifying_data(
    client, auth, make_link, field
):
    make_link()
    client.get("/r/example", follow_redirects=False)
    snapshot = client.get("/api/admin/backup", headers=auth).json()
    invalid = copy.deepcopy(snapshot)
    if field == "link_id":
        invalid["links"][0]["id"] = 1 << 63
        invalid["visits"][0]["short_link_id"] = 1 << 63
    else:
        invalid["visits"][0]["id"] = 1 << 63
    response = client.post(
        "/api/admin/restore",
        headers={**auth, "X-Confirm-Restore": "RESTORE"},
        json=invalid,
    )
    assert response.status_code == 400
    unchanged = client.get("/api/admin/backup", headers=auth).json()
    assert unchanged["links"] == snapshot["links"]
    assert unchanged["visits"] == snapshot["visits"]


def test_large_history_backup_roundtrip_and_bounded_delete(
    client, auth, make_link, monkeypatch
):
    import app.routers.settings as settings_router

    link = make_link(tags=["history"])

    async def seed():
        async with engine.begin() as conn:
            await conn.execute(
                insert(Visit),
                [
                    {"short_link_id": link["id"], "user_agent": "X" * 2048}
                    for _ in range(1205)
                ],
            )

    client.portal.call(seed)
    snapshot = client.get("/api/admin/backup", headers=auth).json()
    assert len(snapshot["visits"]) == 1205
    with monkeypatch.context() as patch:
        patch.setattr(settings_router, "MAX_BACKUP_BYTES", 4096)
        assert client.get("/api/admin/backup", headers=auth).status_code == 413
    restored = client.post(
        "/api/admin/restore",
        headers={**auth, "X-Confirm-Restore": "RESTORE"},
        json=snapshot,
    )
    assert restored.status_code == 200
    assert restored.json()["visits"] == 1205
    assert (
        client.get("/api/links/stats/example", headers=auth).json()["total_visits"]
        == 1205
    )
    loaded_visits = []

    def loaded(visit, context):
        loaded_visits.append(visit.id)

    event.listen(Visit, "load", loaded)
    try:
        assert client.delete("/api/links/example", headers=auth).status_code == 204
    finally:
        event.remove(Visit, "load", loaded)
    assert loaded_visits == [], "Deletion must not materialize the entire visit history"
    snapshot = client.get("/api/admin/backup", headers=auth).json()
    assert snapshot["links"] == snapshot["visits"] == []


def test_commit_failure_returns_error_and_rolls_back_insert(client, auth, monkeypatch):
    async def fail_commit(self):
        raise RuntimeError("simulated commit failure")

    with monkeypatch.context() as patch:
        patch.setattr(AsyncSession, "commit", fail_commit)
        with TestClient(app, raise_server_exceptions=False) as failing_client:
            response = failing_client.post(
                "/api/links",
                headers=auth,
                json={"original_url": "https://example.org", "short_code": "rollback"},
            )
        assert response.status_code == 500
    assert client.get("/api/links", headers=auth).json()["total"] == 0


def test_concurrent_redirects_and_creates_commit_consistently(client, auth, make_link):
    make_link()

    def visit(_):
        return client.get("/r/example", follow_redirects=False).status_code

    def create(index):
        return client.post(
            "/api/links",
            headers=auth,
            json={
                "original_url": "https://example.org",
                "short_code": f"parallel-{index}",
            },
        ).status_code

    with ThreadPoolExecutor(max_workers=6) as pool:
        assert list(pool.map(visit, range(12))) == [302] * 12
        assert list(pool.map(create, range(8))) == [201] * 8
    stats = client.get("/api/analytics", headers=auth).json()
    assert stats["summary"]["total_visits"] == stats["summary"]["period_visits"] == 12
    assert stats["summary"]["links"] == 9
