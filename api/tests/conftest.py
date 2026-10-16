import os
import sys
import tempfile
from pathlib import Path

# Set environment before importing the application; never open the deployment database.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
TEST_DIR = tempfile.TemporaryDirectory(prefix="link-withu-tests-")
os.environ["DATABASE_URL"] = f"sqlite+aiosqlite:///{TEST_DIR.name}/test.db"
os.environ["ADMIN_PASSWORD"] = "integration-test-only"
os.environ["BASE_URL"] = "http://testserver"
os.environ["TZ"] = "Asia/Shanghai"
os.environ["FRONTEND_DIST"] = str(Path(__file__).resolve().parents[2] / "web" / "dist")

import pytest  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402
from sqlalchemy import delete  # noqa: E402

from app.database import engine  # noqa: E402
from app.main import app  # noqa: E402
from app.models import AppSetting, LinkTag, ShortLink, Visit  # noqa: E402


@pytest.fixture
def client():
    with TestClient(app) as client:

        async def clear():
            async with engine.begin() as conn:
                for model in [Visit, LinkTag, ShortLink, AppSetting]:
                    await conn.execute(delete(model))

        portal = client.portal
        assert portal is not None
        portal.call(clear)
        yield client


@pytest.fixture
def auth(client):
    response = client.post(
        "/api/admin/login", json={"password": "integration-test-only"}
    )
    assert response.status_code == 200
    return {"Authorization": "Bearer " + response.json()["token"]}


@pytest.fixture
def make_link(client, auth):
    def create(code="example", **kwargs):
        response = client.post(
            "/api/links",
            headers=auth,
            json={
                "original_url": "https://example.org/path",
                "short_code": code,
                **kwargs,
            },
        )
        assert response.status_code == 201, response.text
        return response.json()

    return create
