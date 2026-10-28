//! Stream uploads into a disposable SQLite database, then atomically replace data.
use crate::*;
use axum::body::Bytes;
use futures_util::StreamExt;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::{fmt, io};

const CHUNK: usize = 64 * 1024;
const MAX_STRING: usize = 128 * 1024; // Includes escaped representations of every allowed field.
struct BodyReader {
    receiver: tokio::sync::mpsc::Receiver<Bytes>,
    chunk: Bytes,
    in_string: bool,
    escaped: bool,
    string_bytes: usize,
}
impl Read for BodyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.chunk.is_empty() {
            match self.receiver.blocking_recv() {
                Some(chunk) => self.chunk = chunk,
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.chunk.len());
        buf[..n].copy_from_slice(&self.chunk[..n]);
        self.chunk = self.chunk.slice(n..);
        for byte in &buf[..n] {
            if self.in_string {
                self.string_bytes += 1;
                if self.string_bytes > MAX_STRING {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "JSON 字符串过长",
                    ));
                }
                if self.escaped {
                    self.escaped = false;
                } else if *byte == b'\\' {
                    self.escaped = true;
                } else if *byte == b'"' {
                    self.in_string = false;
                }
            } else if *byte == b'"' {
                self.in_string = true;
                self.string_bytes = 0;
            }
        }
        Ok(n)
    }
}
pub struct Staged {
    connection: Connection,
    _file: tempfile::NamedTempFile,
    links: i64,
    visits: i64,
    site: Value,
}
impl Staged {
    fn new(c: &Config) -> Result<Self> {
        let directory = std::path::Path::new(&c.database)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let file = tempfile::Builder::new()
            .prefix(".link-withu-restore-")
            .tempfile_in(directory)?;
        let connection = Connection::open(file.path())?;
        connection.set_prepared_statement_cache_capacity(8);
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA cache_size=-512; PRAGMA mmap_size=0; PRAGMA temp_store=FILE; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; BEGIN;")?;
        // Deferred constraints allow visits to precede links in a JSON object.
        connection.execute_batch(&include_str!("schema.sql").replace(
            "REFERENCES short_links(id) ON DELETE CASCADE",
            "REFERENCES short_links(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED",
        ))?;
        Ok(Self {
            connection,
            _file: file,
            links: 0,
            visits: 0,
            site: Value::Null,
        })
    }
}
struct Rows<'a> {
    stage: &'a mut Staged,
    links: bool,
}
impl<'de> DeserializeSeed<'de> for Rows<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Rows<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("backup record array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        if self.links {
            while let Some(link) = seq.next_element::<models::BackupLink>()? {
                let v = serde_json::to_value(link).map_err(de::Error::custom)?;
                validate_link(&v).map_err(|e| de::Error::custom(e.1))?;
                let fields = validation::fields(&v).map_err(|e| de::Error::custom(e.1))?;
                let at = db::parse_date(v["created_at"].as_str().unwrap())
                    .map_err(|e| de::Error::custom(e.1))?
                    .format("%Y-%m-%d %H:%M:%S%.6f")
                    .to_string();
                db::exec(&self.stage.connection,"INSERT INTO short_links(id,short_code,original_url,title,note,is_custom,created_at) VALUES (?,?,?,?,?,?,?)",&[v["id"].clone(),v["short_code"].clone(),fields["original_url"].clone(),fields["title"].clone(),fields["note"].clone(),v["is_custom"].clone(),json!(at)]).map_err(|_|de::Error::custom("备份包含重复 ID 或短码"))?;
                for tag in fields["tags"].as_array().unwrap() {
                    db::exec(
                        &self.stage.connection,
                        "INSERT INTO link_tags(short_link_id,name) VALUES (?,?)",
                        &[v["id"].clone(), tag.clone()],
                    )
                    .map_err(|_| de::Error::custom("标签无效"))?;
                }
                self.stage.links += 1;
            }
        } else {
            while let Some(visit) = seq.next_element::<models::BackupVisit>()? {
                let mut v = serde_json::to_value(visit).map_err(de::Error::custom)?;
                validate_visit(&v).map_err(|e| de::Error::custom(e.1))?;
                v["visited_at"] = json!(
                    db::parse_date(v["visited_at"].as_str().unwrap())
                        .map_err(|e| de::Error::custom(e.1))?
                        .format("%Y-%m-%d %H:%M:%S%.6f")
                        .to_string()
                );
                db::exec(&self.stage.connection,"INSERT INTO visits(id,short_link_id,visited_at,ip,user_agent,referrer_domain,device,browser,is_bot) VALUES (?,?,?,?,?,?,?,?,?)",&[v["id"].clone(),v["short_link_id"].clone(),v["visited_at"].clone(),v["ip"].clone(),v["user_agent"].clone(),v["referrer_domain"].clone(),v["device"].clone(),v["browser"].clone(),v["is_bot"].clone()]).map_err(|_|de::Error::custom("备份包含重复访问 ID"))?;
                self.stage.visits += 1;
                crate::geolocation::remember(&self.stage.connection, &v)
                    .map_err(|e| de::Error::custom(e.1))?;
            }
        }
        Ok(())
    }
}
fn validate_link(v: &Value) -> Result<()> {
    ensure(v["id"].as_i64().is_some_and(|n| n > 0), 400, "备份 ID 无效")?;
    validation::code(v["short_code"].as_str().unwrap(), true)?;
    Ok(())
}
fn validate_visit(v: &Value) -> Result<()> {
    ensure(
        v["id"].as_i64().is_some_and(|n| n > 0)
            && v["short_link_id"].as_i64().is_some_and(|n| n > 0),
        400,
        "访问 ID 无效",
    )?;
    for (key, max, nullable) in [
        ("ip", 45, true),
        ("user_agent", 8192, true),
        ("referrer_domain", 253, true),
        ("device", 20, false),
        ("browser", 30, false),
        ("country", 100, false),
        ("region", 100, false),
        ("city", 100, false),
    ] {
        if !nullable || !v[key].is_null() {
            validation::text(v, key, "", max)?;
        }
    }
    Ok(())
}
struct Document<'a>(&'a mut Staged);
impl<'de> DeserializeSeed<'de> for Document<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Document<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("link-withu backup document")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        let mut seen = std::collections::HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if [
                "format",
                "version",
                "exported_at",
                "settings",
                "links",
                "visits",
            ]
            .contains(&key.as_str())
                && !seen.insert(key.clone())
            {
                return Err(de::Error::custom("备份字段重复"));
            }
            match key.as_str() {
                "format" => {
                    if map.next_value::<String>()? != "link-withu-backup" {
                        return Err(de::Error::custom("备份格式无效"));
                    }
                }
                "version" => {
                    if map.next_value::<i64>()? != 1 {
                        return Err(de::Error::custom("备份版本不兼容"));
                    }
                }
                "exported_at" => {
                    db::parse_date(&map.next_value::<String>()?)
                        .map_err(|e| de::Error::custom(e.1))?;
                }
                "settings" => {
                    let site = map.next_value::<models::SiteSettings>()?;
                    self.0.site =
                        validation::site(&serde_json::to_value(site).map_err(de::Error::custom)?)
                            .map_err(|e| de::Error::custom(e.1))?;
                }
                "links" => map.next_value_seed(Rows {
                    stage: self.0,
                    links: true,
                })?,
                "visits" => map.next_value_seed(Rows {
                    stage: self.0,
                    links: false,
                })?,
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        if seen.len() != 6 {
            return Err(de::Error::custom("备份字段缺失"));
        }
        Ok(())
    }
}
fn stage(receiver: tokio::sync::mpsc::Receiver<Bytes>, c: Config) -> Result<Staged> {
    let mut staged = Staged::new(&c)?;
    let reader = io::BufReader::new(BodyReader {
        receiver,
        chunk: Bytes::new(),
        in_string: false,
        escaped: false,
        string_bytes: 0,
    });
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    Document(&mut staged)
        .deserialize(&mut deserializer)
        .map_err(|_| error(400, "备份格式无效或版本不兼容，现有数据未修改"))?;
    deserializer
        .end()
        .map_err(|_| error(400, "备份文件末尾无效，现有数据未修改"))?;
    staged
        .connection
        .execute_batch("COMMIT")
        .map_err(|_| error(400, "备份存在无对应短链的访问记录，现有数据未修改"))?;
    Ok(staged)
}
/// 从 JSON 备份覆盖恢复
///
/// 上传版本 1 的 Link WithU JSON 备份，并在请求头中设置 X-Confirm-Restore: RESTORE。校验通过后，备份中的短链、访问记录和站点设置会原子替换当前数据；当前 API Key 保留。
///
/// 备份最大 50 MiB，同时只允许一次恢复。格式错误或恢复失败时保留原有数据；成功后返回恢复的短链和访问记录数量。
#[utoipa::path(post,path="/api/admin/restore",tag="admin",request_body=crate::models::BackupData,params(("X-Confirm-Restore"=String,Header,description="必须为 RESTORE",example="RESTORE")),responses((status=200,body=crate::models::RestoreResponse),(status=400,body=crate::models::ApiError),(status=409,body=crate::models::ApiError),(status=413,body=crate::models::ApiError)),security(("bearer_auth"=[])))]
pub async fn receive(app: &App, headers: &HeaderMap, body: Body) -> Result<Reply> {
    ensure(
        headers
            .get("x-confirm-restore")
            .is_some_and(|h| h == "RESTORE"),
        400,
        "请确认覆盖恢复操作",
    )?;
    let _permit = app
        .restore
        .try_acquire()
        .map_err(|_| error(409, "已有恢复操作正在进行"))?;
    if let Some(length) = headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        ensure(length <= MAX_BACKUP, 413, "恢复文件不能超过 50 MiB")?;
    }
    let (sender, receiver) = tokio::sync::mpsc::channel(4);
    let config = app.config.clone();
    let parser = tokio::task::spawn_blocking(move || stage(receiver, config));
    let mut stream = body.into_data_stream();
    let mut size = 0;
    let mut failure = None;
    'upload: while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(v) => v,
            Err(_) => {
                failure = Some(error(400, "恢复文件传输中断"));
                break;
            }
        };
        size += chunk.len();
        if size > MAX_BACKUP {
            failure = Some(error(413, "恢复文件不能超过 50 MiB"));
            break;
        }
        for start in (0..chunk.len()).step_by(CHUNK) {
            if sender
                .send(chunk.slice(start..(start + CHUNK).min(chunk.len())))
                .await
                .is_err()
            {
                break 'upload;
            }
        }
    }
    drop(sender);
    let staged = parser.await.map_err(|_| error(500, "恢复工作线程失败"))?;
    if let Some(failure) = failure {
        return Err(failure);
    }
    let staged = staged?;
    app.writer.restore(move |conn| apply(conn, staged)).await
}
fn apply(conn: &Connection, staged: Staged) -> Result<Reply> {
    for table in [
        "visits",
        "link_tags",
        "short_links",
        "app_settings",
        "ip_locations",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }
    for (table, columns) in [
        (
            "short_links",
            "id,short_code,original_url,title,note,is_custom,created_at",
        ),
        ("link_tags", "short_link_id,name"),
        (
            "ip_locations",
            "ip,country,region,city,status,attempts,updated_at,next_attempt",
        ),
        (
            "visits",
            "id,short_link_id,visited_at,ip,user_agent,referrer_domain,device,browser,is_bot",
        ),
    ] {
        let mut source = staged
            .connection
            .prepare(&format!("SELECT {columns} FROM {table}"))?;
        let mut rows = source.query([])?;
        let placeholders = columns
            .split(',')
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let mut target = conn.prepare(&format!(
            "INSERT INTO {table}({columns}) VALUES ({placeholders})"
        ))?;
        while let Some(row) = rows.next()? {
            let values: std::result::Result<Vec<_>, _> = (0..row.as_ref().column_count())
                .map(|i| row.get_ref(i))
                .collect();
            target.execute(rusqlite::params_from_iter(
                values?
                    .into_iter()
                    .map(rusqlite::types::ToSqlOutput::Borrowed),
            ))?;
        }
    }
    validation::save_site(conn, &staged.site)?;
    Ok(Reply::json(
        json!({"links":staged.links,"visits":staged.visits,"message":"恢复完成"}),
    ))
}
