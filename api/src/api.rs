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
        ("POST", "/api/admin/api-key") => generate_api_key(conn, c),
        ("DELETE", "/api/admin/api-key") => revoke_api_key(conn),
        ("POST", "/api/links") => create(conn, c, &i),
        ("GET", "/api/links") => list(conn, c, &i),
        ("GET", "/api/links/tags") => tags(conn, c, &i),
        ("GET", p) if p.starts_with("/api/links/by-code/") && !p[19..].contains('/') => {
            get_link(conn, c, &i)
        }
        ("GET", "/api/analytics") => {
            limited_query(&i)?;
            crate::analytics::analytics(conn, c, &i)
        }
        ("GET", "/api/analytics/export") => {
            limited_query(&i)?;
            crate::analytics::export(conn, c, &i)
        }
        ("GET", "/api/analytics/breakdown") => {
            limited_query(&i)?;
            crate::analytics::breakdown(conn, c, &i)
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

/// 管理员登录
///
/// 使用管理员密码获取有效期为 24 小时的 Bearer 会话令牌及过期时间。修改管理员密码后，旧会话失效。
#[utoipa::path(post,path="/api/admin/login",tag="admin",request_body=crate::models::LoginRequest,responses((status=200,body=crate::models::LoginResponse),(status=401,body=crate::models::ApiError),(status=422,body=crate::models::ApiError),(status=503,body=crate::models::ApiError)))]
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

/// 获取公开站点信息
///
/// 返回站点名称和应用版本，供登录页等公开页面展示。此接口无需认证。
#[utoipa::path(get,path="/api/site",tag="site",responses((status=200,body=crate::models::Site),(status=422,body=crate::models::ApiError)))]
pub fn site(conn: &Connection, c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(
        json!({"app_name":validation::get_site(conn,c)?["app_name"],"version":VERSION}),
    ))
}

/// 获取管理设置
///
/// 返回站点设置、应用版本、数据库大小、链接和访问记录数量、可信代理层数以及 API Key 是否已生成。响应不包含 API Key 明文。
#[utoipa::path(get,path="/api/admin/settings",tag="admin",responses((status=200,body=crate::models::AdminSettings),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn read_settings(conn: &Connection, c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(
        json!({"site":validation::get_site(conn,c)?,"version":VERSION,"database_bytes":std::fs::metadata(&c.database).map(|m|m.len()).unwrap_or(0),"trusted_proxy_count":c.trusted,"max_restore_bytes":MAX_BACKUP,"links":db::scalar(conn,"SELECT COUNT(*) FROM short_links",&[])?,"visits":db::scalar(conn,"SELECT COUNT(*) FROM visits",&[])? ,"api_key_configured":db::scalar(conn,"SELECT EXISTS(SELECT 1 FROM api_credentials WHERE id=1)",&[])?.as_i64()==Some(1)}),
    ))
}

/// 生成或轮换 API Key
///
/// 生成具有管理员权限的 Bearer API Key。明文只在本次响应中返回；重新生成会立即使旧 Key 失效。数据库仅保存摘要，备份不包含 Key。
#[utoipa::path(post,path="/api/admin/api-key",tag="admin",responses((status=200,body=crate::models::ApiKeyCreated),(status=401,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn generate_api_key(conn: &Connection, c: &Config) -> Result<Reply> {
    let mut random = [0u8; 32];
    rand::rng().fill(&mut random);
    let key = format!("lwu_{}", URL_SAFE_NO_PAD.encode(random));
    let hash = URL_SAFE_NO_PAD.encode(sign(c, &key).finalize().into_bytes());
    db::exec(
        conn,
        "INSERT INTO api_credentials(id,key_hash) VALUES (1,?) ON CONFLICT(id) DO UPDATE SET key_hash=excluded.key_hash",
        &[json!(hash)],
    )?;
    Ok(Reply::json(json!({"api_key":key})).header("cache-control", "no-store"))
}

/// 撤销 API Key
///
/// 撤销当前 API Key，使其立即无法访问管理接口。已有管理员登录会话继续有效。
#[utoipa::path(delete,path="/api/admin/api-key",tag="admin",responses((status=204),(status=401,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn revoke_api_key(conn: &Connection) -> Result<Reply> {
    db::exec(conn, "DELETE FROM api_credentials WHERE id=1", &[])?;
    Ok(Reply {
        status: 204,
        headers: vec![("cache-control".into(), "no-store".into())],
        body: Payload::Bytes(vec![]),
    })
}

/// 更新站点设置
///
/// 保存站点名称、短链公开地址和随机短码长度，并返回规范化后的设置。统计时区由服务的 TZ 环境变量决定，需要重启服务才能修改。
#[utoipa::path(put,path="/api/admin/settings",tag="admin",request_body=crate::models::SiteSettings,responses((status=200,body=crate::models::SiteSettings),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn update_settings(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let body = validation::site(&i.json()?)?;
    validation::save_site(conn, &body)?;
    Ok(Reply::json(validation::get_site(conn, c)?))
}

/// 创建短链
///
/// 根据目标地址创建短链，可同时设置标题、备注和标签。未提供短码时自动生成；自定义短码已存在时返回 409。成功后返回完整短链信息。
#[utoipa::path(post,path="/api/links",tag="links",request_body=crate::models::CreateLink,responses((status=201,body=crate::models::LinkResponse),(status=409,body=crate::models::ApiError),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
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

/// 查询短链列表
///
/// 分页获取短链及其访问次数，可按关键词搜索名称、备注、地址、短码和标签，并按标签筛选。返回匹配总数和当前页；默认每页 10 条。
#[utoipa::path(get,path="/api/links",tag="links",params(("skip"=Option<i64>,Query,minimum=0,example=0,description="跳过的记录数，默认 0"),("limit"=Option<i64>,Query,minimum=1,maximum=100,example=10,description="每页记录数，默认 10，最大 100"),("q"=Option<String>,Query,max_length=200,description="搜索名称、备注、目标地址、短码或标签"),("tag"=Option<String>,Query,max_length=40,description="按标签精确筛选"),("sort"=Option<String>,Query,pattern="^(newest|oldest|visits)$",example="newest",description="排序方式：newest 最新、oldest 最早、visits 访问次数；默认 newest")),responses((status=200,body=crate::models::LinkList),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
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

/// 获取标签列表
///
/// 返回所有已使用的标签及各自关联的短链数量，可用于构建标签筛选器。
#[utoipa::path(get,path="/api/links/tags",tag="links",responses((status=200,body=Vec<crate::models::Tag>),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn tags(conn: &Connection, _c: &Config, _i: &Input) -> Result<Reply> {
    Ok(Reply::json(json!(db::rows(
        conn,
        "SELECT name,COUNT(*) AS link_count FROM link_tags GROUP BY name ORDER BY name",
        &[]
    )?)))
}

/// 按短码获取短链
///
/// 精确查询单条短链，返回目标地址、标题、备注、标签、短链地址及累计访问次数。短码不存在时返回 404。
#[utoipa::path(get,path="/api/links/by-code/{short_code}",tag="links",params(("short_code"=String,Path,example="example",description="要查询的短码")),responses((status=200,body=crate::models::LinkResponse),(status=401,body=crate::models::ApiError),(status=404,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn get_link(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let link = db::link(conn, &i.path[19..])?;
    let count = db::scalar(
        conn,
        "SELECT COUNT(*) FROM visits WHERE short_link_id=?",
        &[link["id"].clone()],
    )?;
    Ok(Reply::json(response(
        conn,
        link,
        &validation::get_site(conn, c)?,
        count,
    )?))
}

/// 访问短链并跳转
///
/// 公开访问短码对应的目标地址。服务先保存访问记录，再以 302 和 Location 响应头跳转；响应体为空。短码不存在时返回 404。
#[utoipa::path(get,path="/r/{short_code}",tag="redirect",params(("short_code"=String,Path,example="example",description="要访问的短码")),responses((status=302),(status=404,body=crate::models::ApiError)))]
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
    crate::geolocation::enqueue(conn, &ip)?;
    let location =
        percent_encoding::utf8_percent_encode(db::text(&link, "original_url")?, URL_ESCAPES)
            .to_string();
    Ok(Reply {
        status: 302,
        headers: vec![("location".into(), location)],
        body: Payload::Bytes(Vec::new()),
    })
}

/// 查询短链访问明细
///
/// 返回指定短链的访问总数和分页明细，包括访问时间、IP、地理位置、来源、设备及浏览器。可按日期和机器人筛选；默认每页 50 条。
///
/// start_date 和 end_date 必须同时提供，格式为 YYYY-MM-DD，按站点时区包含起止两天；未指定时间筛选时查询全部历史。日期区间不能与非零 days 同用。
#[utoipa::path(get,path="/api/links/stats/{short_code}",tag="links",params(("short_code"=String,Path,example="example",description="要查询的短码"),("skip"=Option<i64>,Query,minimum=0,example=0,description="跳过的记录数，默认 0"),("limit"=Option<i64>,Query,minimum=1,maximum=200,example=50,description="每页记录数，默认 50，最大 200"),("exclude_bots"=Option<bool>,Query,example=false,description="是否排除疑似机器人访问，默认 false"),("start_date"=Option<String>,Query,format=Date,description="开始日期，与 end_date 同时提供"),("end_date"=Option<String>,Query,format=Date,description="结束日期，包含当天"),("days"=Option<i64>,Query,minimum=0,maximum=90,description="最近 N 天，包含今天；0 不启用天数筛选")),responses((status=200,body=crate::models::VisitStats),(status=404,body=crate::models::ApiError),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub fn visits(conn: &Connection, c: &Config, i: &Input) -> Result<Reply> {
    let p = i.path.as_str();
    let link = db::link(conn, &p[17..])?;
    let skip = i.number("skip", 0, 0, i64::MAX)?;
    let limit = i.number("limit", 50, 1, 200)?;
    let (mut where_sql, mut params) = crate::analytics::visit_filter(c, i)?;
    where_sql.push_str(" AND s.id=?");
    params.push(link["id"].clone());
    let joined = format!(
        "FROM visits v JOIN short_links s ON s.id=v.short_link_id {}",
        crate::geolocation::JOIN
    );
    let total = db::scalar(
        conn,
        &format!("SELECT COUNT(*) {joined} WHERE {where_sql}"),
        &params,
    )?;
    params.extend([json!(limit), json!(skip)]);
    let mut visits = db::rows(
        conn,
        &format!(
            "SELECT v.visited_at,v.ip,v.user_agent,v.referrer_domain,v.device,v.browser,v.is_bot,{},{} AS location {joined} WHERE {where_sql} ORDER BY v.visited_at DESC,v.id DESC LIMIT ? OFFSET ?",
            crate::geolocation::FIELDS,
            crate::geolocation::LOCATION
        ),
        &params,
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

/// 更新短链
///
/// 更新指定短链的目标地址、标题、备注和标签，短码保持不变。必须提供 original_url；未提供的标题、备注和标签会重置为空。
#[utoipa::path(put,path="/api/links/{short_code}",tag="links",request_body=crate::models::LinkFields,params(("short_code"=String,Path,example="example",description="要更新的短码")),responses((status=200,body=crate::models::LinkResponse),(status=404,body=crate::models::ApiError),(status=422,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
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

/// 删除短链
///
/// 删除指定短链及其关联的全部访问记录和标签。成功后返回 204，短码不存在时返回 404。
#[utoipa::path(delete,path="/api/links/{short_code}",tag="links",params(("short_code"=String,Path,example="example",description="要删除的短码")),responses((status=204),(status=404,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
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
/// 导出完整 JSON 备份
///
/// 下载包含全部短链、标签、访问记录和站点设置的版本 1 JSON 备份。API Key 和管理员密码不包含在备份中；备份超过 50 MiB 时返回 413。
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
        let query = if table == "visits" {
            format!(
                "SELECT v.*,{} FROM visits v {} ORDER BY v.id",
                crate::geolocation::FIELDS,
                crate::geolocation::JOIN
            )
        } else {
            format!("SELECT * FROM {table} ORDER BY id")
        };
        let mut stmt = conn.prepare(&query)?;
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
