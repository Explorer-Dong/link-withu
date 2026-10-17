use crate::*;
use std::collections::HashSet;

pub fn text<'a>(v: &'a Value, key: &str, default: &'a str, max: usize) -> Result<&'a str> {
    let s = match v.get(key) {
        None => default,
        Some(Value::String(s)) => s,
        _ => return Err(error(422, "字段类型无效")),
    };
    ensure(s.chars().count() <= max, 422, "字段过长")?;
    Ok(s)
}
pub fn url(value: &str) -> Result<String> {
    let value = value.trim();
    let authority = value.split(['/', '?', '#']).next().unwrap_or("");
    let port = authority
        .rsplit_once(':')
        .is_some_and(|(_, port)| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()));
    let scheme = value.split_once(':').is_some_and(|(s, _)| {
        !s.is_empty()
            && s.starts_with(|c: char| c.is_ascii_alphabetic())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    let value = if port || !scheme {
        format!("https://{value}")
    } else {
        value.to_string()
    };
    let parsed =
        url::Url::parse(&value).map_err(|_| error(422, "请输入有效的 HTTP / HTTPS 地址"))?;
    // WHATWG URL parsing accepts forms such as `http:example.org` and repairs
    // extra slashes. Require the explicit authority used by stored URLs.
    let authority = value
        .split_once("://")
        .is_some_and(|(_, rest)| !rest.is_empty() && !rest.starts_with(['/', '\\']));
    ensure(
        authority
            && matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && !value.chars().any(|c| c.is_whitespace() || c < ' '),
        422,
        "请输入有效的 HTTP / HTTPS 地址，不支持带账号密码的地址",
    )?;
    ensure(
        value.chars().count() <= 2048,
        422,
        "链接不能超过 2048 个字符",
    )?;
    Ok(value)
}
pub fn fields(v: &Value) -> Result<Value> {
    ensure(v.is_object(), 422, "对象格式无效")?;
    let original = url(text(v, "original_url", "", 2048)?)?;
    let title = text(v, "title", "", 120)?.trim();
    let note = text(v, "note", "", 2000)?.trim();
    let empty = vec![];
    let tags = match v.get("tags") {
        None => &empty,
        Some(Value::Array(tags)) => tags,
        _ => return Err(error(422, "标签类型无效")),
    };
    ensure(tags.len() <= 20, 422, "标签过多")?;
    let mut unique = HashSet::new();
    let mut result = Vec::new();
    for tag in tags {
        let tag = tag
            .as_str()
            .ok_or_else(|| error(422, "标签类型无效"))?
            .trim()
            .strip_prefix('#')
            .unwrap_or(tag.as_str().unwrap().trim());
        if tag.is_empty() {
            continue;
        }
        ensure(
            tag.chars().count() <= 40 && !tag.chars().any(|c| c < ' '),
            422,
            "标签不能超过 40 个字符或包含控制字符",
        )?;
        if unique.insert(tag) {
            result.push(tag);
        }
    }
    Ok(json!({"original_url":original,"title":title,"note":note,"tags":result}))
}
pub fn code(s: &str, legacy: bool) -> Result<()> {
    ensure(
        !s.is_empty()
            && s.chars().count() <= 32
            && s.chars().all(|c| {
                c == '-'
                    || c == '_'
                    || if legacy {
                        c.is_alphanumeric()
                    } else {
                        c.is_ascii_alphanumeric()
                    }
            }),
        422,
        "短码需为 1–32 位英文字母、数字、横线或下划线",
    )
}
pub fn site(v: &Value) -> Result<Value> {
    ensure(v.is_object(), 422, "站点配置无效")?;
    let app_name = text(v, "app_name", "Link WithU", 60)?.trim();
    ensure(!app_name.is_empty(), 422, "站点名称不能为空")?;
    let base_url = url(text(v, "base_url", "http://localhost:5201", 2048)?)?
        .trim_end_matches('/')
        .to_string();
    let authority = base_url
        .split_once("://")
        .ok_or_else(|| error(422, "公开地址格式无效"))?
        .1;
    ensure(
        !authority.contains(['/', '?', '#']),
        422,
        "公开地址只包含协议、域名和端口",
    )?;
    let timezone = text(v, "timezone", "Asia/Shanghai", 100)?;
    ensure(
        timezone.parse::<chrono_tz::Tz>().is_ok(),
        422,
        "请输入有效时区，例如 Asia/Shanghai",
    )?;
    let length = match v.get("random_code_length") {
        None => 6,
        Some(v) => v
            .as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(|| error(422, "短码长度无效"))?,
    };
    ensure((4..=16).contains(&length), 422, "短码长度需为 4–16")?;
    Ok(
        json!({"app_name":app_name,"base_url":base_url,"timezone":timezone,"random_code_length":length}),
    )
}
pub fn get_site(conn: &Connection, c: &Config) -> Result<Value> {
    let mut value = json!({"app_name":c.app_name,"base_url":c.base_url,"random_code_length":c.code_length,"timezone":c.tz_name});
    for row in db::rows(conn, "SELECT key,value FROM app_settings", &[])? {
        if let Some(key) = row["key"].as_str() {
            value[key] = row["value"].clone();
        }
    }
    value["timezone"] = json!(c.tz_name);
    site(&value)
}
pub fn save_site(conn: &Connection, v: &Value) -> Result<()> {
    for key in ["app_name", "base_url", "random_code_length"] {
        let text = if let Some(s) = v[key].as_str() {
            s.to_string()
        } else {
            v[key].to_string()
        };
        db::exec(
            conn,
            "INSERT INTO app_settings(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            &[json!(key), json!(text)],
        )?;
    }
    Ok(())
}
pub fn agent(agent: &str) -> (&'static str, &'static str, bool) {
    let a = agent.to_ascii_lowercase();
    let has = |patterns: &[&str]| patterns.iter().any(|p| a.contains(p));
    let device = if a.is_empty() {
        "unknown"
    } else if has(&["ipad", "tablet"])
        || a.find("android")
            .is_some_and(|i| !a[i..].contains("mobile"))
    {
        "tablet"
    } else if has(&["mobile", "iphone", "ipod", "android"]) {
        "mobile"
    } else if has(&["windows", "macintosh", "x11", "linux", "cros"]) {
        "desktop"
    } else {
        "unknown"
    };
    let browser = if has(&["edg/", "edge/", "edga/", "edgios/"]) {
        "Edge"
    } else if has(&["opr/", "opera/"]) {
        "Opera"
    } else if has(&["firefox/", "fxios/"]) {
        "Firefox"
    } else if has(&["chrome/", "crios/"]) {
        "Chrome"
    } else if a
        .find("version/")
        .is_some_and(|i| a[i..].contains("safari/"))
    {
        "Safari"
    } else {
        "unknown"
    };
    let bot = has(&[
        "bot",
        "crawler",
        "spider",
        "slurp",
        "preview",
        "facebookexternalhit",
        "headless",
        "curl/",
        "wget/",
        "python-requests",
        "httpx/",
        "lighthouse",
        "monitor",
        "uptime",
    ]);
    (device, browser, bot)
}
