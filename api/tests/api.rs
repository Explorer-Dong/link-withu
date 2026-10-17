mod common;
use common::Server;
use reqwest::Method;
use serde_json::{Value, json};

#[test]
fn private_management_and_invalid_authentication() {
    let s = Server::new();
    for path in [
        "/api/links",
        "/api/links/tags",
        "/api/analytics",
        "/api/admin/settings",
        "/api/admin/backup",
        "/api/analytics/export",
    ] {
        assert_eq!(s.request(Method::GET, path, None, false).status(), 401);
    }
    assert_eq!(
        s.post("/api/admin/login", &json!({"password":"wrong"}))
            .status(),
        401
    );
    for token in ["invalid", "a.b", "abc.", "abc.x", "a.b.c"] {
        assert_eq!(
            s.client
                .get(format!("{}/api/links", s.base))
                .bearer_auth(token)
                .send()
                .unwrap()
                .status(),
            401
        );
    }
}
#[test]
fn crud_search_tags_sort_and_public_redirect() {
    let s = Server::new();
    let one = s.create(
        "one",
        json!({"title":"博客文章","tags":["博客","技术","博客"],"note":"测试备注"}),
    );
    let two = s.create("two", json!({"title":"活动"}));
    assert_eq!(one["tags"], json!(["博客", "技术"]));
    let redirect = s.request(Method::GET, "/r/one", None, false);
    assert_eq!(redirect.status(), 302);
    assert!(redirect.bytes().unwrap().is_empty());
    for query in ["博客文章", "备注", "技术", "/path", "one"] {
        let result: Value = s
            .client
            .get(format!("{}/api/links", s.base))
            .bearer_auth(&s.token)
            .query(&[("q", query)])
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert!(
            result["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["id"] == one["id"])
        );
    }
    assert_eq!(s.json("/api/links?q=%25")["total"], 0);
    assert_eq!(
        s.json("/api/links?tag=%E5%8D%9A%E5%AE%A2")["items"][0]["short_code"],
        "one"
    );
    let ordered = s.json("/api/links?sort=visits&limit=1");
    assert_eq!(ordered["total"], 2);
    assert_eq!(ordered["items"][0]["short_code"], "one");
    let changed = s.put(
        "/api/links/one",
        &json!({"original_url":"new.example.org","title":"更新","tags":["新标签"]}),
    );
    assert_eq!(changed.status(), 200);
    let changed: Value = changed.json().unwrap();
    assert_eq!(changed["short_code"], "one");
    assert_eq!(changed["tags"], json!(["新标签"]));
    assert_eq!(
        s.get("/r/one").headers()["location"],
        "https://new.example.org"
    );
    assert_eq!(s.delete("/api/links/one").status(), 204);
    assert_eq!(s.get("/r/one").status(), 404);
    let backup = s.json("/api/admin/backup");
    assert_eq!(backup["visits"], json!([]));
    assert_eq!(backup["links"][0]["id"], two["id"]);
}
#[test]
fn input_validation_and_duplicate_codes() {
    let s = Server::new();
    s.create("example", json!({}));
    assert_eq!(
        s.post(
            "/api/links",
            &json!({"original_url":"https://example.org","short_code":"example"})
        )
        .status(),
        409
    );
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://",
        "https://user:pass@example.org",
        "https://example.org:invalid",
        "https://example.org/\nLocation: evil",
    ] {
        assert_eq!(
            s.post("/api/links", &json!({"original_url":url})).status(),
            422,
            "{url}"
        );
    }
    for path in [
        "/api/links?skip=-1",
        "/api/links?limit=101",
        "/api/links?sort=bad",
        "/api/analytics?days=91",
        "/api/analytics?exclude_bots=bad",
        "/api/analytics/export?kind=bad",
    ] {
        assert_eq!(s.get(path).status(), 422, "{path}");
    }
    for fields in [
        json!({"original_url":"example.org","tags":[1]}),
        json!({"original_url":"example.org","tags":vec!["x";21]}),
        json!({"original_url":"example.org","short_code":"中文"}),
        json!({"original_url":"example.org","title":"x".repeat(121)}),
    ] {
        assert_eq!(s.post("/api/links", &fields).status(), 422);
    }
}
#[test]
fn malformed_authority_cannot_stop_the_database_writer() {
    let s = Server::new();
    for address in [
        "http:example.org",
        "https:/example.org",
        "https:///example.org",
        "//example.org",
        "https:\\example.org",
    ] {
        assert_eq!(
            s.put("/api/admin/settings", &json!({"base_url": address}))
                .status(),
            422,
            "{address}"
        );
        assert_eq!(
            s.post("/api/links", &json!({"original_url": address}))
                .status(),
            422,
            "{address}"
        );
    }
    s.create("writer-still-usable", json!({}));
    assert_eq!(s.json("/api/links")["total"], 1);
}
#[test]
fn optional_scheme_and_ports() {
    let s = Server::new();
    for address in [
        "example.org:8080/path",
        "localhost:8080/path",
        "[::1]:8080/path",
    ] {
        let r = s.post("/api/links", &json!({"original_url":address}));
        assert_eq!(r.status(), 201);
        let link: Value = r.json().unwrap();
        assert_eq!(link["original_url"], format!("https://{address}"));
        assert_eq!(
            s.get(&format!("/r/{}", link["short_code"].as_str().unwrap()))
                .headers()["location"],
            format!("https://{address}")
        );
    }
    assert_eq!(
        s.put("/api/admin/settings", &json!({"base_url":"localhost:8080"}))
            .json::<Value>()
            .unwrap()["base_url"],
        "https://localhost:8080"
    );
}
#[test]
fn visitor_classification_filters_and_analytics_agree() {
    let s = Server::new();
    s.create("example", json!({"tags":["博客","推广"]}));
    let agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1";
    assert_eq!(
        s.client
            .get(format!("{}/r/example", s.base))
            .header("User-Agent", agent)
            .header("Referer", "https://blog.example.com/private?secret=hidden")
            .send()
            .unwrap()
            .status(),
        302
    );
    assert_eq!(
        s.client
            .get(format!("{}/r/example", s.base))
            .header("User-Agent", "Googlebot/2.1")
            .send()
            .unwrap()
            .status(),
        302
    );
    let result = s.json("/api/analytics?days=7");
    assert_eq!(result["summary"]["total_visits"], 2);
    assert_eq!(result["summary"]["today"], 2);
    assert_eq!(result["summary"]["suspected_bots"], 1);
    assert_eq!(
        result["trend"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["count"].as_i64().unwrap())
            .sum::<i64>(),
        2
    );
    assert!(
        result["devices"]
            .as_array()
            .unwrap()
            .contains(&json!({"name":"mobile","count":1}))
    );
    assert!(
        result["browsers"]
            .as_array()
            .unwrap()
            .contains(&json!({"name":"Safari","count":1}))
    );
    assert!(
        result["referrers"]
            .as_array()
            .unwrap()
            .contains(&json!({"name":"blog.example.com","count":1}))
    );
    let clean = s.json("/api/analytics?exclude_bots=true");
    assert_eq!(clean["summary"]["suspected_bots"], 1);
    assert_eq!(clean["summary"]["total_visits"], 1);
    assert_eq!(clean["summary"]["period_visits"], 1);
    assert_eq!(clean["top_links"][0]["count"], 1);
    assert!(
        clean["tags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["count"] == 1)
    );
    assert_eq!(s.json("/api/analytics?tag=missing")["summary"]["links"], 0);
    let stats = s.json("/api/links/stats/example?exclude_bots=true");
    assert_eq!(stats["total_visits"], 1);
    assert_eq!(stats["visits"][0]["device"], "mobile");
    assert_eq!(stats["visits"][0]["referrer_domain"], "blog.example.com");
    let csv = s
        .get("/api/analytics/export?exclude_bots=true&days=7")
        .text()
        .unwrap();
    assert_eq!(csv.lines().count(), 2);
    assert!(!csv.contains("secret=hidden"));
}
#[test]
fn all_history_aggregates_pagination_and_long_time_buckets() {
    let s = Server::new();
    let link = s.create("example", json!({"tags":["历史"]}));
    s.seed_visits(link["id"].as_i64().unwrap(), 650, "");
    let stats = s.json("/api/links/stats/example");
    assert_eq!(stats["total_visits"], 650);
    assert_eq!(stats["visits"].as_array().unwrap().len(), 50);
    assert_eq!(
        s.json("/api/links/stats/example?skip=600&limit=100")["visits"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    assert_eq!(s.json("/api/analytics")["summary"]["total_visits"], 650);
    s.sql("DELETE FROM visits; INSERT INTO visits(short_link_id,visited_at,is_bot) VALUES (1,datetime('now'),0),(1,datetime('now','-180 days'),0),(1,datetime('now','-4000 days'),0),(1,datetime('now','-5000 days'),1),(1,datetime('now','+5 days'),0);");
    let result = s.json("/api/analytics");
    assert_eq!(result["summary"]["total_visits"], 4);
    assert_eq!(result["summary"]["period_visits"], 4);
    assert_eq!(result["tags"][0]["count"], 4);
    assert!(result["trend"].as_array().unwrap().len() <= 90);
    assert!(result["bucket_days"].as_i64().unwrap() > 1);
    assert_eq!(
        s.json("/api/analytics?exclude_bots=true")["summary"]["period_visits"],
        3
    );
    assert_eq!(
        s.json("/api/analytics?days=90")["summary"]["period_visits"],
        1
    );
    assert_eq!(
        s.get("/api/analytics/export?days=90")
            .text()
            .unwrap()
            .lines()
            .count(),
        2
    );
}
#[test]
fn timezone_boundaries_and_saved_timezone_override() {
    use chrono::{Duration, TimeZone};
    let s = Server::configured("America/New_York", 0);
    s.create("example", json!({}));
    let zone: chrono_tz::Tz = "America/New_York".parse().unwrap();
    let today = chrono::Utc::now().with_timezone(&zone).date_naive();
    let beginning = zone
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .unwrap()
        .naive_utc();
    s.sql(&format!(
        "INSERT INTO visits(short_link_id,visited_at) VALUES (1,'{}'),(1,'{}');",
        (beginning - Duration::seconds(1)).format("%Y-%m-%d %H:%M:%S%.6f"),
        beginning.format("%Y-%m-%d %H:%M:%S%.6f")
    ));
    let result = s.json("/api/analytics?days=7");
    assert_eq!(result["summary"]["today"], 1);
    assert_eq!(result["summary"]["last_7_days"], 2);
    assert_eq!(
        result["trend"].as_array().unwrap().last().unwrap()["count"],
        1
    );
    s.sql("INSERT INTO app_settings(key,value) VALUES ('timezone','Asia/Shanghai');");
    assert_eq!(
        s.put("/api/admin/settings", &json!({"timezone":"UTC"}))
            .json::<Value>()
            .unwrap()["timezone"],
        "America/New_York"
    );
    let mut snapshot = s.json("/api/admin/backup");
    snapshot["settings"]["timezone"] = json!("UTC");
    assert_eq!(s.restore(&snapshot).status(), 200);
    assert_eq!(
        s.json("/api/admin/settings")["site"]["timezone"],
        "America/New_York"
    );
}
#[test]
fn settings_apply_to_new_links_and_existing_tags_can_be_edited() {
    let s = Server::new();
    assert_eq!(s.put("/api/admin/settings",&json!({"app_name":"私人短链","base_url":"https://go.example.com/","timezone":"UTC","random_code_length":9})).status(),200);
    assert_eq!(s.json("/api/site")["app_name"], "私人短链");
    let link = s
        .post("/api/links", &json!({"original_url":"example.org"}))
        .json::<Value>()
        .unwrap();
    assert_eq!(link["short_code"].as_str().unwrap().len(), 9);
    assert!(
        link["short_url"]
            .as_str()
            .unwrap()
            .starts_with("https://go.example.com/r/")
    );
    for body in [
        json!({"timezone":"Invalid/Time"}),
        json!({"random_code_length":1}),
        json!({"base_url":"https://example.org/path"}),
        json!({"app_name":"  "}),
    ] {
        assert_eq!(s.put("/api/admin/settings", &body).status(), 422);
    }
    s.create("example", json!({"tags":["keep","remove"]}));
    for _ in 0..2 {
        let r = s.put(
            "/api/links/example",
            &json!({"original_url":"example.org","tags":["keep","new"]}),
        );
        assert_eq!(r.status(), 200);
        assert_eq!(r.json::<Value>().unwrap()["tags"], json!(["keep", "new"]));
    }
}
#[test]
fn csv_export_neutralizes_spreadsheet_formulas() {
    let s = Server::new();
    s.create(
        "example",
        json!({"title":"=HYPERLINK(\"https://evil.org\")","note":"  +SUM(1,2)","tags":["@attack"]}),
    );
    let text = s.get("/api/analytics/export?kind=links").text().unwrap();
    let mut csv = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let row = csv.records().next().unwrap().unwrap();
    assert!(row[2].starts_with("'="));
    assert!(row[3].starts_with("'+"));
    assert!(row[4].starts_with("'@"));
}
#[test]
fn concurrent_redirects_and_creates_commit_without_lost_visits() {
    let s = Server::new();
    s.create("example", json!({}));
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..12 {
            handles.push(scope.spawn(|| assert_eq!(s.get("/r/example").status(), 302)));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for n in 0..8 {
            let s = &s;
            handles.push(scope.spawn(move || {
                s.create(&format!("parallel-{n}"), json!({}));
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    let stats = s.json("/api/analytics");
    assert_eq!(stats["summary"]["total_visits"], 12);
    assert_eq!(stats["summary"]["period_visits"], 12);
    assert_eq!(stats["summary"]["links"], 9);
}
#[test]
fn trusted_proxy_chain_and_unicode_redirect() {
    for (trusted, expected) in [
        (0, "127.0.0.1"),
        (1, "2.2.2.2"),
        (2, "1.1.1.1"),
        (3, "1.1.1.1"),
    ] {
        let s = Server::configured("Asia/Shanghai", trusted);
        s.create(
            "unicode",
            json!({"original_url":"https://example.org/中文"}),
        );
        let r = s
            .client
            .get(format!("{}/r/unicode", s.base))
            .header("X-Forwarded-For", "1.1.1.1, 2.2.2.2")
            .send()
            .unwrap();
        assert_eq!(r.status(), 302);
        assert_eq!(
            r.headers()["location"],
            "https://example.org/%E4%B8%AD%E6%96%87"
        );
        assert_eq!(
            s.json("/api/links/stats/unicode")["visits"][0]["ip"],
            expected
        );
    }
}
#[test]
fn generated_openapi_versions_routes_and_spa_boundaries() {
    let s = Server::new();
    let health = s.json("/api/health");
    assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
    let document = s.json("/openapi.json");
    assert_eq!(document["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(document["components"]["schemas"]["BackupVisit"].is_object());
    assert_eq!(
        document["components"]["securitySchemes"]["bearer_auth"]["scheme"],
        "bearer"
    );
    let expected_operations = [
        ("/api/health", "get"),
        ("/api/site", "get"),
        ("/api/admin/login", "post"),
        ("/api/links", "get"),
        ("/api/links", "post"),
        ("/api/links/tags", "get"),
        ("/api/links/stats/{short_code}", "get"),
        ("/api/links/{short_code}", "put"),
        ("/api/links/{short_code}", "delete"),
        ("/r/{short_code}", "get"),
        ("/api/analytics", "get"),
        ("/api/analytics/export", "get"),
        ("/api/admin/settings", "get"),
        ("/api/admin/settings", "put"),
        ("/api/admin/backup", "get"),
        ("/api/admin/restore", "post"),
    ];
    for (path, method) in expected_operations {
        assert!(
            document["paths"][path][method].is_object(),
            "{method} {path}"
        );
    }
    let documented_operations = document["paths"]
        .as_object()
        .unwrap()
        .values()
        .map(|operations| {
            operations
                .as_object()
                .unwrap()
                .keys()
                .filter(|method| {
                    ["get", "post", "put", "delete", "patch", "head", "options"]
                        .contains(&method.as_str())
                })
                .count()
        })
        .sum::<usize>();
    // A new documented operation must also be added to the reviewed test inventory.
    assert_eq!(documented_operations, expected_operations.len());
    assert_eq!(s.get("/docs").status(), 200);
    assert_eq!(s.get("/redoc").status(), 200);
    assert_eq!(s.post("/api/links/tags", &json!({})).status(), 405);
    assert_eq!(s.get("/api/not-a-route").status(), 404);
    assert_eq!(s.get("/does-not-exist").status(), 404);
    if std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("web/dist/index.html")
        .exists()
    {
        for path in ["/", "/login", "/links", "/analytics", "/settings"] {
            let r = s.get(path);
            assert_eq!(r.status(), 200);
            assert!(r.text().unwrap().contains("<div id=\"app\">"));
        }
    }
}

#[test]
fn invalid_persisted_timestamp_returns_error_without_stopping_database_worker() {
    let s = Server::new();
    s.create("invalid", json!({}));
    s.sql("UPDATE short_links SET created_at=NULL WHERE short_code='invalid';");
    assert_eq!(s.get("/api/links").status(), 500);
    assert_eq!(s.delete("/api/links/invalid").status(), 204);
    s.create("healthy", json!({}));
    assert_eq!(s.json("/api/links")["total"], 1);
}
