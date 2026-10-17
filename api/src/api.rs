use crate::*;
use rand::{Rng, distr::Alphanumeric};
use subtle::ConstantTimeEq;
const URL_ESCAPES: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub fn response(conn: &Connection, mut link: Value, site: &Value, count: Value) -> Result<Value> {
    link["tags"] = db::tags(conn, &link["id"])?;
    link["short_url"] = json!(format!(
        "{}/r/{}",
        site["base_url"].as_str().unwrap(),
        db::text(&link, "short_code")?
    ));
    link["created_at"] = json!(db::iso(db::text(&link, "created_at")?, false)?);
    link["is_custom"] = json!(link["is_custom"].as_i64().unwrap_or(0) != 0);
    link["visit_count"] = count;
    if let Some(map) = link.as_object_mut() {
        map.remove("n");
    }
    Ok(link)
}
fn save_tags(conn: &Connection, id: &Value, tags: &Value) -> Result<()> {
    for tag in tags.as_array().unwrap() {
        db::exec(
            conn,
            "INSERT INTO link_tags(short_link_id,name) VALUES (?,?)",
            &[id.clone(), tag.clone()],
        )?;
    }
    Ok(())
}
fn limited_query(i: &Input) -> Result<()> {
    ensure(
        i.param("short_code").chars().count() <= 32 && i.param("tag").chars().count() <= 40,
        422,
        "参数过长",
    )
}
pub fn dispatch(conn: &Connection, c: &Config, i: Input) -> Result<Reply> {
    match (i.method.as_str(), i.path.as_str()) {
        ("POST", "/api/admin/login") => login(conn, c, &i),
        ("GET", "/api/site") => site(conn, c, &i),
        ("GET", "/api/admin/settings") => read_settings(conn, c, &i),
        ("PUT", "/api/admin/settings") => update_settings(conn, c, &i),
        ("POST", "/api/links") => create(conn, c, &i),
        ("GET", "/api/links") => list(conn, c, &i),
        ("GET", "/api/links/tags") => tags(conn, c, &i),
        ("GET", "/api/analytics") => {
            limited_query(&i)?;
            crate::analytics::analytics(conn, c, &i)
        }
        ("GET", "/api/analytics/export") => {
            limited_query(&i)?;
            crate::analytics::export(conn, c, &i)
        }
        ("GET", "/api/admin/backup") => backup(conn, c),
        ("GET", p) if p.starts_with("/r/") => redirect(conn, c, &i),
        ("GET", p) if p.starts_with("/api/links/stats/") => visits(conn, c, &i),
        ("PUT", p) if p.starts_with("/api/links/") && !p[11..].contains('/') => update(conn, c, &i),
        ("DELETE", p) if p.starts_with("/api/links/") && !p[11..].contains('/') => {
            delete(conn, c, &i)
        }
        _ => {
            if let Some(allow) = crate::openapi::allowed_methods(&i.path) {
                Ok(Reply::json(json!({"detail":"Method Not Allowed"}))
                    .status(405)
                    .header("allow", allow))
            } else {
                Err(error(404, "页面不存在"))
            }
        }
    }
}

#[utoipa::path(post,path="/api/admin/login",tag="admin",request_body=crate::models::LoginRequest,responses((status=200,body=crate::models::LoginResponse),(status=422,body=crate::models::ApiError)))]
pub fn login(_conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    ensure(!c.password.is_empty(), 503, "管理员密码未配置")?;
    let body: models::LoginRequest =
        serde_json::from_slice(&i.body).map_err(|_| error(422, "密码字段无效"))?;
    let password = body.password;
    ensure(password.chars().count() <= 1024, 422, "密码字段过长")?;
    ensure(
        bool::from(password.as_bytes().ct_eq(c.password.as_bytes())),
        401,
        "密码错误",
    )?;
    let expires = Utc::now().timestamp() + 86400;
    let nonce: String = rand::rng()
        .sample_iter(Alphanumeric)
        .take(22)
        .map(char::from)
        .collect();
    let payload =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({"exp":expires,"nonce":nonce})).unwrap());
    let signature = URL_SAFE_NO_PAD.encode(sign(c, &payload).finalize().into_bytes());
    Ok(Reply::json(
        json!({"token":format!("{payload}.{signature}"),"expires_at":expires}),
    ))
}

#[utoipa::path(get,path="/api/site",tag="site",responses((status=200,body=crate::models::Site),(status=422,body=crate::models::ApiError)))]
pub fn site(conn: &Connection, c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(
        json!({"app_name":validation::get_site(conn,c)?["app_name"],"version":VERSION}),
    ))
}

#[utoipa::path(get,path="/api/admin/settings",tag="admin",responses((status=200,body=crate::models::AdminSettings),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn read_settings(conn: &Connection, c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(
        json!({"site":validation::get_site(conn,c)?,"version":VERSION,"database_bytes":std::fs::metadata(&c.database).map(|m|m.len()).unwrap_or(0),"trusted_proxy_count":c.trusted,"max_restore_bytes":MAX_BACKUP,"links":db::scalar(conn,"SELECT COUNT(*) FROM short_links",&[])?,"visits":db::scalar(conn,"SELECT COUNT(*) FROM visits",&[])?}),
    ))
}

#[utoipa::path(put,path="/api/admin/settings",tag="admin",request_body=crate::models::SiteSettings,responses((status=200,body=crate::models::SiteSettings),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn update_settings(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let body = validation::site(&i.json()?)?;
    validation::save_site(conn, &body)?;
    Ok(Reply::json(validation::get_site(conn, c)?))
}

#[utoipa::path(post,path="/api/links",tag="links",request_body=crate::models::CreateLink,responses((status=201,body=crate::models::LinkResponse),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn create(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let body = i.json()?;
    let fields = validation::fields(&body)?;
    let site = validation::get_site(conn, c)?;
    let code = match body.get("short_code") {
        None | Some(Value::Null) => "",
        Some(Value::String(s)) => s.trim(),
        _ => return Err(error(422, "短码类型无效")),
    };
    if !code.is_empty() {
        validation::code(code, false)?;
    }
    for _ in 0..50 {
        let random: String = rand::rng()
            .sample_iter(Alphanumeric)
            .take(site["random_code_length"].as_u64().unwrap() as usize)
            .map(char::from)
            .collect();
        let chosen = if code.is_empty() { &random } else { code };
        if !db::scalar(
            conn,
            "SELECT id FROM short_links WHERE short_code=?",
            &[json!(chosen)],
        )?
        .is_null()
        {
            if !code.is_empty() {
                return Err(error(409, "该短码已被使用"));
            }
            continue;
        }
        db::exec(
            conn,
            "INSERT INTO short_links(short_code,original_url,title,note,is_custom,created_at) VALUES (?,?,?,?,?,?)",
            &[
                json!(chosen),
                fields["original_url"].clone(),
                fields["title"].clone(),
                fields["note"].clone(),
                json!(!code.is_empty()),
                json!(db::now()),
            ],
        )?;
        let id = json!(conn.last_insert_rowid());
        save_tags(conn, &id, &fields["tags"])?;
        return Ok(
            Reply::json(response(conn, db::link(conn, chosen)?, &site, json!(0))?).status(201),
        );
    }
    Err(error(503, "生成短码失败，请重试"))
}

#[utoipa::path(get,path="/api/links",tag="links",params(("skip"=Option<i64>,Query,minimum=0,example=0),("limit"=Option<i64>,Query,minimum=1,maximum=100,example=10),("q"=Option<String>,Query,max_length=200),("tag"=Option<String>,Query,max_length=40),("sort"=Option<String>,Query,pattern="^(newest|oldest|visits)$",example="newest")),responses((status=200,body=crate::models::LinkList),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn list(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let skip = i.number("skip", 0, 0, i64::MAX)?;
    let limit = i.number("limit", 10, 1, 100)?;
    let sort = if i.param("sort").is_empty() {
        "newest"
    } else {
        i.param("sort")
    };
    ensure(
        i.param("q").chars().count() <= 200 && i.param("tag").chars().count() <= 40,
        422,
        "参数过长",
    )?;
    let ordering = match sort {
        "newest" => "s.id DESC",
        "oldest" => "s.id ASC",
        "visits" => "n DESC",
        _ => return Err(error(422, "排序参数无效")),
    };
    let (mut filters, mut params) = (vec!["1".to_string()], vec![]);
    if !i.param("q").trim().is_empty() {
        let q = format!(
            "%{}%",
            i.param("q")
                .trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        filters.push(r"(s.title LIKE ? ESCAPE '\' OR s.note LIKE ? ESCAPE '\' OR s.original_url LIKE ? ESCAPE '\' OR s.short_code LIKE ? ESCAPE '\' OR EXISTS (SELECT 1 FROM link_tags t WHERE t.short_link_id=s.id AND t.name LIKE ? ESCAPE '\'))".into());
        params.extend(std::iter::repeat_n(json!(q), 5));
    }
    if !i.param("tag").is_empty() {
        filters.push(
            "EXISTS (SELECT 1 FROM link_tags t WHERE t.short_link_id=s.id AND t.name=?)".into(),
        );
        params.push(json!(i.param("tag")));
    }
    let where_sql = filters.join(" AND ");
    let total = db::scalar(
        conn,
        &format!("SELECT COUNT(*) FROM short_links s WHERE {where_sql}"),
        &params,
    )?;
    params.extend([json!(limit), json!(skip)]);
    let rows = db::rows(
        conn,
        &format!(
            "SELECT s.*,(SELECT COUNT(*) FROM visits v WHERE v.short_link_id=s.id) AS n FROM short_links s WHERE {where_sql} ORDER BY {ordering},s.id DESC LIMIT ? OFFSET ?"
        ),
        &params,
    )?;
    let site = validation::get_site(conn, c)?;
    let items: Result<Vec<_>> = rows
        .into_iter()
        .map(|link| {
            let n = link["n"].clone();
            response(conn, link, &site, n)
        })
        .collect();
    Ok(Reply::json(json!({"total":total,"items":items?})))
}

#[utoipa::path(get,path="/api/links/tags",tag="links",responses((status=200,body=Vec<crate::models::Tag>),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn tags(conn: &Connection, _c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(json!(db::rows(
        conn,
        "SELECT name,COUNT(*) AS link_count FROM link_tags GROUP BY name ORDER BY name",
        &[]
    )?)))
}

#[utoipa::path(get,path="/r/{short_code}",tag="redirect",params(("short_code"=String,Path)),responses((status=302),(status=422,body=crate::models::ApiError)))]
pub fn redirect(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let p = i.path.as_str();
    let link = db::link(conn, &p[3..])?;
    let ua = i.header("user-agent");
    let agent: String = ua.chars().take(8192).collect();
    let (device, browser, bot) = validation::agent(ua);
    let referer = url::Url::parse(i.header("referer"))
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .and_then(|u| {
            u.host_str()
                .map(|s| s.chars().take(253).collect::<String>())
        });
    let mut ip = i.ip.clone();
    let forwarded: Vec<_> = i
        .header("x-forwarded-for")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if c.trusted > 0 && !forwarded.is_empty() {
        // An invalid proxy value must not create an oversized/unrestorable IP.
        if let Ok(address) =
            forwarded[forwarded.len().saturating_sub(c.trusted)].parse::<std::net::IpAddr>()
        {
            ip = address.to_string();
        }
    }
    db::exec(
        conn,
        "INSERT INTO visits(short_link_id,visited_at,ip,user_agent,referrer_domain,device,browser,is_bot) VALUES (?,?,?,?,?,?,?,?)",
        &[
            link["id"].clone(),
            json!(db::now()),
            json!(ip),
            json!(agent),
            json!(referer),
            json!(device),
            json!(browser),
            json!(bot),
        ],
    )?;
    let location =
        percent_encoding::utf8_percent_encode(db::text(&link, "original_url")?, URL_ESCAPES)
            .to_string();
    Ok(Reply {
        status: 302,
        headers: vec![("location".into(), location)],
        body: Payload::Bytes(Vec::new()),
    })
}

#[utoipa::path(get,path="/api/links/stats/{short_code}",tag="links",params(("short_code"=String,Path),("skip"=Option<i64>,Query,minimum=0,example=0),("limit"=Option<i64>,Query,minimum=1,maximum=200,example=50),("exclude_bots"=Option<bool>,Query,example=false)),responses((status=200,body=crate::models::VisitStats),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn visits(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let p = i.path.as_str();
    let link = db::link(conn, &p[17..])?;
    let skip = i.number("skip", 0, 0, i64::MAX)?;
    let limit = i.number("limit", 50, 1, 200)?;
    let where_sql = if i.boolean("exclude_bots")? {
        "short_link_id=? AND is_bot=0"
    } else {
        "short_link_id=?"
    };
    let total = db::scalar(
        conn,
        &format!("SELECT COUNT(*) FROM visits WHERE {where_sql}"),
        &[link["id"].clone()],
    )?;
    let mut visits = db::rows(
        conn,
        &format!(
            "SELECT visited_at,ip,user_agent,referrer_domain,device,browser,is_bot FROM visits WHERE {where_sql} ORDER BY visited_at DESC,id DESC LIMIT ? OFFSET ?"
        ),
        &[link["id"].clone(), json!(limit), json!(skip)],
    )?;
    for visit in &mut visits {
        visit["visited_at"] = json!(db::iso(db::text(visit, "visited_at")?, true)?);
        visit["is_bot"] = json!(visit["is_bot"].as_i64().unwrap_or(0) != 0);
    }
    let site = validation::get_site(conn, c)?;
    Ok(Reply::json(
        json!({"short_code":link["short_code"],"short_url":format!("{}/r/{}",site["base_url"].as_str().unwrap(),db::text(&link,"short_code")?),"original_url":link["original_url"],"total_visits":total,"visits":visits}),
    ))
}

#[utoipa::path(put,path="/api/links/{short_code}",tag="links",request_body=crate::models::LinkFields,params(("short_code"=String,Path)),responses((status=200,body=crate::models::LinkResponse),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn update(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let p = i.path.as_str();
    let code = &p[11..];
    let link = db::link(conn, code)?;
    let fields = validation::fields(&i.json()?)?;
    db::exec(
        conn,
        "UPDATE short_links SET original_url=?,title=?,note=? WHERE id=?",
        &[
            fields["original_url"].clone(),
            fields["title"].clone(),
            fields["note"].clone(),
            link["id"].clone(),
        ],
    )?;
    db::exec(
        conn,
        "DELETE FROM link_tags WHERE short_link_id=?",
        &[link["id"].clone()],
    )?;
    save_tags(conn, &link["id"], &fields["tags"])?;
    let count = db::scalar(
        conn,
        "SELECT COUNT(*) FROM visits WHERE short_link_id=?",
        &[link["id"].clone()],
    )?;
    Ok(Reply::json(response(
        conn,
        db::link(conn, code)?,
        &validation::get_site(conn, c)?,
        count,
    )?))
}

#[utoipa::path(delete,path="/api/links/{short_code}",tag="links",params(("short_code"=String,Path)),responses((status=204),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn delete(conn: &Connection, _c: &Config, i: &Input) -> Result<Reply> {
    let p = i.path.as_str();
    let link = db::link(conn, &p[11..])?;
    for table in ["visits", "link_tags"] {
        db::exec(
            conn,
            &format!("DELETE FROM {table} WHERE short_link_id=?"),
            &[link["id"].clone()],
        )?;
    }
    db::exec(
        conn,
        "DELETE FROM short_links WHERE id=?",
        &[link["id"].clone()],
    )?;
    Ok(Reply {
        status: 204,
        headers: vec![],
        body: Payload::Bytes(vec![]),
    })
}

pub struct LimitedFile {
    pub file: std::io::BufWriter<std::fs::File>,
    pub written: usize,
}
impl Write for LimitedFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written + buf.len() > MAX_BACKUP {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "backup too large",
            ));
        }
        let n = self.file.write(buf)?;
        self.written += n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
#[utoipa::path(get,path="/api/admin/backup",tag="admin",responses((status=200,body=crate::models::BackupData),(status=413,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn backup(conn: &Connection, c: &Config) -> Result<Reply> {
    let mut output = LimitedFile {
        file: std::io::BufWriter::new(tempfile::tempfile()?),
        written: 0,
    };
    fn write_json(output: &mut LimitedFile, v: &Value) -> Result<()> {
        serde_json::to_writer(output, v).map_err(|e| {
            error(
                if e.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
                    413
                } else {
                    500
                },
                "便携备份超过 50 MiB，请停机备份挂载的 data 目录",
            )
        })
    }
    let mut meta = json!({"format":"link-withu-backup","version":1,"exported_at":Utc::now().to_rfc3339(),"settings":validation::get_site(conn,c)?});
    // Emit incrementally; never collect visits or the complete serialized backup.
    let prefix = serde_json::to_vec(&meta).unwrap();
    output.write_all(&prefix[..prefix.len() - 1])?;
    output.write_all(b",\"links\":[")?;
    for (table, time_field, boolean) in [
        ("short_links", "created_at", "is_custom"),
        ("visits", "visited_at", "is_bot"),
    ] {
        if table == "visits" {
            output.write_all(b"],\"visits\":[")?;
        }
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY id"))?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query([])?;
        let mut first = true;
        while let Some(row) = rows.next()? {
            meta = db::row_json(row, &names)?;
            meta[time_field] = json!(db::iso(db::text(&meta, time_field)?, true)?);
            meta[boolean] = json!(meta[boolean].as_i64().unwrap_or(0) != 0);
            if table == "short_links" {
                meta["tags"] = db::tags(conn, &meta["id"])?;
            }
            if !first {
                output.write_all(b",")?;
            }
            write_json(&mut output, &meta)?;
            first = false;
        }
    }
    output.write_all(b"]}")?;
    Reply::file(
        output.file.into_inner().map_err(|e| e.into_error())?,
        "application/json",
        Some(&format!(
            "link-withu-{}.json",
            Utc::now().format("%Y%m%d-%H%M%S")
        )),
    )
}
