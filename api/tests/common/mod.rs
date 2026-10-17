#![allow(dead_code)]
use reqwest::{
    Method,
    blocking::{Client, Response},
    redirect::Policy,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub fn binary() -> PathBuf {
    std::env::var_os("LINK_WITHU_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_link-withu")))
}

pub struct Server {
    child: Child,
    pub client: Client,
    pub base: String,
    pub token: String,
    pub database: PathBuf,
    pub directory: tempfile::TempDir,
}
impl Server {
    pub fn new() -> Self {
        Self::configured("Asia/Shanghai", 0)
    }
    pub fn configured(tz: &str, trusted: usize) -> Self {
        Self::from_directory(tempfile::tempdir().unwrap(), tz, trusted)
    }
    pub fn from_directory(directory: tempfile::TempDir, tz: &str, trusted: usize) -> Self {
        Self::start(directory, tz, trusted, &[])
    }
    pub fn with_env(environment: &[(&str, &str)]) -> Self {
        Self::start(
            tempfile::tempdir().unwrap(),
            "Asia/Shanghai",
            0,
            environment,
        )
    }
    fn start(
        directory: tempfile::TempDir,
        tz: &str,
        trusted: usize,
        environment: &[(&str, &str)],
    ) -> Self {
        let database = directory.path().join("test.db");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let log = std::fs::File::create(directory.path().join("server.log")).unwrap();
        let mut child = Command::new(binary())
            .env("PORT", port.to_string())
            .env("TZ", tz)
            .env("TRUSTED_PROXY_COUNT", trusted.to_string())
            .env("DATABASE_URL", format!("sqlite:///{}", database.display()))
            .env("ADMIN_PASSWORD", "integration-test-only")
            .env("APP_NAME", "Link WithU")
            .env("RANDOM_CODE_LENGTH", "6")
            .env("BASE_URL", "http://testserver")
            .env(
                "FRONTEND_DIST",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("web/dist"),
            )
            .stdout(Stdio::null())
            .stderr(log)
            .envs(environment.iter().copied())
            .spawn()
            .unwrap();
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let base = format!("http://127.0.0.1:{port}");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if client
                .get(format!("{base}/api/health"))
                .send()
                .is_ok_and(|r| r.status() == 200)
            {
                break;
            }
            if child.try_wait().unwrap().is_some() || Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "Server startup failed: {}",
                    std::fs::read_to_string(directory.path().join("server.log")).unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let password = environment
            .iter()
            .find(|(key, _)| *key == "ADMIN_PASSWORD")
            .map_or("integration-test-only", |(_, value)| *value);
        let token = if password.is_empty() {
            String::new()
        } else {
            let response = client
                .post(format!("{base}/api/admin/login"))
                .json(&json!({"password":password}))
                .send()
                .unwrap();
            assert_eq!(response.status(), 200);
            response.json::<Value>().unwrap()["token"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        Self {
            child,
            client,
            base,
            token,
            database,
            directory,
        }
    }
    pub fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        auth: bool,
    ) -> Response {
        let mut request = self.client.request(method, format!("{}{path}", self.base));
        if auth {
            request = request.bearer_auth(&self.token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().unwrap()
    }
    pub fn get(&self, path: &str) -> Response {
        self.request(Method::GET, path, None, true)
    }
    pub fn json(&self, path: &str) -> Value {
        let r = self.get(path);
        assert_eq!(r.status(), 200, "{}", r.text().unwrap());
        r.json().unwrap()
    }
    pub fn post(&self, path: &str, body: &Value) -> Response {
        self.request(Method::POST, path, Some(body), true)
    }
    pub fn put(&self, path: &str, body: &Value) -> Response {
        self.request(Method::PUT, path, Some(body), true)
    }
    pub fn delete(&self, path: &str) -> Response {
        self.request(Method::DELETE, path, None, true)
    }
    pub fn create(&self, code: &str, extra: Value) -> Value {
        let mut body = json!({"original_url":"https://example.org/path","short_code":code});
        if let Some(fields) = extra.as_object() {
            for (k, v) in fields {
                body[k] = v.clone();
            }
        }
        let r = self.post("/api/links", &body);
        assert_eq!(r.status(), 201, "{}", r.text().unwrap());
        r.json().unwrap()
    }
    pub fn restore(&self, body: &Value) -> Response {
        self.client
            .post(format!("{}/api/admin/restore", self.base))
            .bearer_auth(&self.token)
            .header("X-Confirm-Restore", "RESTORE")
            .json(body)
            .send()
            .unwrap()
    }
    pub fn sql(&self, sql: &str) {
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .execute_batch(sql)
            .unwrap();
    }
    pub fn seed_visits(&self, link: i64, rows: usize, agent: &str) {
        let mut db = rusqlite::Connection::open(&self.database).unwrap();
        let tx = db.transaction().unwrap();
        {
            let mut insert = tx
                .prepare("INSERT INTO visits(short_link_id,visited_at,user_agent) VALUES (?,?,?)")
                .unwrap();
            for _ in 0..rows {
                insert
                    .execute(rusqlite::params![
                        link,
                        chrono::Utc::now()
                            .format("%Y-%m-%d %H:%M:%S%.6f")
                            .to_string(),
                        agent
                    ])
                    .unwrap();
            }
        }
        tx.commit().unwrap();
    }
    pub fn stop(&mut self) -> std::process::ExitStatus {
        if let Some(status) = self.child.try_wait().unwrap() {
            return status;
        }
        #[cfg(unix)]
        {
            let _ = Command::new("sh")
                .args([
                    "-c",
                    "kill -TERM \"$1\"",
                    "sh",
                    &self.child.id().to_string(),
                ])
                .status();
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = self.child.kill();
        self.child.wait().unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
