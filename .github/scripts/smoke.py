"""Exercise a running production container from the host using Python stdlib only."""

import json
import os
import re
import time
from http.client import HTTPException
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen

BASE = os.environ.get("SMOKE_BASE_URL", "http://127.0.0.1:5201")


def request(path, *, data=None, token=None, headers=None):
    headers = dict(headers or {})
    if token:
        headers["Authorization"] = "Bearer " + token
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = Request(
        BASE + path,
        data=json.dumps(data).encode() if data is not None else None,
        headers=headers,
    )
    with urlopen(req, timeout=5) as response:
        return json.load(response)


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def wait_for_health(timeout=60):
    deadline = time.monotonic() + timeout
    while True:
        try:
            return request("/api/health")
        except (URLError, TimeoutError, ConnectionError, HTTPException) as error:
            if time.monotonic() >= deadline:
                raise RuntimeError(
                    f"Production server did not become ready: {error}"
                ) from error
            time.sleep(0.2)


def main():
    expected_version = os.environ["EXPECTED_VERSION"]
    health = wait_for_health()
    assert health == {"status": "ok", "version": expected_version}, health
    assert request("/api/site")["version"] == expected_version
    assert request("/openapi.json")["info"]["version"] == expected_version
    for path in ["/", "/login", "/links", "/settings"]:
        with urlopen(BASE + path, timeout=5) as response:
            html = response.read().decode()
            assert '<div id="app">' in html
    assets = re.findall(r'(?:src|href)="(/assets/[^\"]+)"', html)
    assert assets, "Built frontend assets are missing"
    for path in assets:
        with urlopen(BASE + path, timeout=5) as response:
            assert response.read(), path
    try:
        request("/api/links")
        raise AssertionError("Management endpoints must require login")
    except HTTPError as error:
        assert error.code == 401, error.code
    token = request("/api/admin/login", data={"password": os.environ["ADMIN_PASSWORD"]})[
        "token"
    ]
    created = request(
        "/api/links",
        data={
            "original_url": "https://example.org/smoke",
            "short_code": "smoke-test",
            "tags": ["smoke"],
        },
        token=token,
    )
    assert created["short_code"] == "smoke-test"
    try:
        build_opener(NoRedirect).open(BASE + "/r/smoke-test", timeout=5)
        raise AssertionError("Short link must redirect")
    except HTTPError as error:
        assert error.code == 302
        assert error.headers["Location"] == "https://example.org/smoke"
    stats = request("/api/analytics", token=token)
    assert stats["summary"]["total_visits"] == 1
    snapshot = request("/api/admin/backup", token=token)
    assert len(snapshot["links"]) == len(snapshot["visits"]) == 1
    restored = request(
        "/api/admin/restore",
        data=snapshot,
        token=token,
        headers={"X-Confirm-Restore": "RESTORE"},
    )
    assert restored["links"] == restored["visits"] == 1
    assert request("/api/admin/settings", token=token)["version"] == expected_version
    print(
        f"Production HTTP, frontend assets, auth, redirect, analytics and backup/restore OK ({expected_version})"
    )


if __name__ == "__main__":
    main()
