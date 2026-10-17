mod common;
use common::Server;
use serde_json::json;
use std::{path::PathBuf, process::Command};
#[test]
fn old_schema_migration_preserves_history_and_is_repeatable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE short_links (id INTEGER PRIMARY KEY, short_code VARCHAR(32) UNIQUE NOT NULL, original_url VARCHAR(2048) NOT NULL, is_custom INTEGER, created_at DATETIME); CREATE TABLE visits (id INTEGER PRIMARY KEY, short_link_id INTEGER NOT NULL REFERENCES short_links(id), visited_at DATETIME, ip VARCHAR(45), user_agent TEXT); INSERT INTO short_links VALUES (12,'旧链接','https://example.org',1,'2024-01-01 00:00:00'); INSERT INTO visits VALUES (18,12,'2024-01-02 00:00:00','127.0.0.1','Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/130.0'); INSERT INTO visits VALUES (19,12,'2024-01-03 00:00:00','127.0.0.1','Googlebot/2.1');").unwrap();
    }
    for _ in 0..2 {
        let output = Command::new(common::binary())
            .arg("--init-db")
            .env("DATABASE_URL", format!("sqlite:///{}", path.display()))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let s = Server::from_directory(directory, "Asia/Shanghai", 0);
    let links = s.json("/api/links");
    assert_eq!(links["items"][0]["id"], 12);
    assert_eq!(links["items"][0]["title"], "");
    assert_eq!(s.get("/r/旧链接").status(), 302);
    let visits = s.json("/api/admin/backup")["visits"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(visits[0]["device"], "desktop");
    assert_eq!(visits[0]["browser"], "Chrome");
    assert_eq!(visits[1]["is_bot"], true);
    assert_eq!(s.restore(&s.json("/api/admin/backup")).status(), 200);
    assert_eq!(s.delete("/api/links/旧链接").status(), 204);
    assert_eq!(s.json("/api/admin/backup")["visits"], json!([]));
    let db = rusqlite::Connection::open(&s.database).unwrap();
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_none()
    );
}
#[test]
fn invalid_environment_is_rejected_without_opening_database() {
    for (key, value) in [
        ("TZ", "Invalid/Time"),
        ("RANDOM_CODE_LENGTH", "3"),
        ("TRUSTED_PROXY_COUNT", "-1"),
        ("PORT", "0"),
        ("BASE_URL", "http:example.org"),
        ("DATABASE_URL", "postgresql://localhost/db"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("invalid.db");
        let output = Command::new(common::binary())
            .arg("--init-db")
            .env("DATABASE_URL", format!("sqlite:///{}", db.display()))
            .env(key, value)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{key}");
        assert!(!db.exists());
    }
}
#[test]
fn health_probe_and_compiled_version() {
    let mut s = Server::new();
    let port = s.base.rsplit(':').next().unwrap();
    assert!(
        Command::new(common::binary())
            .arg("--healthcheck")
            .env("PORT", port)
            .status()
            .unwrap()
            .success()
    );
    let output = Command::new(common::binary())
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .starts_with(&format!("v{};", env!("CARGO_PKG_VERSION")))
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("web/package.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    let port = port.to_owned();
    assert!(s.stop().success());
    for value in [port.as_str(), "invalid"] {
        assert!(
            !Command::new(common::binary())
                .arg("--healthcheck")
                .env("PORT", value)
                .status()
                .unwrap()
                .success()
        );
    }
}
