use crate::*;
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
};

pub fn bind(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(v) => SqlValue::Integer(*v as i64),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                SqlValue::Integer(n)
            } else {
                SqlValue::Real(v.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(v) => SqlValue::Text(v.clone()),
        _ => SqlValue::Text(v.to_string()),
    }
}
pub fn rows(conn: &Connection, sql: &str, params: &[Value]) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut result = stmt.query(params_from_iter(params.iter().map(bind)))?;
    let mut output = Vec::new();
    while let Some(row) = result.next()? {
        output.push(row_json(row, &names)?);
    }
    Ok(output)
}
pub fn row_json(row: &rusqlite::Row, names: &[String]) -> Result<Value> {
    let mut map = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        map.insert(
            name.clone(),
            match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => json!(v),
                ValueRef::Real(v) => json!(v),
                ValueRef::Text(v) => json!(String::from_utf8_lossy(v)),
                ValueRef::Blob(_) => Value::Null,
            },
        );
    }
    Ok(Value::Object(map))
}
pub fn scalar(conn: &Connection, sql: &str, params: &[Value]) -> Result<Value> {
    Ok(rows(conn, sql, params)?
        .into_iter()
        .next()
        .and_then(|v| v.as_object().and_then(|m| m.values().next().cloned()))
        .unwrap_or(Value::Null))
}
pub fn exec(conn: &Connection, sql: &str, params: &[Value]) -> Result<usize> {
    Ok(conn
        .prepare_cached(sql)?
        .execute(params_from_iter(params.iter().map(bind)))?)
}
pub fn connect(config: &Config) -> Result<Connection> {
    let conn = Connection::open(&config.database)?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.set_prepared_statement_cache_capacity(64);
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA cache_size=-512; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    Ok(conn)
}
pub fn init(config: &Config) -> Result<()> {
    let conn = connect(config)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; BEGIN IMMEDIATE;")?;
    let tables = rows(
        &conn,
        "SELECT name FROM sqlite_master WHERE type='table'",
        &[],
    )?;
    let has = |table: &str| tables.iter().any(|r| r["name"] == table);
    let mut backfill = false;
    for (table, fields) in [
        (
            "short_links",
            vec![
                ("title", "VARCHAR(120) NOT NULL DEFAULT ''"),
                ("note", "VARCHAR(2000) NOT NULL DEFAULT ''"),
            ],
        ),
        (
            "visits",
            vec![
                ("referrer_domain", "VARCHAR(253)"),
                ("device", "VARCHAR(20) NOT NULL DEFAULT 'unknown'"),
                ("browser", "VARCHAR(30) NOT NULL DEFAULT 'unknown'"),
                ("is_bot", "INTEGER NOT NULL DEFAULT 0"),
            ],
        ),
    ] {
        if !has(table) {
            continue;
        }
        let existing = rows(&conn, &format!("PRAGMA table_info({table})"), &[])?;
        for (field, definition) in fields {
            if !existing.iter().any(|r| r["name"] == field) {
                backfill |= table == "visits" && field == "device";
                conn.execute_batch(&format!(
                    "ALTER TABLE {table} ADD COLUMN {field} {definition}"
                ))?;
            }
        }
    }
    conn.execute_batch(include_str!("schema.sql"))?;
    if backfill {
        let mut last = 0;
        loop {
            let batch = rows(
                &conn,
                "SELECT id,user_agent FROM visits WHERE id>? ORDER BY id LIMIT 500",
                &[json!(last)],
            )?;
            if batch.is_empty() {
                break;
            }
            for row in batch {
                let (device, browser, bot) =
                    crate::validation::agent(row["user_agent"].as_str().unwrap_or(""));
                exec(
                    &conn,
                    "UPDATE visits SET device=?,browser=?,is_bot=? WHERE id=?",
                    &[json!(device), json!(browser), json!(bot), row["id"].clone()],
                )?;
                last = row["id"].as_i64().unwrap();
            }
        }
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
}
pub fn tags(conn: &Connection, id: &Value) -> Result<Value> {
    Ok(Value::Array(
        rows(
            conn,
            "SELECT name FROM link_tags WHERE short_link_id=? ORDER BY name",
            std::slice::from_ref(id),
        )?
        .into_iter()
        .map(|r| r["name"].clone())
        .collect(),
    ))
}
pub fn link(conn: &Connection, code: &str) -> Result<Value> {
    rows(
        conn,
        "SELECT * FROM short_links WHERE short_code=?",
        &[json!(code)],
    )?
    .into_iter()
    .next()
    .ok_or_else(|| error(404, "短链不存在"))
}
pub fn now() -> String {
    Utc::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}
pub fn parse_date(s: &str) -> Result<chrono::NaiveDateTime> {
    use chrono::Datelike;
    let date = if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        d.naive_utc()
    } else {
        chrono::NaiveDateTime::parse_from_str(&s.replace('T', " "), "%Y-%m-%d %H:%M:%S%.f")
            .map_err(|_| error(400, "时间格式无效"))?
    };
    // Keep backups compatible with Python datetime and browser date parsing.
    // Extreme chrono dates can also overflow when converted to a site timezone.
    ensure(
        (1..=9999).contains(&date.year())
            && date.and_utc().timestamp_subsec_nanos() < 1_000_000_000,
        400,
        "时间超出支持范围",
    )?;
    Ok(date)
}
pub fn iso(s: &str, z: bool) -> Result<String> {
    let d = parse_date(s)?;
    Ok(format!(
        "{}{}",
        d.format(if d.and_utc().timestamp_subsec_micros() == 0 {
            "%Y-%m-%dT%H:%M:%S"
        } else {
            "%Y-%m-%dT%H:%M:%S%.6f"
        }),
        if z { "Z" } else { "" }
    ))
}

pub fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| error(500, "数据库字段无效"))
}

/// A bulk restore can otherwise leave a database-sized WAL allocated indefinitely.
/// Checkpoint after commit, without waiting for active reader snapshots.
/// Failure here must not turn a successful, committed restore into an HTTP error.
pub fn checkpoint(conn: &Connection) {
    let _ = conn.busy_timeout(std::time::Duration::ZERO);
    if let Err(e) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
        r.get::<_, i64>(0)
    }) {
        eprintln!("WAL checkpoint: {e}");
    }
    let _ = conn.busy_timeout(std::time::Duration::from_secs(10));
}
