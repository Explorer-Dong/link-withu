"""Exercise a production image using only its installed runtime dependencies."""

import json
import os
import re
import subprocess
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen

BASE = "http://127.0.0.1:5201"
EXPECTED_VERSION = os.environ["EXPECTED_VERSION"]


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


server = subprocess.Popen(
    [
        sys.executable,
        "-m",
        "uvicorn",
        "app.main:app",
        "--host",
        "127.0.0.1",
        "--port",
        "5201",
        "--no-proxy-headers",
    ]
)
try:
    deadline = time.monotonic() + 60
    while True:
        try:
            health = request("/api/health")
            break
        except (URLError, TimeoutError):
            if server.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError("Production server did not become ready") from None
            time.sleep(0.2)
    assert health == {"status": "ok", "version": EXPECTED_VERSION}, health
    assert request("/api/site")["version"] == EXPECTED_VERSION
    assert request("/openapi.json")["info"]["version"] == EXPECTED_VERSION
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
    token = request(
        "/api/admin/login", data={"password": os.environ["ADMIN_PASSWORD"]}
    )["token"]
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
    assert request("/api/admin/settings", token=token)["version"] == EXPECTED_VERSION
    print(
        f"Production HTTP, frontend assets, auth, redirect, analytics and backup/restore OK ({EXPECTED_VERSION})"
    )
finally:
    server.terminate()
    try:
        server.wait(timeout=10)
    except subprocess.TimeoutExpired:
        server.kill()
        server.wait()
