mod analytics;
mod api;
mod config;
mod db;
mod models;
mod openapi;
mod restore;
mod validation;
use config::Config;

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    io::{BufRead, Read, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, mpsc},
};
use tokio::sync::{Semaphore, oneshot};
use tokio_util::io::ReaderStream;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_BACKUP: usize = 50 * 1024 * 1024;
type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
struct Error(u16, String);
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        eprintln!("sqlite: {e}");
        Self(500, "数据库操作失败".into())
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        eprintln!("io: {e}");
        Self(
            if e.kind() == std::io::ErrorKind::FileTooLarge {
                413
            } else {
                500
            },
            "文件操作失败".into(),
        )
    }
}
fn error(status: u16, text: &str) -> Error {
    Error(status, text.into())
}
fn ensure(ok: bool, status: u16, text: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(status, text)) }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Payload,
}
enum Payload {
    Bytes(Vec<u8>),
    File(std::fs::File),
}
impl Reply {
    fn json(v: Value) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Payload::Bytes(serde_json::to_vec(&v).expect("JSON serialization")),
        }
    }
    fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
    fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    fn file(mut file: std::fs::File, kind: &str, name: Option<&str>) -> Result<Self> {
        use std::io::{Seek, SeekFrom};
        let size = file.metadata()?.len();
        file.seek(SeekFrom::Start(0))?;
        let mut r = Self {
            status: 200,
            headers: vec![
                ("content-type".into(), kind.into()),
                ("content-length".into(), size.to_string()),
            ],
            body: Payload::File(file),
        };
        if let Some(name) = name {
            r = r
                .header(
                    "content-disposition",
                    format!("attachment; filename=\"{name}\""),
                )
                .header("cache-control", "no-store");
        }
        Ok(r)
    }
}
impl IntoResponse for Reply {
    fn into_response(self) -> Response {
        let mut builder = Response::builder().status(self.status);
        for (k, v) in self.headers {
            builder = builder.header(k, v);
        }
        let body = match self.body {
            Payload::Bytes(v) => Body::from(v),
            Payload::File(f) => Body::from_stream(ReaderStream::new(tokio::fs::File::from_std(f))),
        };
        builder.body(body).expect("valid response")
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        Reply::json(json!({"detail":self.1}))
            .status(self.0)
            .into_response()
    }
}

type Action = Box<dyn FnOnce(&Connection) -> Result<Reply> + Send>;
struct Job {
    action: Action,
    result: oneshot::Sender<Result<Reply>>,
    checkpoint: bool,
}
#[derive(Clone)]
struct Worker(mpsc::SyncSender<Job>);
impl Worker {
    fn start(config: &Config, write: bool) -> Self {
        let conn = db::connect(config).expect("open database");
        let (sender, receiver) = mpsc::sync_channel::<Job>(64);
        std::thread::Builder::new()
            .name(
                if write {
                    "sqlite-writer"
                } else {
                    "sqlite-reader"
                }
                .into(),
            )
            .stack_size(512 * 1024)
            .spawn(move || {
                for job in receiver {
                    if job.result.is_closed() {
                        continue;
                    }
                    let result = (|| {
                        conn.execute_batch(if write { "BEGIN IMMEDIATE" } else { "BEGIN" })?;
                        let reply = (job.action)(&conn)?;
                        conn.execute_batch("COMMIT")?;
                        if job.checkpoint {
                            db::checkpoint(&conn);
                        }
                        Ok(reply)
                    })();
                    if !conn.is_autocommit()
                        && let Err(e) = conn.execute_batch("ROLLBACK")
                    {
                        eprintln!("rollback: {e}");
                    }
                    let _ = job.result.send(result);
                }
            })
            .expect("SQLite thread");
        Self(sender)
    }
    async fn call(
        &self,
        action: impl FnOnce(&Connection) -> Result<Reply> + Send + 'static,
    ) -> Result<Reply> {
        self.submit(Box::new(action), false).await
    }
    async fn restore(
        &self,
        action: impl FnOnce(&Connection) -> Result<Reply> + Send + 'static,
    ) -> Result<Reply> {
        self.submit(Box::new(action), true).await
    }
    async fn submit(&self, action: Action, checkpoint: bool) -> Result<Reply> {
        let (sender, receiver) = oneshot::channel();
        self.0
            .try_send(Job {
                action,
                result: sender,
                checkpoint,
            })
            .map_err(|_| error(503, "数据库繁忙，请重试"))?;
        receiver
            .await
            .map_err(|_| error(500, "数据库工作线程已停止"))?
    }
}
struct App {
    config: Config,
    reader: Worker,
    writer: Worker,
    admission: Semaphore,
    restore: Semaphore,
}
struct Input {
    method: String,
    path: String,
    query: std::collections::HashMap<String, String>,
    headers: HeaderMap,
    body: Vec<u8>,
    ip: String,
}
impl Input {
    fn header(&self, key: &str) -> &str {
        self.headers
            .get(key)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }
    fn param(&self, key: &str) -> &str {
        self.query.get(key).map(String::as_str).unwrap_or("")
    }
    fn number(&self, key: &str, default: i64, min: i64, max: i64) -> Result<i64> {
        let n = if self.param(key).is_empty() {
            default
        } else {
            self.param(key)
                .parse()
                .map_err(|_| error(422, "参数无效"))?
        };
        ensure((min..=max).contains(&n), 422, "参数超出范围")?;
        Ok(n)
    }
    fn boolean(&self, key: &str) -> Result<bool> {
        match self.param(key).to_ascii_lowercase().as_str() {
            "" | "false" | "0" | "no" | "off" => Ok(false),
            "true" | "1" | "yes" | "on" => Ok(true),
            _ => Err(error(422, "布尔参数无效")),
        }
    }
    fn json(&self) -> Result<Value> {
        serde_json::from_slice(&self.body).map_err(|_| error(422, "JSON 格式无效"))
    }
}
fn sign(config: &Config, payload: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(config.password.as_bytes()).expect("HMAC key");
    mac.update(payload.as_bytes());
    mac
}
fn authenticate(config: &Config, headers: &HeaderMap) -> Result<()> {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (_, token) = value
        .split_once(' ')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .ok_or_else(|| error(401, "请先登录管理员"))?;
    let token = token.trim();
    ensure(!config.password.is_empty(), 503, "管理员密码未配置")?;
    let (payload, signature) = token
        .split_once('.')
        .ok_or_else(|| error(401, "管理员登录无效"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| error(401, "管理员登录无效"))?;
    sign(config, payload)
        .verify_slice(&decoded)
        .map_err(|_| error(401, "管理员登录无效"))?;
    let data: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| error(401, "管理员登录无效"))?,
    )
    .map_err(|_| error(401, "管理员登录无效"))?;
    ensure(
        data["exp"].as_i64().unwrap_or(0) > Utc::now().timestamp(),
        401,
        "管理员登录已过期",
    )
}
#[utoipa::path(get,path="/api/health",tag="health",responses((status=200,body=models::Health)))]
fn health() -> Reply {
    Reply::json(json!({"status":"ok","version":VERSION}))
}
async fn handle(
    State(app): State<Arc<App>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    match handle_result(app, peer, request).await {
        Ok(reply) => reply.into_response(),
        Err(e) => e.into_response(),
    }
}
async fn handle_result(app: Arc<App>, peer: SocketAddr, request: Request) -> Result<Reply> {
    let (parts, body) = request.into_parts();
    let path = percent_encoding::percent_decode_str(parts.uri.path())
        .decode_utf8()
        .map_err(|_| error(400, "路径无效"))?
        .into_owned();
    if parts.method == "GET" && path == "/api/health" {
        return Ok(health());
    }
    let _permit = app
        .admission
        .try_acquire()
        .map_err(|_| error(503, "服务繁忙，请重试"))?;
    if parts.method == "GET" {
        match path.as_str() {
            "/openapi.json" => return Ok(Reply::json(openapi::document(&app.config))),
            "/docs" => {
                return Ok(Reply {
                    status: 200,
                    headers: vec![("content-type".into(), "text/html; charset=utf-8".into())],
                    body: Payload::Bytes(include_bytes!("docs.html").to_vec()),
                });
            }
            "/redoc" => {
                return Ok(Reply {
                    status: 200,
                    headers: vec![("content-type".into(), "text/html; charset=utf-8".into())],
                    body: Payload::Bytes(include_bytes!("redoc.html").to_vec()),
                });
            }
            _ => {}
        }
    }
    if !path.starts_with("/api/") && !path.starts_with("/r/") {
        ensure(parts.method == "GET", 405, "Method Not Allowed")?;
        let relative = match path.as_str() {
            "" | "/" | "/login" | "/links" | "/analytics" | "/settings" => "index.html",
            "/favicon.svg" => "favicon.svg",
            p if p.starts_with("/assets/") => &p[1..],
            _ => return Err(error(404, "页面不存在")),
        };
        ensure(
            std::path::Path::new(relative)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
            404,
            "页面不存在",
        )?;
        let file = tokio::fs::File::open(app.config.frontend.join(relative))
            .await
            .map_err(|_| error(404, "页面不存在"))?;
        let mime = if relative.ends_with(".js") {
            "text/javascript"
        } else if relative.ends_with(".css") {
            "text/css"
        } else if relative.ends_with(".svg") {
            "image/svg+xml"
        } else if relative.ends_with(".json") {
            "application/json"
        } else {
            "text/html; charset=utf-8"
        };
        return Reply::file(file.into_std().await, mime, None);
    }
    if (path.starts_with("/api/links")
        || path.starts_with("/api/analytics")
        || path.starts_with("/api/admin/"))
        && path != "/api/admin/login"
    {
        authenticate(&app.config, &parts.headers)?;
    }
    if parts.method == "POST" && path == "/api/admin/restore" {
        return restore::receive(&app, &parts.headers, body).await;
    }
    let write = matches!(parts.method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE")
        || path.starts_with("/r/");
    let body = to_bytes(body, 64 * 1024)
        .await
        .map_err(|_| error(413, "请求超过 64 KiB"))?
        .to_vec();
    let input = Input {
        method: parts.method.to_string(),
        path,
        query: url::form_urlencoded::parse(parts.uri.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect(),
        headers: parts.headers,
        body,
        ip: peer.ip().to_string(),
    };
    let config = app.config.clone();
    let worker = if write { &app.writer } else { &app.reader };
    worker
        .call(move |conn| api::dispatch(conn, &config, input))
        .await
}
fn healthcheck() -> std::io::Result<bool> {
    use std::net::TcpStream;
    let mut stream = TcpStream::connect_timeout(
        &format!(
            "127.0.0.1:{}",
            std::env::var("PORT").unwrap_or_else(|_| "5201".into())
        )
        .parse()
        .map_err(std::io::Error::other)?,
        std::time::Duration::from_secs(3),
    )?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    stream.write_all(b"GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut status = String::new();
    std::io::BufReader::new(stream)
        .take(64)
        .read_line(&mut status)?;
    Ok(status.split_whitespace().nth(1) == Some("200"))
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::args().any(|arg| arg == "--healthcheck") {
        std::process::exit(if healthcheck().unwrap_or(false) { 0 } else { 1 });
    }
    if std::env::args().any(|arg| arg == "--version") {
        println!("v{VERSION}; SQLite {}", rusqlite::version());
        return;
    }
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    db::init(&config).expect("initialize database");
    if std::env::args().any(|arg| arg == "--init-db") {
        return;
    }
    let app = Arc::new(App {
        reader: Worker::start(&config, false),
        writer: Worker::start(&config, true),
        config,
        admission: Semaphore::new(64),
        restore: Semaphore::new(1),
    });
    let address = format!("0.0.0.0:{}", app.config.port);
    let router = Router::new().fallback(handle).with_state(app);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("bind server");
    eprintln!("Link WithU v{VERSION} listening on {address}");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=term.recv()=>{} }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .expect("serve HTTP");
}
