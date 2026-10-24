mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct GeoProvider {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    mode: Arc<AtomicUsize>,
}
impl GeoProvider {
    fn new() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mode = Arc::new(AtomicUsize::new(0));
        let server = Self {
            url: format!("http://{}/", listener.local_addr().unwrap()),
            requests: requests.clone(),
            mode: mode.clone(),
        };
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            while Arc::strong_count(&mode) > 1 {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut buffer = [0; 8192];
                let n = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..n]);
                let target = request.split_whitespace().nth(1).unwrap_or("").to_owned();
                let path = url::Url::parse(&format!("http://localhost{target}")).unwrap();
                let ip = percent_encoding::percent_decode_str(path.path().trim_start_matches('/'))
                    .decode_utf8()
                    .unwrap();
                requests.lock().unwrap().push(target.clone());
                let (status, body, extra) = match mode.load(Ordering::Relaxed) {
                    1 => (429, json!({"success":false}), "Retry-After: 120\r\n"),
                    2 => (200, json!({"ip":ip,"success":false}), ""),
                    3 => (
                        200,
                        json!({"ip":"9.9.9.9","success":true,"country":"伪造位置","region":"","city":"错误"}),
                        "",
                    ),
                    4 => {
                        std::thread::sleep(Duration::from_secs(6));
                        (200, json!({}), "")
                    }
                    _ => {
                        let (country, region, city) = match ip.as_ref() {
                            "8.8.8.8" => ("美国", "加利福尼亚州", "洛杉矶"),
                            "1.1.1.1" => ("中国", "江苏省", "苏州"),
                            _ => ("英国", "英格兰", "伦敦"),
                        };
                        (
                            200,
                            json!({"ip":ip,"success":true,"country":country,"region":region,"city":city}),
                            "",
                        )
                    }
                };
                let body = body.to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
            }
        });
        server
    }
    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "background lookup timed out");
        std::thread::sleep(Duration::from_millis(30));
    }
}
fn redirect_from(s: &Server, ip: &str) {
    assert_eq!(
        s.client
            .get(format!("{}/r/geo", s.base))
            .header("X-Forwarded-For", ip)
            .send()
            .unwrap()
            .status(),
        302
    );
}

#[test]
fn online_locations_are_cached_grouped_exported_and_restored() {
    let provider = GeoProvider::new();
    let s = Server::with_env(&[
        ("GEOIP_API_URL", &provider.url),
        ("TRUSTED_PROXY_COUNT", "1"),
    ]);
    s.create("geo", json!({"tags":["geo"]}));
    for ip in [
        "8.8.8.8",
        "8.8.8.8",
        "1.1.1.1",
        "2606:4700:4700::1111",
        "192.168.1.1",
        "203.0.113.1",
    ] {
        redirect_from(&s, ip);
    }
    wait_for(|| s.json("/api/analytics")["summary"]["pending_locations"] == 0);
    assert_eq!(provider.count(), 3, "one lookup per distinct public IP");
    let detail = s.json("/api/links/stats/geo");
    for (ip, location) in [
        ("8.8.8.8", "美国 洛杉矶"),
        ("1.1.1.1", "中国 苏州"),
        ("2606:4700:4700::1111", "英国 伦敦"),
        ("192.168.1.1", "内网"),
        ("203.0.113.1", "保留地址"),
    ] {
        let row = detail["visits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["ip"] == ip)
            .unwrap();
        assert_eq!(row["location"], location);
    }
    let stats = s.json("/api/analytics?tag=geo");
    assert_eq!(stats["locations"].as_array().unwrap().len(), 3);
    assert_eq!(
        stats["locations"][0],
        json!({"name":"美国 洛杉矶","count":2})
    );
    let groups = s.json("/api/analytics/breakdown?kind=locations&limit=2&skip=2&short_code=geo");
    assert_eq!(groups["total"], 5);
    assert_eq!(groups["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        s.json("/api/analytics/breakdown?kind=locations&tag=missing")["total"],
        0
    );
    let text = s.get("/api/analytics/export?kind=visits").text().unwrap();
    assert!(text.starts_with('\u{feff}'));
    assert!(text.contains("ip,location,referrer_domain"));
    assert!(text.contains("美国 洛杉矶"));
    let backup = s.json("/api/admin/backup");
    assert_eq!(backup["visits"][0]["city"], "洛杉矶");
    assert_eq!(s.restore(&backup).status(), 200);
    assert_eq!(s.json("/api/admin/backup")["visits"], backup["visits"]);
    redirect_from(&s, "8.8.8.8");
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(provider.count(), 3);
    let requests = provider.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .all(|target| target.contains("lang=zh-CN") && !target.contains("geo"))
    );
    drop(requests);
    let mut legacy = backup.clone();
    for visit in legacy["visits"].as_array_mut().unwrap() {
        let fields = visit.as_object_mut().unwrap();
        for name in ["country", "region", "city"] {
            fields.remove(name);
        }
    }
    assert_eq!(s.restore(&legacy).status(), 200);
    wait_for(|| s.json("/api/analytics")["summary"]["pending_locations"] == 0);
    assert_eq!(
        provider.count(),
        6,
        "legacy visits must be enriched after restore"
    );
}

#[test]
fn failed_lookups_retry_without_delaying_redirects_or_trusting_invalid_results() {
    for mode in [2, 3, 4] {
        let provider = GeoProvider::new();
        provider.mode.store(mode, Ordering::Relaxed);
        let s = Server::with_env(&[
            ("GEOIP_API_URL", &provider.url),
            ("TRUSTED_PROXY_COUNT", "1"),
        ]);
        s.create("geo", json!({}));
        let start = Instant::now();
        redirect_from(&s, "8.8.8.8");
        assert!(start.elapsed() < Duration::from_secs(1));
        wait_for(|| {
            let conn = rusqlite::Connection::open(&s.database).unwrap();
            conn.query_row(
                "SELECT status='failed' FROM ip_locations WHERE ip='8.8.8.8'",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap()
        });
        assert_eq!(
            s.json("/api/links/stats/geo")["visits"][0]["location"],
            "未知"
        );
        provider.mode.store(0, Ordering::Relaxed);
        s.sql("UPDATE ip_locations SET next_attempt=0 WHERE ip='8.8.8.8'");
        wait_for(|| s.json("/api/links/stats/geo")["visits"][0]["location"] == "美国 洛杉矶");
        assert_eq!(provider.count(), 2);
    }
}

#[test]
fn rate_limits_pause_the_whole_queue_and_cache_survives_restarts() {
    let provider = GeoProvider::new();
    provider.mode.store(1, Ordering::Relaxed);
    let mut s = Server::with_env(&[
        ("GEOIP_API_URL", &provider.url),
        ("TRUSTED_PROXY_COUNT", "1"),
    ]);
    s.create("geo", json!({}));
    redirect_from(&s, "8.8.8.8");
    wait_for(|| provider.count() == 1);
    wait_for(|| {
        rusqlite::Connection::open(&s.database)
            .unwrap()
            .query_row(
                "SELECT retry_after>0 FROM geolocation_state WHERE id=1",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap()
    });
    redirect_from(&s, "1.1.1.1");
    std::thread::sleep(Duration::from_millis(2200));
    assert_eq!(
        provider.count(),
        1,
        "429 must pause all IPs, including new ones"
    );
    let conn = rusqlite::Connection::open(&s.database).unwrap();
    let retry: i64 = conn
        .query_row(
            "SELECT next_attempt FROM ip_locations WHERE ip='8.8.8.8'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let paused: i64 = conn
        .query_row(
            "SELECT retry_after FROM geolocation_state WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(paused >= chrono::Utc::now().timestamp() + 110);
    assert!(retry >= paused && retry <= paused + 1);
    s.sql(
        "UPDATE ip_locations SET country='美国',city='洛杉矶',status='resolved' WHERE ip='8.8.8.8'",
    );
    assert!(s.stop().success());
    let directory = std::mem::replace(&mut s.directory, tempfile::tempdir().unwrap());
    let resumed = Server::from_directory(directory, "Asia/Shanghai", 1);
    assert_eq!(
        resumed.json("/api/links/stats/geo")["visits"][1]["location"],
        "美国 洛杉矶"
    );
}

#[test]
fn custom_dates_share_timezone_boundaries_across_summary_details_and_export() {
    let s = Server::configured("America/New_York", 0);
    s.create("dates", json!({"tags":["chosen"]}));
    s.sql(
        "INSERT INTO visits(short_link_id,visited_at,is_bot,referrer_domain,device,browser) VALUES
        (1,'2026-03-08 04:59:59',0,'before.example','desktop','Firefox'),
        (1,'2026-03-08 05:00:00',0,'selected.example','desktop','Firefox'),
        (1,'2026-03-09 03:59:59',0,'selected.example','mobile','Safari'),
        (1,'2026-03-08 12:00:00',1,'bot.example','unknown','Bot'),
        (1,'2026-03-09 04:00:00',0,'after.example','tablet','Chrome'),
        (1,'2099-01-01 00:00:00',0,'future.example','desktop','Firefox');",
    );
    let range = "start_date=2026-03-08&end_date=2026-03-08&tag=chosen";
    let stats = s.json(&format!("/api/analytics?{range}"));
    assert_eq!(stats["summary"]["period_visits"], 3);
    assert_eq!(stats["summary"]["suspected_bots"], 1);
    assert_eq!(stats["summary"]["active_links"], 1);
    assert_eq!(
        stats["trend"],
        json!([{"date":"2026-03-08","end_date":"2026-03-08","count":3}])
    );
    let clean = s.json(&format!("/api/analytics?{range}&exclude_bots=true"));
    assert_eq!(clean["summary"]["period_visits"], 2);
    let detail = s.json(&format!("/api/links/stats/dates?{range}&exclude_bots=true"));
    assert_eq!(detail["total_visits"], 2);
    assert_eq!(detail["visits"].as_array().unwrap().len(), 2);
    let groups = s.json(&format!(
        "/api/analytics/breakdown?kind=referrers&{range}&exclude_bots=true"
    ));
    assert_eq!(
        groups,
        json!({"total":1,"items":[{"name":"selected.example","count":2}]})
    );
    let csv = s
        .get(&format!("/api/analytics/export?{range}&exclude_bots=true"))
        .text()
        .unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert_eq!(csv.lines().count(), 3);
    assert!(!csv.contains("before.example"));
    assert!(!csv.contains("after.example"));
    assert!(!csv.contains("bot.example"));
    let empty = s.json("/api/analytics?start_date=2099-01-01&end_date=2099-01-01");
    assert_eq!(empty["summary"]["period_visits"], 0);
    assert_eq!(empty["trend"][0]["count"], 0);
    let long = s.json("/api/analytics?start_date=2020-01-01&end_date=2030-01-01");
    assert!(long["trend"].as_array().unwrap().len() <= 366);
    assert_eq!(long["summary"]["period_visits"], 5);
}

#[test]
fn partial_midnight_gaps_do_not_drop_visits_from_the_selected_day() {
    for (zone, day, before, first, later) in [
        (
            "Asia/Kathmandu",
            "1986-01-01",
            "1985-12-31 18:29:59",
            "1985-12-31 18:30:00",
            "1985-12-31 19:00:00",
        ),
        (
            "Africa/Monrovia",
            "1972-01-07",
            "1972-01-07 00:44:29",
            "1972-01-07 00:44:30",
            "1972-01-07 01:00:00",
        ),
    ] {
        let s = Server::configured(zone, 0);
        s.create("midnight", json!({}));
        s.sql(&format!("INSERT INTO visits(short_link_id,visited_at,is_bot) VALUES (1,'{before}',0),(1,'{first}',0),(1,'{later}',0)"));
        let range = format!("start_date={day}&end_date={day}");
        let stats = s.json(&format!("/api/analytics?{range}"));
        assert_eq!(stats["summary"]["period_visits"], 2, "{zone}");
        assert_eq!(stats["trend"][0]["count"], 2, "{zone}");
        assert_eq!(
            s.json(&format!("/api/links/stats/midnight?{range}"))["total_visits"],
            2,
            "{zone}"
        );
        assert_eq!(
            s.json(&format!("/api/analytics/breakdown?kind=top_links&{range}"))["items"][0]["count"],
            2,
            "{zone}"
        );
        let csv = s
            .get(&format!("/api/analytics/export?{range}"))
            .text()
            .unwrap();
        assert_eq!(csv.lines().count(), 3, "{zone}");
    }
}

#[test]
fn pagination_includes_every_group_beyond_previous_ranking_limits() {
    let s = Server::new();
    for n in 1..=15 {
        let link = s.create(
            &format!("rank-{n:02}"),
            json!({"tags":[format!("tag-{n:02}")]}),
        );
        s.sql(&format!("INSERT INTO visits(short_link_id,visited_at,referrer_domain,device,browser,is_bot) VALUES ({},'2026-01-01 12:00:00','source-{n:02}.example','desktop','Browser-{n:02}',0)", link["id"]));
    }
    let preview = s.json("/api/analytics");
    for kind in ["referrers", "browsers", "top_links", "tags"] {
        assert_eq!(preview[kind].as_array().unwrap().len(), 3);
        let first = s.json(&format!("/api/analytics/breakdown?kind={kind}"));
        let last = s.json(&format!(
            "/api/analytics/breakdown?kind={kind}&limit=6&skip=12"
        ));
        assert_eq!(first["total"], 15);
        assert_eq!(first["items"].as_array().unwrap().len(), 6);
        assert_eq!(last["items"].as_array().unwrap().len(), 3);
        assert!(
            !first["items"]
                .as_array()
                .unwrap()
                .contains(&last["items"][0])
        );
        let filtered = s.json(&format!("/api/analytics/breakdown?kind={kind}&tag=tag-01&start_date=2026-01-01&end_date=2026-01-01"));
        assert_eq!(filtered["total"], 1);
        let beyond = s.json(&format!("/api/analytics/breakdown?kind={kind}&skip=100"));
        assert!(beyond["items"].as_array().unwrap().is_empty());
    }
    assert_eq!(
        s.json("/api/analytics/breakdown?kind=devices")["items"],
        json!([{"name":"desktop","count":15}])
    );
}

#[test]
fn malformed_dates_and_pagination_are_rejected() {
    let s = Server::new();
    for query in [
        "start_date=2026-01-01",
        "end_date=2026-01-01",
        "start_date=2026-02-30&end_date=2026-03-01",
        "start_date=2026-03-02&end_date=2026-03-01",
        "start_date=0000-01-01&end_date=2026-01-01",
        "start_date=2026-1-1&end_date=2026-01-02",
        "start_date=2026-01-01&end_date=2026-01-02&days=7",
    ] {
        for path in [
            "/api/analytics",
            "/api/analytics/export",
            "/api/analytics/breakdown?kind=referrers&unused=1",
        ] {
            let separator = if path.contains('?') { '&' } else { '?' };
            assert_eq!(
                s.get(&format!("{path}{separator}{query}")).status(),
                422,
                "{path} {query}"
            );
        }
    }
    for query in [
        "kind=unknown",
        "kind=tags&skip=-1",
        "kind=devices&limit=101",
        "kind=browsers&limit=0",
    ] {
        assert_eq!(
            s.get(&format!("/api/analytics/breakdown?{query}")).status(),
            422
        );
    }
    assert_eq!(
        s.request(
            reqwest::Method::GET,
            "/api/analytics/breakdown?kind=tags",
            None,
            false
        )
        .status(),
        401
    );
    let document: Value = s.json("/openapi.json");
    assert!(document["paths"]["/api/analytics/breakdown"]["get"].is_object());
}
