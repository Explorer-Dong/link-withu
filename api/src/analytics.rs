use crate::*;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone};

fn selected_range(i: &Input) -> Result<Option<(NaiveDate, NaiveDate)>> {
    if i.param("start_date").is_empty() && i.param("end_date").is_empty() {
        return Ok(None);
    }
    let parse = |name: &str| -> Result<NaiveDate> {
        let value = i.param(name);
        let day = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| error(422, "时间区间必须包含有效的开始和结束日期"))?;
        ensure(
            value == day.to_string() && (1..=9998).contains(&day.year()),
            422,
            "日期必须使用 YYYY-MM-DD 格式，年份范围为 0001–9998",
        )?;
        Ok(day)
    };
    let (start, end) = (parse("start_date")?, parse("end_date")?);
    ensure(start <= end, 422, "开始日期不能晚于结束日期")?;
    ensure(
        i.number("days", 0, 0, 90)? == 0,
        422,
        "时间区间不能与天数筛选同时使用",
    )?;
    Ok(Some((start, end)))
}

pub fn visit_filter(c: &Config, i: &Input) -> Result<(String, Vec<Value>)> {
    let (mut filter, mut params) = scope(i);
    filter.push_str(" AND v.visited_at<=?");
    params.push(json!(db::now()));
    let days = i.number("days", 0, 0, 90)?;
    if let Some((start, end)) = selected_range(i)? {
        filter.push_str(" AND v.visited_at>=? AND v.visited_at<?");
        params.extend([boundary(c, start), boundary(c, end + Duration::days(1))]);
    } else if days > 0 {
        filter.push_str(" AND v.visited_at>=?");
        params.push(boundary(
            c,
            Utc::now().with_timezone(&c.tz).date_naive() - Duration::days(days - 1),
        ));
    }
    if i.boolean("exclude_bots")? {
        filter.push_str(" AND v.is_bot=0");
    }
    Ok((filter, params))
}

fn group_query(c: &Config, i: &Input, kind: &str) -> Result<(String, Vec<Value>)> {
    let (period, params) = visit_filter(c, i)?;
    let query = match kind {
        "referrers" | "devices" | "browsers" => {
            let (column, unknown) = match kind {
                "referrers" => ("referrer_domain", "直接访问 / 未知"),
                "devices" => ("device", "unknown"),
                _ => ("browser", "unknown"),
            };
            format!(
                "SELECT COALESCE(v.{column},'{unknown}') AS name,COUNT(*) AS count {JOINED} WHERE {period} GROUP BY name ORDER BY count DESC,name"
            )
        }
        "locations" => format!(
            "SELECT {} AS name,COUNT(*) AS count {JOINED} {} WHERE {period} GROUP BY name ORDER BY count DESC,name",
            crate::geolocation::LOCATION,
            crate::geolocation::JOIN
        ),
        "top_links" => format!(
            "SELECT s.short_code,s.title,s.original_url,COUNT(*) AS count {JOINED} WHERE {period} GROUP BY s.id ORDER BY count DESC,s.id DESC"
        ),
        "tags" => {
            let (scoped, scoped_params) = scope(i);
            let mut params = params;
            params.extend(scoped_params);
            return Ok((
                format!(
                    "SELECT t.name,COUNT(*) AS link_count,COALESCE(SUM(p.n),0) AS count FROM link_tags t JOIN short_links s ON s.id=t.short_link_id LEFT JOIN (SELECT v.short_link_id,COUNT(*) AS n {JOINED} WHERE {period} GROUP BY v.short_link_id) p ON p.short_link_id=s.id WHERE {scoped} GROUP BY t.name ORDER BY count DESC,t.name"
                ),
                params,
            ));
        }
        _ => return Err(error(422, "统计分类无效")),
    };
    Ok((query, params))
}

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
            if hour == 0 {
                return value.naive_utc();
            }
            // A midnight gap can end between whole hours, including at a second.
            let mut low = (hour - 1) * 3600;
            let mut high = hour * 3600;
            while low < high {
                let middle = low + (high - low) / 2;
                if c.tz
                    .from_local_datetime(&(local + Duration::seconds(middle)))
                    .earliest()
                    .is_some()
                {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            return c
                .tz
                .from_local_datetime(&(local + Duration::seconds(high)))
                .earliest()
                .unwrap()
                .naive_utc();
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
#[utoipa::path(get,path="/api/analytics",tag="analytics",params(("start_date"=Option<String>,Query,format=Date,description="开始日期，按站点时区，含当天；与 end_date 配对使用"),("end_date"=Option<String>,Query,format=Date,description="结束日期，按站点时区，含当天"),("days"=Option<i64>,Query,minimum=0,maximum=90,example=0),("short_code"=Option<String>,Query,max_length=32),("tag"=Option<String>,Query,max_length=40),("exclude_bots"=Option<bool>,Query,example=false)),responses((status=200,body=crate::models::Analytics),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
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
    let range = selected_range(i)?;
    let mut first_day = today - Duration::days(days.max(1) - 1);
    let mut last_day = today;
    let mut bucket = 1;
    if let Some((start, end)) = range {
        first_day = start;
        last_day = end;
        bucket = (((end - start).num_days() + 366) / 366).max(1);
    } else if days == 0 {
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
    let span = (last_day - first_day).num_days();
    let dates: Vec<_> = (0..=span)
        .step_by(bucket as usize)
        .map(|n| first_day + Duration::days(n))
        .collect();
    if exclude {
        common.push_str(" AND v.is_bot=0");
    }
    let period = format!("{common} AND v.visited_at>=? AND v.visited_at<?");
    let mut period_params = common_params.clone();
    period_params.push(boundary(c, first_day));
    period_params.push(boundary(c, last_day + Duration::days(1)));
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
                (*day + Duration::days(bucket)).min(last_day + Duration::days(1)),
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
    let preview = |kind: &str| -> Result<Vec<Value>> {
        let (query, params) = group_query(c, i, kind)?;
        db::rows(conn, &format!("{query} LIMIT 3"), &params)
    };
    let mut bot_params = scoped_params.clone();
    bot_params.extend([
        boundary(c, first_day),
        json!(now.format("%Y-%m-%d %H:%M:%S%.6f").to_string()),
        boundary(c, last_day + Duration::days(1)),
    ]);
    let bots = count(
        conn,
        &format!(
            "SELECT COUNT(*) {JOINED} WHERE {scoped} AND v.visited_at>=? AND v.visited_at<=? AND v.visited_at<? AND v.is_bot=1"
        ),
        &bot_params,
    )?;
    Ok(Reply::json(json!({
        "timezone":c.tz_name,"days":days,"bucket_days":bucket,"range_start":first_day.to_string(),"range_end":last_day.to_string(),"exclude_bots":exclude,
        "summary":{"active_links":count(conn,&format!("SELECT COUNT(DISTINCT s.id) {JOINED} WHERE {period}"),&period_params)?,"links":count(conn,&format!("SELECT COUNT(*) FROM short_links s WHERE {scoped}"),&scoped_params)?,"total_visits":counters["total"],"today":counters["today"],"last_7_days":counters["week"],"last_30_days":counters["month"],"period_visits":trend.values().sum::<i64>(),"suspected_bots":bots,"pending_locations":if c.geoip_url.is_some() { count(conn,&format!("SELECT COUNT(*) {JOINED} {} WHERE {period} AND g.status='pending' AND (SELECT retry_after FROM geolocation_state WHERE id=1)<=CAST(strftime('%s','now') AS INTEGER)",crate::geolocation::JOIN),&period_params)? } else { 0 }},
        "trend":dates.iter().map(|d|json!({"date":d.to_string(),"end_date":(*d+Duration::days(bucket-1)).min(last_day).to_string(),"count":trend.get(&d.to_string()).copied().unwrap_or(0)})).collect::<Vec<_>>(),
        "top_links":preview("top_links")?,"referrers":preview("referrers")?,"devices":preview("devices")?,"browsers":preview("browsers")?,"tags":preview("tags")?,"locations":preview("locations")?
    })))
}
#[utoipa::path(get,path="/api/analytics/breakdown",tag="analytics",params(("kind"=String,Query,pattern="^(referrers|devices|browsers|top_links|tags|locations)$",example="locations"),("skip"=Option<i64>,Query,minimum=0),("limit"=Option<i64>,Query,minimum=1,maximum=100),("start_date"=Option<String>,Query,format=Date),("end_date"=Option<String>,Query,format=Date),("days"=Option<i64>,Query,minimum=0,maximum=90),("short_code"=Option<String>,Query,max_length=32),("tag"=Option<String>,Query,max_length=40),("exclude_bots"=Option<bool>,Query)),responses((status=200,body=crate::models::BreakdownPage),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn breakdown(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    if !i.param("short_code").is_empty() {
        db::link(conn, i.param("short_code"))?;
    }
    let skip = i.number("skip", 0, 0, i64::MAX)?;
    let limit = i.number("limit", 6, 1, 100)?;
    let (query, mut params) = group_query(c, i, i.param("kind"))?;
    let total = count(conn, &format!("SELECT COUNT(*) FROM ({query})"), &params)?;
    params.extend([json!(limit), json!(skip)]);
    let items = db::rows(conn, &format!("{query} LIMIT ? OFFSET ?"), &params)?;
    Ok(Reply::json(json!({"total":total,"items":items})))
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
#[utoipa::path(get,path="/api/analytics/export",tag="analytics",params(("start_date"=Option<String>,Query,format=Date),("end_date"=Option<String>,Query,format=Date),("kind"=Option<String>,Query,pattern="^(visits|links)$",example="visits"),("days"=Option<i64>,Query,minimum=0,maximum=90),("short_code"=Option<String>,Query,max_length=32),("tag"=Option<String>,Query,max_length=40),("exclude_bots"=Option<bool>,Query,example=false)),responses((status=200,body=String,content_type="text/csv")),security(("bearer_auth"=[])))]
pub fn export(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let kind = if i.param("kind").is_empty() {
        "visits"
    } else {
        i.param("kind")
    };
    ensure(matches!(kind, "visits" | "links"), 422, "导出类型无效")?;
    let (mut filters, mut params) = scope(i);
    i.number("days", 0, 0, 90)?;
    i.boolean("exclude_bots")?;
    selected_range(i)?;
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
        (filters, params) = visit_filter(c, i)?;
        writer
            .write_record([
                "short_code",
                "visited_at_utc",
                "ip",
                "location",
                "referrer_domain",
                "device",
                "browser",
                "suspected_bot",
                "user_agent",
            ])
            .map_err(csv_error)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT v.*,s.short_code,{} AS location {JOINED} {} WHERE {filters} ORDER BY v.id",
            crate::geolocation::LOCATION,
            crate::geolocation::JOIN
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
                    safe(&v["location"]),
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

    #[test]
    fn local_midnights_handle_partial_and_skipped_days() {
        let mut c = Config::load().unwrap();
        for (zone, date, expected) in [
            ("Asia/Kathmandu", (1986, 1, 1), "1985-12-31 18:30:00"),
            ("Africa/Monrovia", (1972, 1, 7), "1972-01-07 00:44:30"),
            ("Pacific/Apia", (2011, 12, 30), "2011-12-30 10:00:00"),
        ] {
            c.tz = zone.parse().unwrap();
            let day = NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap();
            assert_eq!(midnight(&c, day).to_string(), expected, "{zone}");
        }
    }
}
