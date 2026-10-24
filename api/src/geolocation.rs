//! Persistent, deduplicated IP lookups. Network requests never run in a DB job.
use crate::*;
use futures_util::StreamExt;
use std::{net::IpAddr, time::Duration};

pub const JOIN: &str = "LEFT JOIN ip_locations g ON g.ip=v.ip";
pub const LOCATION: &str = "CASE WHEN COALESCE(g.country,'')='' THEN '未知' ELSE TRIM(g.country || ' ' || COALESCE(NULLIF(g.city,''),g.region)) END";
pub const FIELDS: &str =
    "COALESCE(g.country,'') AS country,COALESCE(g.region,'') AS region,COALESCE(g.city,'') AS city";

fn address(value: &str) -> Option<IpAddr> {
    value.parse::<IpAddr>().ok().map(|ip| ip.to_canonical())
}

fn local(ip: IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            if ip.is_private() || ip.is_loopback() || ip.is_link_local() {
                Some("内网")
            } else if a == 0
                || a >= 224
                || ip.is_documentation()
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (18..=19).contains(&b))
                || (a == 192 && b == 0 && c == 0)
            {
                Some("保留地址")
            } else {
                None
            }
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            if ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local() {
                Some("内网")
            } else if segments[0] & 0xe000 != 0x2000
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || segments[0] == 0x3fff
            {
                Some("保留地址")
            } else {
                None
            }
        }
    }
}

pub fn enqueue(conn: &Connection, ip: &str) -> Result<()> {
    let Some(parsed) = address(ip) else {
        return Ok(());
    };
    let label = local(parsed).unwrap_or("");
    db::exec(
        conn,
        "INSERT INTO ip_locations(ip,country,status,updated_at) VALUES (?,?,?,?) ON CONFLICT(ip) DO UPDATE SET status='pending',attempts=0,next_attempt=0 WHERE ip_locations.status='resolved' AND ip_locations.country NOT IN ('内网','保留地址','未知') AND ip_locations.updated_at<?",
        &[
            json!(ip),
            json!(label),
            json!(if label.is_empty() {
                "pending"
            } else {
                "resolved"
            }),
            json!(if label.is_empty() {
                0
            } else {
                Utc::now().timestamp()
            }),
            json!(Utc::now().timestamp() - 30 * 86400),
        ],
    )?;
    Ok(())
}

/// Optional fields in format-v1 backups preserve resolved locations on restore.
pub fn remember(conn: &Connection, visit: &Value) -> Result<()> {
    let Some(ip) = visit["ip"].as_str() else {
        return Ok(());
    };
    enqueue(conn, ip)?;
    if address(ip).is_some() && visit["country"].as_str().is_some_and(|s| !s.is_empty()) {
        db::exec(
            conn,
            "UPDATE ip_locations SET country=?,region=?,city=?,status='resolved',updated_at=?,next_attempt=0 WHERE ip=?",
            &[
                visit["country"].clone(),
                visit["region"].clone(),
                visit["city"].clone(),
                json!(Utc::now().timestamp()),
                json!(ip),
            ],
        )?;
    }
    Ok(())
}

fn text(data: &Value, key: &str) -> Option<String> {
    let value = data[key].as_str()?.trim();
    (value.chars().count() <= 100 && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

async fn lookup(
    client: &reqwest::Client,
    base: &url::Url,
    ip: &str,
) -> std::result::Result<Value, u64> {
    let parsed = address(ip).ok_or(3600_u64)?;
    let mut url = base.clone();
    url.path_segments_mut()
        .map_err(|_| 3600_u64)?
        .pop_if_empty()
        .push(&parsed.to_string());
    url.query_pairs_mut()
        .append_pair("lang", "zh-CN")
        .append_pair("fields", "ip,success,country,region,city");
    let response = client.get(url).send().await.map_err(|_| 0_u64)?;
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(86400)
            .clamp(60, 86400));
    }
    if !response.status().is_success() {
        return Err(0);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| 0_u64)?;
        if bytes.len() + chunk.len() > 16 * 1024 {
            return Err(0);
        }
        bytes.extend_from_slice(&chunk);
    }
    let data: Value = serde_json::from_slice(&bytes).map_err(|_| 0_u64)?;
    if data["success"] != true || data["ip"].as_str().and_then(address) != Some(parsed) {
        return Err(0);
    }
    let country = text(&data, "country")
        .filter(|s| !s.is_empty())
        .ok_or(0_u64)?;
    let region = text(&data, "region").ok_or(0_u64)?;
    let city = text(&data, "city").ok_or(0_u64)?;
    Ok(json!({"country":country,"region":region,"city":city}))
}

pub async fn run(app: Arc<App>) {
    let Some(base) = &app.config.geoip_url else {
        return;
    };
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Link-WithU/", env!("CARGO_PKG_VERSION")))
        .build()
    else {
        eprintln!("geolocation: HTTP client initialization failed");
        return;
    };
    loop {
        let job = app.writer.value(|conn| {
            let now = Utc::now().timestamp();
            let job = db::rows(conn,
                "SELECT ip,attempts FROM ip_locations WHERE status IN ('pending','failed') AND next_attempt<=? AND (SELECT retry_after FROM geolocation_state WHERE id=1)<=? ORDER BY next_attempt,ip LIMIT 1",
                &[json!(now),json!(now)])?.into_iter().next().unwrap_or(Value::Null);
            if !job.is_null() {
                db::exec(conn,"UPDATE ip_locations SET next_attempt=?,attempts=MIN(attempts+1,20) WHERE ip=?",&[json!(now+60),job["ip"].clone()])?;
            }
            Ok(job)
        }).await;
        let Ok(job) = job else {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        };
        let Some(ip) = job["ip"].as_str() else {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };
        let result = match address(ip) {
            Some(parsed) => match local(parsed) {
                Some(label) => Ok(json!({"country":label,"region":"","city":""})),
                None => lookup(&client, base, ip).await,
            },
            None => Ok(json!({"country":"未知","region":"","city":""})),
        };
        let cooldown = result.as_ref().err().copied().unwrap_or(0);
        let ip = ip.to_owned();
        let retry = cooldown
            .max(60 * 2_u64.pow(job["attempts"].as_u64().unwrap_or(0).min(10) as u32))
            .min(86400);
        let _ = app.writer.value(move |conn| {
            if cooldown > 0 {
                db::exec(conn,"UPDATE geolocation_state SET retry_after=MAX(retry_after,?) WHERE id=1",&[json!(Utc::now().timestamp()+cooldown as i64)])?;
            }
            match result {
                Ok(location) => { db::exec(conn,
                    "UPDATE ip_locations SET country=?,region=?,city=?,status='resolved',updated_at=?,next_attempt=0 WHERE ip=? AND status IN ('pending','failed')",
                    &[location["country"].clone(),location["region"].clone(),location["city"].clone(),json!(Utc::now().timestamp()),json!(ip)])?; },
                Err(_) => { db::exec(conn,
                    "UPDATE ip_locations SET status='failed',next_attempt=? WHERE ip=? AND status IN ('pending','failed')",
                    &[json!(Utc::now().timestamp()+retry as i64),json!(ip)])?; },
            }
            Ok(Value::Null)
        }).await;
        tokio::time::sleep(Duration::from_secs(cooldown.max(1))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_and_reserved_ranges_never_need_network_queries() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:192.168.1.1",
        ] {
            assert_eq!(local(address(ip).unwrap()), Some("内网"), "{ip}");
        }
        for ip in [
            "0.0.0.0",
            "100.64.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "2001:db8::1",
            "ff02::1",
            "::",
        ] {
            assert_eq!(local(address(ip).unwrap()), Some("保留地址"), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "240e:1234::1", "2606:4700:4700::1111"] {
            assert_eq!(local(address(ip).unwrap()), None, "{ip}");
        }
    }
}
