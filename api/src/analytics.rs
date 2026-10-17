use crate::*;
use chrono::{Duration, NaiveDate, NaiveDateTime, TimeZone};

fn scope(i: &Input) -> (String, Vec<Value>) {
    let (mut filters, mut params) = (vec!["1"], vec![]);
    if !i.param("short_code").is_empty() {
        filters.push("s.short_code=?");
        params.push(json!(i.param("short_code")));
    }
    if !i.param("tag").is_empty() {
        filters.push("EXISTS (SELECT 1 FROM link_tags t WHERE t.short_link_id=s.id AND t.name=?)");
        params.push(json!(i.param("tag")));
    }
    (filters.join(" AND "), params)
}
fn midnight(c: &Config, day: NaiveDate) -> NaiveDateTime {
    let local = day.and_hms_opt(0, 0, 0).unwrap();
    for hour in 0..=48 {
        if let Some(value) =
            c.tz.from_local_datetime(&(local + Duration::hours(hour)))
                .earliest()
        {
            return value.naive_utc();
        }
    }
    unreachable!("timezone midnight")
}
fn boundary(c: &Config, day: NaiveDate) -> Value {
    json!(midnight(c, day).format("%Y-%m-%d %H:%M:%S").to_string())
}
fn count(conn: &Connection, sql: &str, params: &[Value]) -> Result<i64> {
    Ok(db::scalar(conn, sql, params)?.as_i64().unwrap_or(0))
}
const JOINED: &str = "FROM visits v JOIN short_links s ON s.id=v.short_link_id";
#[utoipa::path(get,path="/api/analytics",tag="analytics",params(("days"=Option<i64>,Query,minimum=0,maximum=90,example=0),("short_code"=Option<String>,Query,max_length=32),("tag"=Option<String>,Query,max_length=40),("exclude_bots"=Option<bool>,Query,example=false)),responses((status=200,body=crate::models::Analytics),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn analytics(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let days = i.number("days", 0, 0, 90)?;
    let exclude = i.boolean("exclude_bots")?;
    let (scoped, scoped_params) = scope(i);
    if !i.param("short_code").is_empty() {
        db::link(conn, i.param("short_code"))?;
    }
    let now = Utc::now();
    let today = now.with_timezone(&c.tz).date_naive();
    let mut common = format!("{scoped} AND v.visited_at<=?");
    let mut common_params = scoped_params.clone();
    common_params.push(json!(now.format("%Y-%m-%d %H:%M:%S%.6f").to_string()));
    let mut first_day = today - Duration::days(days.max(1) - 1);
    let mut bucket = 1;
    if days == 0 {
        let first = db::scalar(
            conn,
            &format!("SELECT MIN(v.visited_at) {JOINED} WHERE {common}"),
            &common_params,
        )?;
        first_day = if let Some(first) = first.as_str() {
            db::parse_date(first)?
                .and_utc()
                .with_timezone(&c.tz)
                .date_naive()
        } else {
            today
        };
        let span = (today - first_day).num_days() + 1;
        bucket = ((span + 89) / 90).max(1);
    }
    let span = (today - first_day).num_days();
    let dates: Vec<_> = (0..=span)
        .step_by(bucket as usize)
        .map(|n| first_day + Duration::days(n))
        .collect();
    if exclude {
        common.push_str(" AND v.is_bot=0");
    }
    let period = format!("{common} AND v.visited_at>=?");
    let mut period_params = common_params.clone();
    period_params.push(boundary(c, first_day));
    let mut counter_params = vec![
        boundary(c, today),
        boundary(c, today - Duration::days(6)),
        boundary(c, today - Duration::days(29)),
    ];
    counter_params.extend(common_params.clone());
    let counters=db::rows(conn,&format!("SELECT COUNT(*) AS total,COALESCE(SUM(v.visited_at>=?),0) AS today,COALESCE(SUM(v.visited_at>=?),0) AS week,COALESCE(SUM(v.visited_at>=?),0) AS month {JOINED} WHERE {common}"),&counter_params)?.remove(0);
    let mut branches = Vec::new();
    let mut trend_params = Vec::new();
    for day in &dates {
        branches.push("WHEN v.visited_at>=? AND v.visited_at<? THEN ?");
        trend_params.extend([
            boundary(c, *day),
            boundary(
                c,
                (*day + Duration::days(bucket)).min(today + Duration::days(1)),
            ),
            json!(day.to_string()),
        ]);
    }
    trend_params.extend(period_params.clone());
    let trend_rows = db::rows(
        conn,
        &format!(
            "SELECT CASE {} END AS day,COUNT(*) AS n {JOINED} WHERE {period} GROUP BY day",
            branches.join(" ")
        ),
        &trend_params,
    )?;
    let mut trend = std::collections::HashMap::new();
    for row in trend_rows {
        if let Some(day) = row["day"].as_str() {
            trend.insert(day.to_string(), row["n"].as_i64().unwrap_or(0));
        }
    }
    let breakdown = |column: &str, unknown: &str| -> Result<Vec<Value>> {
        let mut params = vec![json!(unknown)];
        params.extend(period_params.clone());
        db::rows(
            conn,
            &format!(
                "SELECT COALESCE(v.{column},?) AS name,COUNT(*) AS count {JOINED} WHERE {period} GROUP BY name ORDER BY count DESC,name LIMIT 12"
            ),
            &params,
        )
    };
    let ranked = db::rows(
        conn,
        &format!(
            "SELECT s.short_code,s.title,s.original_url,COUNT(*) AS count {JOINED} WHERE {period} GROUP BY s.id ORDER BY count DESC,s.id DESC LIMIT 10"
        ),
        &period_params,
    )?;
    let mut tag_params = period_params.clone();
    tag_params.extend(scoped_params.clone());
    let groups = db::rows(
        conn,
        &format!(
            "SELECT t.name,COUNT(*) AS link_count,COALESCE(SUM(p.n),0) AS count FROM link_tags t JOIN short_links s ON s.id=t.short_link_id LEFT JOIN (SELECT v.short_link_id,COUNT(*) AS n {JOINED} WHERE {period} GROUP BY v.short_link_id) p ON p.short_link_id=s.id WHERE {scoped} GROUP BY t.name ORDER BY count DESC,t.name"
        ),
        &tag_params,
    )?;
    let mut bot_params = scoped_params.clone();
    bot_params.extend([
        boundary(c, first_day),
        json!(now.format("%Y-%m-%d %H:%M:%S%.6f").to_string()),
    ]);
    let bots = count(
        conn,
        &format!(
            "SELECT COUNT(*) {JOINED} WHERE {scoped} AND v.visited_at>=? AND v.visited_at<=? AND v.is_bot=1"
        ),
        &bot_params,
    )?;
    Ok(Reply::json(json!({
        "timezone":c.tz_name,"days":days,"bucket_days":bucket,"range_start":first_day.to_string(),"range_end":today.to_string(),"exclude_bots":exclude,
        "summary":{"links":count(conn,&format!("SELECT COUNT(*) FROM short_links s WHERE {scoped}"),&scoped_params)?,"total_visits":counters["total"],"today":counters["today"],"last_7_days":counters["week"],"last_30_days":counters["month"],"period_visits":trend.values().sum::<i64>(),"suspected_bots":bots},
        "trend":dates.iter().map(|d|json!({"date":d.to_string(),"end_date":(*d+Duration::days(bucket-1)).min(today).to_string(),"count":trend.get(&d.to_string()).copied().unwrap_or(0)})).collect::<Vec<_>>(),
        "top_links":ranked,"referrers":breakdown("referrer_domain","直接访问 / 未知")?,"devices":breakdown("device","unknown")?,"browsers":breakdown("browser","unknown")?,"tags":groups
    })))
}
fn safe(value: &Value) -> String {
    let s = match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "1" } else { "0" }.into(),
        v => v.to_string(),
    };
    if s.trim_start().starts_with(['=', '+', '-', '@']) || s.starts_with(['\t', '\r', '\n']) {
        format!("'{s}")
    } else {
        s
    }
}
#[utoipa::path(get,path="/api/analytics/export",tag="analytics",params(("kind"=Option<String>,Query,pattern="^(visits|links)$",example="visits"),("days"=Option<i64>,Query,minimum=0,maximum=90),("short_code"=Option<String>,Query,max_length=32),("tag"=Option<String>,Query,max_length=40),("exclude_bots"=Option<bool>,Query,example=false)),responses((status=200,body=String,content_type="text/csv")),security(("bearer_auth"=[])))]
pub fn export(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let kind = if i.param("kind").is_empty() {
        "visits"
    } else {
        i.param("kind")
    };
    ensure(matches!(kind, "visits" | "links"), 422, "导出类型无效")?;
    let (mut filters, mut params) = scope(i);
    let days = i.number("days", 0, 0, 90)?;
    let exclude = i.boolean("exclude_bots")?;
    let mut output = tempfile::tempfile()?;
    output.write_all("\u{feff}".as_bytes())?;
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(&mut output);
    let csv_error = |e: csv::Error| {
        eprintln!("csv: {e}");
        error(500, "CSV 导出失败")
    };
    if kind == "links" {
        writer
            .write_record([
                "short_code",
                "original_url",
                "title",
                "note",
                "tags",
                "created_at_utc",
            ])
            .map_err(csv_error)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT s.* FROM short_links s WHERE {filters} ORDER BY s.id"
        ))?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query(rusqlite::params_from_iter(params.iter().map(db::bind)))?;
        while let Some(row) = rows.next()? {
            let link = db::row_json(row, &names)?;
            let tags = db::tags(conn, &link["id"])?;
            let tags = tags
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(";");
            writer
                .write_record([
                    safe(&link["short_code"]),
                    safe(&link["original_url"]),
                    safe(&link["title"]),
                    safe(&link["note"]),
                    safe(&json!(tags)),
                    db::iso(db::text(&link, "created_at")?, true)?,
                ])
                .map_err(csv_error)?;
        }
    } else {
        filters.push_str(" AND v.visited_at<=?");
        params.push(json!(db::now()));
        if days > 0 {
            filters.push_str(" AND v.visited_at>=?");
            params.push(boundary(
                c,
                Utc::now().with_timezone(&c.tz).date_naive() - Duration::days(days - 1),
            ));
        }
        if exclude {
            filters.push_str(" AND v.is_bot=0");
        }
        writer
            .write_record([
                "short_code",
                "visited_at_utc",
                "ip",
                "referrer_domain",
                "device",
                "browser",
                "suspected_bot",
                "user_agent",
            ])
            .map_err(csv_error)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT v.*,s.short_code {JOINED} WHERE {filters} ORDER BY v.id"
        ))?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query(rusqlite::params_from_iter(params.iter().map(db::bind)))?;
        while let Some(row) = rows.next()? {
            let v = db::row_json(row, &names)?;
            writer
                .write_record([
                    safe(&v["short_code"]),
                    db::iso(db::text(&v, "visited_at")?, true)?,
                    safe(&v["ip"]),
                    safe(&v["referrer_domain"]),
                    safe(&v["device"]),
                    safe(&v["browser"]),
                    safe(&v["is_bot"]),
                    safe(&v["user_agent"]),
                ])
                .map_err(csv_error)?;
        }
    }
    writer.flush()?;
    drop(writer);
    Reply::file(
        output,
        "text/csv; charset=utf-8",
        Some(&format!("link-withu-{kind}.csv")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_midnights_preserve_dst_days() {
        let mut c = Config::load().unwrap();
        c.tz = "America/New_York".parse().unwrap();
        let d = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(
            (midnight(&c, d(2026, 3, 9)) - midnight(&c, d(2026, 3, 8))).num_hours(),
            23
        );
        assert_eq!(
            (midnight(&c, d(2026, 11, 2)) - midnight(&c, d(2026, 11, 1))).num_hours(),
            25
        );
    }
}
