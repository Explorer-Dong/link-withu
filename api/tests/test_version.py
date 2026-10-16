import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


def test_api_version_follows_project_manifest(tmp_path):
    api_root = Path(__file__).resolve().parents[1]
    shutil.copytree(
        api_root / "app",
        tmp_path / "app",
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    manifest = (api_root / "pyproject.toml").read_text()
    current_version = tomllib.loads(manifest)["project"]["version"]
    future_version = "9.8.7"
    (tmp_path / "pyproject.toml").write_text(
        manifest.replace(
            f'version = "{current_version}"', f'version = "{future_version}"', 1
        )
    )
    subprocess.run(
        [
            sys.executable,
            "-c",
            """
import os
from fastapi.testclient import TestClient
from app.main import app
expected = os.environ["EXPECTED_VERSION"]
assert app.version == expected
with TestClient(app) as client:
    assert client.get("/api/health").json()["version"] == expected
    assert client.get("/api/site").json()["version"] == expected
    token = client.post("/api/admin/login", json={"password": "version-test-only"}).json()["token"]
    response = client.get("/api/admin/settings", headers={"Authorization": "Bearer " + token})
    assert response.status_code == 200
    assert response.json()["version"] == expected
""",
        ],
        cwd=tmp_path,
        env={
            **os.environ,
            "PYTHONPATH": str(tmp_path),
            "DATABASE_URL": f"sqlite+aiosqlite:///{tmp_path / 'version.db'}",
            "ADMIN_PASSWORD": "version-test-only",
            "EXPECTED_VERSION": future_version,
        },
        check=True,
        capture_output=True,
        text=True,
    )
