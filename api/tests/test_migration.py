import os
import sqlite3
import subprocess
import sys
from pathlib import Path


def test_old_database_upgrade_preserves_history_and_is_repeatable(tmp_path):
    path = tmp_path / "legacy.db"
    with sqlite3.connect(path) as conn:
        conn.executescript("""
        CREATE TABLE short_links (id INTEGER PRIMARY KEY, short_code VARCHAR(32) UNIQUE NOT NULL, original_url VARCHAR(2048) NOT NULL, is_custom INTEGER, created_at DATETIME);
        CREATE TABLE visits (id INTEGER PRIMARY KEY, short_link_id INTEGER NOT NULL REFERENCES short_links(id), visited_at DATETIME, ip VARCHAR(45), user_agent TEXT);
        INSERT INTO short_links VALUES (12, '旧链接', 'https://example.org', 1, '2024-01-01 00:00:00');
        INSERT INTO visits VALUES (18, 12, '2024-01-02 00:00:00', '127.0.0.1', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/130.0');
        INSERT INTO visits VALUES (19, 12, '2024-01-03 00:00:00', '127.0.0.1', 'Googlebot/2.1');
        """)
    env = {**os.environ, "DATABASE_URL": f"sqlite+aiosqlite:///{path}"}
    code = "import asyncio; from app.database import init_db, engine; asyncio.run(init_db()); asyncio.run(engine.dispose())"
    for _ in range(2):
        subprocess.run(
            [sys.executable, "-c", code],
            cwd=Path(__file__).resolve().parents[1],
            env=env,
            check=True,
            capture_output=True,
        )
    with sqlite3.connect(path) as conn:
        assert conn.execute(
            "SELECT id, short_code, title, note FROM short_links"
        ).fetchall() == [(12, "旧链接", "", "")]
        assert conn.execute(
            "SELECT id, short_link_id, device, browser, is_bot, referrer_domain FROM visits ORDER BY id"
        ).fetchall() == [
            (18, 12, "desktop", "Chrome", 0, None),
            (19, 12, "unknown", "unknown", 1, None),
        ]
        assert conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        assert conn.execute("PRAGMA foreign_key_check").fetchall() == []
        assert "link_tags" in {
            row[0]
            for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")
        }
