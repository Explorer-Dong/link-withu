mod common;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::Server;
use hmac::{Hmac, Mac};
use reqwest::Method;
use serde_json::{Value, json};
use sha2::Sha256;

fn token(payload: Value, secret: &str) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    format!(
        "{payload}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}

#[test]
fn auth_expiry_tampering_password_rotation_and_case_insensitive_scheme() {
    let s = Server::new();
    for (payload, secret) in [
        (json!({"exp":1}), "integration-test-only"),
        (json!({"exp":"9999999999"}), "integration-test-only"),
        (json!({"nonce":"missing-exp"}), "integration-test-only"),
        (
            json!({"exp":chrono::Utc::now().timestamp()+3600}),
            "old-password",
        ),
    ] {
        assert_eq!(
            s.client
                .get(format!("{}/api/links", s.base))
                .bearer_auth(token(payload, secret))
                .send()
                .unwrap()
                .status(),
            401
        );
    }
    for method in [Method::POST, Method::PUT, Method::DELETE] {
        for path in [
            "/api/links",
            "/api/links/example",
            "/api/admin/settings",
            "/api/admin/restore",
        ] {
            assert_eq!(
                s.request(method.clone(), path, Some(&json!({})), false)
                    .status(),
                401
            );
        }
    }
    for scheme in ["Bearer", "bearer", "BEARER"] {
        assert_eq!(
            s.client
                .get(format!("{}/api/links", s.base))
                .header("Authorization", format!("{scheme} {}", s.token))
                .send()
                .unwrap()
                .status(),
            200
        );
    }
}

#[test]
fn missing_password_disables_management_but_health_and_redirect_remain_available() {
    let s = Server::with_env(&[("ADMIN_PASSWORD", "")]);
    assert_eq!(
        s.post("/api/admin/login", &json!({"password":""})).status(),
        503
    );
    assert_eq!(
        s.client
            .get(format!("{}/api/links", s.base))
            .bearer_auth("anything")
            .send()
            .unwrap()
            .status(),
        503
    );
    s.sql("INSERT INTO short_links(short_code,original_url,created_at) VALUES('public','https://example.org','2026-01-01 00:00:00');");
    assert_eq!(s.get("/api/health").status(), 200);
    assert_eq!(s.get("/r/public").status(), 302);
    let db = rusqlite::Connection::open(&s.database).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM visits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn malformed_json_field_boundaries_and_large_requests_leave_writer_usable() {
    let s = Server::new();
    for body in [
        "{",
        "null",
        "[]",
        "42",
        r#"{"original_url":null}"#,
        r#"{"original_url":"example.org","short_code":1}"#,
        r#"{"original_url":"example.org","tags":null}"#,
    ] {
        assert_eq!(
            s.client
                .post(format!("{}/api/links", s.base))
                .bearer_auth(&s.token)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .unwrap()
                .status(),
            422
        );
    }
    for body in [
        json!({}),
        json!({"password":null}),
        json!({"password":"x".repeat(1025)}),
    ] {
        assert_eq!(s.post("/api/admin/login", &body).status(), 422);
    }
    assert_eq!(
        s.client
            .post(format!("{}/api/links", s.base))
            .bearer_auth(&s.token)
            .body(" ".repeat(64 * 1024 + 1))
            .send()
            .unwrap()
            .status(),
        413
    );
    for body in [
        json!({"original_url":"example.org","note":"x".repeat(2001)}),
        json!({"original_url":"example.org","tags":["x".repeat(41)]}),
        json!({"original_url":"example.org","tags":["a\nb"]}),
    ] {
        assert_eq!(s.post("/api/links", &body).status(), 422);
    }
    let link = s.create(
        "valid",
        json!({"title":" 标题 ","note":" 备注 ","tags":["#keep","keep"," ","#","#other"]}),
    );
    assert_eq!(link["title"], "标题");
    assert_eq!(link["note"], "备注");
    assert_eq!(link["tags"], json!(["keep", "other"]));
}

#[test]
fn every_numeric_filter_and_method_boundary_is_checked() {
    let s = Server::new();
    s.create("boundary", json!({}));
    for path in [
        "/api/links?skip=x",
        "/api/links?limit=0",
        "/api/links?limit=9223372036854775808",
        "/api/links/stats/boundary?skip=-1",
        "/api/links/stats/boundary?limit=201",
        "/api/analytics?days=-1",
        "/api/analytics?days=x",
        "/api/analytics/export?days=91",
        "/api/analytics/export?exclude_bots=invalid",
    ] {
        assert_eq!(s.get(path).status(), 422, "{path}");
    }
    for path in [
        "/api/analytics?short_code=missing",
        "/api/links/stats/missing",
        "/r/missing",
    ] {
        assert_eq!(s.get(path).status(), 404, "{path}");
    }
    for method in [Method::PUT, Method::DELETE, Method::POST] {
        let r = s.request(method, "/api/links/stats/boundary", Some(&json!({})), true);
        assert_eq!(r.status(), 405);
        assert_eq!(r.headers()["allow"], "GET");
    }
    assert_eq!(s.json("/api/links?sort=oldest&skip=1")["items"], json!([]));
}

#[test]
fn invalid_forwarded_addresses_cannot_poison_portable_backups() {
    let s = Server::configured("Asia/Shanghai", 1);
    s.create("proxy", json!({}));
    for address in ["invalid", &"X".repeat(200), "203.0.113.5", "2001:db8::1"] {
        assert_eq!(
            s.client
                .get(format!("{}/r/proxy", s.base))
                .header("X-Forwarded-For", address)
                .send()
                .unwrap()
                .status(),
            302
        );
    }
    let backup = s.json("/api/admin/backup");
    let ips: Vec<_> = backup["visits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["ip"].as_str().unwrap())
        .collect();
    assert_eq!(
        ips,
        vec!["127.0.0.1", "127.0.0.1", "203.0.113.5", "2001:db8::1"]
    );
    assert_eq!(s.restore(&backup).status(), 200);
}

#[test]
fn unsupported_dates_are_rejected_atomically_before_analytics_can_fail() {
    let s = Server::configured("America/New_York", 0);
    s.create("dates", json!({}));
    assert_eq!(s.get("/r/dates").status(), 302);
    let backup = s.json("/api/admin/backup");
    for date in [
        "0000-01-01T00:00:00",
        "-262143-01-01T00:00:00",
        "+10000-01-01T00:00:00",
        "2016-12-31T23:59:60Z",
    ] {
        for (section, field) in [("links", "created_at"), ("visits", "visited_at")] {
            let mut invalid = backup.clone();
            invalid[section][0][field] = json!(date);
            assert_eq!(s.restore(&invalid).status(), 400, "{date}");
            assert_eq!(s.json("/api/admin/backup")[section], backup[section]);
        }
    }
    assert_eq!(s.json("/api/analytics")["summary"]["total_visits"], 1);
}

#[test]
fn static_routes_assets_mime_types_and_traversal_guards_are_always_tested() {
    use std::io::{Read, Write};
    let frontend = tempfile::tempdir().unwrap();
    std::fs::create_dir(frontend.path().join("assets")).unwrap();
    for (path, body) in [
        ("index.html", "<div id=\"app\">fixture</div>"),
        ("favicon.svg", "<svg/>"),
        ("assets/app.js", "console.log('ok')"),
        ("assets/app.css", "body{}"),
        ("assets/manifest.json", "{}"),
    ] {
        std::fs::write(frontend.path().join(path), body).unwrap();
    }
    let s = Server::with_env(&[("FRONTEND_DIST", frontend.path().to_str().unwrap())]);
    for path in ["/", "/login", "/links", "/analytics", "/settings"] {
        assert!(s.get(path).text().unwrap().contains("fixture"));
    }
    for (path, mime) in [
        ("/favicon.svg", "image/svg+xml"),
        ("/assets/app.js", "text/javascript"),
        ("/assets/app.css", "text/css"),
        ("/assets/manifest.json", "application/json"),
    ] {
        let r = s.get(path);
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["content-type"], mime);
        assert!(
            r.headers()["content-length"]
                .to_str()
                .unwrap()
                .parse::<usize>()
                .unwrap()
                > 0
        );
    }
    for path in [
        "/assets/missing.js",
        "/assets/%2e%2e/Cargo.toml",
        "/assets/%2fetc/passwd",
    ] {
        let mut socket =
            std::net::TcpStream::connect(s.base.trim_start_matches("http://")).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(
            socket,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut result = String::new();
        socket.read_to_string(&mut result).unwrap();
        assert!(result.starts_with("HTTP/1.1 404"), "{path}: {result}");
    }
}

#[test]
fn graceful_shutdown_commits_visits_and_closes_the_listener() {
    let mut s = Server::new();
    s.create("shutdown", json!({}));
    assert_eq!(s.get("/r/shutdown").status(), 302);
    assert!(s.stop().success());
    assert!(std::net::TcpStream::connect(s.base.trim_start_matches("http://")).is_err());
    let db = rusqlite::Connection::open(&s.database).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM visits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn admission_backpressure_preserves_health_and_recovers_after_disconnect() {
    use std::io::{BufRead, Write};
    let s = Server::new();
    let mut sockets = Vec::new();
    for _ in 0..64 {
        let mut socket =
            std::net::TcpStream::connect(s.base.trim_start_matches("http://")).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(socket,"POST /api/admin/login HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1024\r\nContent-Type: application/json\r\nExpect: 100-continue\r\n\r\n").unwrap();
        // Wait until this handler is reading its body before occupying the next slot.
        // Probing too early could itself consume the 64th permit and reject an upload.
        let mut reader = std::io::BufReader::new(&mut socket);
        let mut status = String::new();
        reader.read_line(&mut status).unwrap();
        assert!(status.contains(" 100 "), "{status}");
        status.clear();
        reader.read_line(&mut status).unwrap();
        assert_eq!(status, "\r\n");
        sockets.push(socket);
    }
    assert_eq!(s.get("/api/site").status(), 503);
    assert_eq!(s.get("/api/health").status(), 200);
    drop(sockets);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while s.get("/api/site").status() != 200 {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    s.create("after-backpressure", json!({}));
}

#[test]
fn scoped_csv_exports_keep_real_records_and_escape_visitor_formulas() {
    let s = Server::new();
    s.create(
        "csv-one",
        json!({"tags":["chosen"],"note":"comma, quote\" and\nnewline"}),
    );
    s.create("csv-two", json!({"tags":["other"]}));
    assert_eq!(
        s.client
            .get(format!("{}/r/csv-one", s.base))
            .header("User-Agent", "=FORMULA()")
            .header("Referer", "https://blog.example.org/private?secret=hidden")
            .send()
            .unwrap()
            .status(),
        302
    );
    assert_eq!(s.get("/r/csv-two").status(), 302);
    let response = s.get("/api/analytics/export?kind=visits&tag=chosen&short_code=csv-one");
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    let text = response.text().unwrap();
    assert!(text.starts_with('\u{feff}'));
    let records: Vec<_> = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes())
        .records()
        .map(Result::unwrap)
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(&records[0][0], "csv-one");
    assert_eq!(&records[0][3], "blog.example.org");
    assert_eq!(&records[0][7], "'=FORMULA()");
    let text = s
        .get("/api/analytics/export?kind=links&tag=chosen")
        .text()
        .unwrap();
    let records: Vec<_> = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes())
        .records()
        .map(Result::unwrap)
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(&records[0][3], "comma, quote\" and\nnewline");
}

#[test]
fn browser_device_and_bot_categories_are_classified_from_realistic_agents() {
    let s = Server::new();
    s.create("agents", json!({}));
    let cases = [
        ("", "unknown", "unknown", false),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/130.0 Safari/537.36 Edg/130.0",
            "desktop",
            "Edge",
            false,
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64) Chrome/130.0 Safari/537.36 OPR/114.0",
            "desktop",
            "Opera",
            false,
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; SM-X710) Chrome/130.0 Safari/537.36",
            "tablet",
            "Chrome",
            false,
        ),
        (
            "Mozilla/5.0 (iPad; CPU OS 18_0) Version/18.0 Safari/604.1",
            "tablet",
            "Safari",
            false,
        ),
        (
            "Mozilla/5.0 (Android 14; Mobile; rv:132.0) Gecko/132.0 Firefox/132.0",
            "mobile",
            "Firefox",
            false,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0) CriOS/130.0 Mobile Safari/604.1",
            "mobile",
            "Chrome",
            false,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0) FxiOS/132.0 Mobile Safari/604.1",
            "mobile",
            "Firefox",
            false,
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; Mobile) Chrome/130.0 EdgA/130.0",
            "mobile",
            "Edge",
            false,
        ),
        ("curl/8.0", "unknown", "unknown", true),
        ("python-requests/2.32", "unknown", "unknown", true),
        ("facebookexternalhit/1.1", "unknown", "unknown", true),
    ];
    for (agent, _, _, _) in cases {
        assert_eq!(
            s.client
                .get(format!("{}/r/agents", s.base))
                .header("User-Agent", agent)
                .header("Referer", "invalid-referrer")
                .send()
                .unwrap()
                .status(),
            302
        );
    }
    let backup = s.json("/api/admin/backup");
    for (record, (_, device, browser, bot)) in
        backup["visits"].as_array().unwrap().iter().zip(cases)
    {
        assert_eq!(record["device"], device);
        assert_eq!(record["browser"], browser);
        assert_eq!(record["is_bot"], bot);
        assert_eq!(record["referrer_domain"], Value::Null);
    }
    assert_eq!(s.restore(&backup).status(), 200);
}
