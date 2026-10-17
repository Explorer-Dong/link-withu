mod common;
use common::Server;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

fn seed_snapshot(s: &Server) -> Value {
    s.create(
        "example",
        json!({"title":"备份测试","tags":["备份"],"note":"保留备注"}),
    );
    assert_eq!(s.get("/r/example").status(), 302);
    s.json("/api/admin/backup")
}
#[test]
fn backup_restore_roundtrip_and_invalid_documents_preserve_data() {
    let s = Server::new();
    let snapshot = seed_snapshot(&s);
    s.create("new", json!({}));
    assert_eq!(s.post("/api/admin/restore", &snapshot).status(), 400);
    for mutation in [
        "duplicate_link",
        "orphan_visit",
        "bad_url",
        "duplicate_visit",
        "bad_settings",
        "overflow_link",
        "overflow_visit",
        "bad_time",
        "bad_agent",
    ] {
        let mut invalid = snapshot.clone();
        match mutation {
            "duplicate_link" => {
                let link = invalid["links"][0].clone();
                invalid["links"].as_array_mut().unwrap().push(link);
            }
            "orphan_visit" => invalid["visits"][0]["short_link_id"] = json!(9999),
            "bad_url" => invalid["links"][0]["original_url"] = json!("file:///etc/passwd"),
            "duplicate_visit" => {
                let visit = invalid["visits"][0].clone();
                invalid["visits"].as_array_mut().unwrap().push(visit);
            }
            "bad_settings" => invalid["settings"]["timezone"] = json!("Invalid/Time"),
            "overflow_link" => invalid["links"][0]["id"] = json!(1_u64 << 63),
            "overflow_visit" => invalid["visits"][0]["id"] = json!(1_u64 << 63),
            "bad_time" => invalid["visits"][0]["visited_at"] = json!("invalid"),
            "bad_agent" => invalid["visits"][0]["user_agent"] = json!("X".repeat(8193)),
            _ => unreachable!(),
        }
        let r = s.restore(&invalid);
        assert_eq!(r.status(), 400, "{mutation}: {}", r.text().unwrap());
        assert_eq!(s.json("/api/links")["total"], 2);
    }
    assert_eq!(s.restore(&snapshot).status(), 200);
    let restored = s.json("/api/admin/backup");
    assert_eq!(restored["links"], snapshot["links"]);
    assert_eq!(restored["visits"], snapshot["visits"]);
    assert_eq!(restored["settings"], snapshot["settings"]);
    assert!(!restored.to_string().contains("admin_password"));
}
#[test]
fn streaming_restore_accepts_reordered_members_and_normalizes_utc() {
    let s = Server::new();
    let mut snapshot = seed_snapshot(&s);
    snapshot["links"][0]["created_at"] = json!("2024-01-01T08:00:00+08:00");
    let body = format!(
        "{{\"visits\":{},\"links\":{},\"exported_at\":{},\"settings\":{},\"version\":1,\"format\":\"link-withu-backup\",\"extra\":[1,2,3]}}",
        snapshot["visits"], snapshot["links"], snapshot["exported_at"], snapshot["settings"]
    );
    let r = s
        .client
        .post(format!("{}/api/admin/restore", s.base))
        .bearer_auth(&s.token)
        .header("X-Confirm-Restore", "RESTORE")
        .header("Content-Type", "application/json")
        .body(reqwest::blocking::Body::new(std::io::Cursor::new(
            body.into_bytes(),
        )))
        .send()
        .unwrap();
    assert_eq!(r.status(), 200, "{}", r.text().unwrap());
    assert_eq!(
        s.json("/api/admin/backup")["links"][0]["created_at"],
        "2024-01-01T00:00:00Z"
    );
}

#[test]
fn legacy_optional_fields_and_metadata_validation_are_covered() {
    let s = Server::new();
    let original = seed_snapshot(&s);
    for field in [
        "format",
        "version",
        "exported_at",
        "settings",
        "links",
        "visits",
    ] {
        let mut invalid = original.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert_eq!(s.restore(&invalid).status(), 400, "missing {field}");
    }
    for (field, value) in [
        ("format", json!("other")),
        ("version", json!(2)),
        ("exported_at", json!("invalid")),
        ("links", json!({})),
        ("visits", json!({})),
    ] {
        let mut invalid = original.clone();
        invalid[field] = value;
        assert_eq!(s.restore(&invalid).status(), 400, "invalid {field}");
    }
    let mut legacy = original.clone();
    legacy["settings"] = json!({});
    for field in ["title", "note", "tags"] {
        legacy["links"][0].as_object_mut().unwrap().remove(field);
    }
    for field in [
        "ip",
        "user_agent",
        "referrer_domain",
        "device",
        "browser",
        "is_bot",
    ] {
        legacy["visits"][0].as_object_mut().unwrap().remove(field);
    }
    assert_eq!(s.restore(&legacy).status(), 200);
    let restored = s.json("/api/admin/backup");
    assert_eq!(restored["links"][0]["title"], "");
    assert_eq!(restored["links"][0]["tags"], json!([]));
    assert_eq!(restored["visits"][0]["device"], "unknown");
    assert_eq!(restored["visits"][0]["is_bot"], false);
    assert_eq!(restored["visits"][0]["user_agent"], Value::Null);
    assert_eq!(restored["settings"]["app_name"], "Link WithU");
    assert_eq!(s.json("/api/analytics")["summary"]["total_visits"], 1);
}
#[test]
fn malformed_and_oversized_strings_never_publish_staged_records() {
    let s = Server::new();
    let snapshot = seed_snapshot(&s);
    for body in [
        format!("{}x", snapshot),
        snapshot.to_string().trim_end_matches('}').to_string(),
        format!("{{\"unknown\":\"{}\"}}", "X".repeat(128 * 1024 + 1)),
        r#"{"format":"link-withu-backup","format":"link-withu-backup","version":1}"#.to_string(),
    ] {
        let r = s
            .client
            .post(format!("{}/api/admin/restore", s.base))
            .bearer_auth(&s.token)
            .header("X-Confirm-Restore", "RESTORE")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .unwrap();
        assert_eq!(r.status(), 400);
        assert_eq!(s.json("/api/admin/backup")["links"], snapshot["links"]);
    }
    assert_eq!(
        std::fs::read_dir(s.directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".link-withu-restore-"))
            .count(),
        0
    );
}
#[test]
fn failed_restore_database_write_rolls_back_the_entire_replacement() {
    let s = Server::new();
    let snapshot = seed_snapshot(&s);
    s.create("also-keep", json!({}));
    s.sql("CREATE TRIGGER fail_settings BEFORE INSERT ON app_settings BEGIN SELECT RAISE(ABORT,'injected write failure'); END;");
    assert_eq!(s.restore(&snapshot).status(), 500);
    assert_eq!(s.json("/api/links")["total"], 2);
    assert_eq!(
        s.json("/api/links/tags"),
        json!([{"name":"备份","link_count":1}])
    );
    s.sql("DROP TRIGGER fail_settings;");
    assert_eq!(s.restore(&snapshot).status(), 200);
}
#[test]
fn failed_commit_rolls_back_insert_and_keeps_worker_usable() {
    let s = Server::new();
    s.sql("CREATE TABLE deferred_failure(id INTEGER REFERENCES short_links(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_commit AFTER INSERT ON short_links BEGIN INSERT INTO deferred_failure VALUES (-999); END;");
    assert_eq!(
        s.post(
            "/api/links",
            &json!({"original_url":"https://example.org","short_code":"rollback"})
        )
        .status(),
        500
    );
    assert_eq!(s.json("/api/links")["total"], 0);
    s.sql("DROP TRIGGER fail_commit;");
    s.create("after", json!({}));
    assert_eq!(s.json("/api/links")["total"], 1);
}
#[test]
fn large_history_roundtrip_and_real_backup_limit() {
    let s = Server::new();
    let link = s.create("example", json!({"tags":["history"]}));
    s.seed_visits(link["id"].as_i64().unwrap(), 1205, &"X".repeat(2048));
    let snapshot = s.json("/api/admin/backup");
    assert_eq!(snapshot["visits"].as_array().unwrap().len(), 1205);
    assert_eq!(s.restore(&snapshot).status(), 200);
    assert_eq!(s.json("/api/links/stats/example")["total_visits"], 1205);
    s.seed_visits(link["id"].as_i64().unwrap(), 7000, &"X".repeat(8192));
    assert_eq!(s.get("/api/admin/backup").status(), 413);
    assert_eq!(s.json("/api/links/stats/example")["total_visits"], 8205);
    assert_eq!(s.delete("/api/links/example").status(), 204);
    let db = rusqlite::Connection::open(&s.database).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM visits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        !db.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_some()
    );
}
#[test]
fn oversized_content_length_is_rejected_before_upload() {
    let s = Server::new();
    let address = s.base.trim_start_matches("http://");
    let mut socket = std::net::TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    write!(socket,"POST /api/admin/restore HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nX-Confirm-Restore: RESTORE\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",s.token,50*1024*1024+1).unwrap();
    let mut status = String::new();
    std::io::BufReader::new(socket)
        .read_line(&mut status)
        .unwrap();
    assert!(status.contains(" 413 "), "{status}");
}

#[test]
fn chunked_uploads_without_content_length_cannot_bypass_the_restore_limit() {
    use std::io::Read;
    let s = Server::new();
    let original = seed_snapshot(&s);
    // Whitespace keeps the streaming parser waiting for JSON instead of failing
    // early, so this exercises the actual received-byte limit.
    let body = std::io::repeat(b' ').take(50 * 1024 * 1024 + 1);
    let response = s
        .client
        .post(format!("{}/api/admin/restore", s.base))
        .bearer_auth(&s.token)
        .header("X-Confirm-Restore", "RESTORE")
        .header("Content-Type", "application/json")
        .body(reqwest::blocking::Body::new(body))
        .send()
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(s.json("/api/admin/backup")["links"], original["links"]);
    assert_eq!(s.restore(&original).status(), 200);
}
#[test]
fn disconnected_upload_rolls_back_and_releases_restore_slot() {
    let s = Server::new();
    let snapshot = seed_snapshot(&s);
    let mut socket = std::net::TcpStream::connect(s.base.trim_start_matches("http://")).unwrap();
    write!(socket,"POST /api/admin/restore HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nX-Confirm-Restore: RESTORE\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n{{\"format\":\"link-withu-backup\",\"version\":1,\"links\":[",s.token).unwrap();
    drop(socket);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let r = s.restore(&snapshot);
        if r.status() == 200 {
            break;
        }
        assert_eq!(r.status(), 409);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.json("/api/admin/backup")["links"], snapshot["links"]);
}

#[test]
fn slow_restore_upload_does_not_lock_short_link_writes() {
    let s = Server::new();
    let snapshot = seed_snapshot(&s);
    let mut socket = std::net::TcpStream::connect(s.base.trim_start_matches("http://")).unwrap();
    write!(socket,"POST /api/admin/restore HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nX-Confirm-Restore: RESTORE\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n{{\"format\":\"link-withu-backup\",\"version\":1,\"links\":[",s.token).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let entries = std::fs::read_dir(s.directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".link-withu-restore-")
            })
            .count();
        if entries > 0 {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.restore(&snapshot).status(), 409);
    for _ in 0..10 {
        assert_eq!(s.get("/r/example").status(), 302);
    }
    assert_eq!(s.json("/api/links/stats/example")["total_visits"], 11);
    drop(socket);
}
