use crate::*;

#[derive(Clone)]
pub struct Config {
    pub password: String,
    pub app_name: String,
    pub base_url: String,
    pub code_length: usize,
    pub tz: chrono_tz::Tz,
    pub tz_name: String,
    pub trusted: usize,
    pub database: String,
    pub frontend: PathBuf,
    pub port: u16,
    pub geoip_url: Option<url::Url>,
}
impl Config {
    pub fn load() -> std::result::Result<Self, String> {
        Self::load_from(PathBuf::from(env!("CARGO_MANIFEST_DIR")), |key| {
            std::env::var(key).ok()
        })
    }
    fn load_from(
        root: PathBuf,
        variable: impl Fn(&str) -> Option<String>,
    ) -> std::result::Result<Self, String> {
        let mut dotenv = std::collections::HashMap::new();
        for path in [root.parent().unwrap().join(".env"), root.join(".env")] {
            if path.is_file() {
                let entries = dotenvy::from_path_iter(path).map_err(|_| ".env 格式无效")?;
                for entry in entries {
                    let (key, value) = entry.map_err(|_| ".env 格式无效")?;
                    dotenv.insert(key, value);
                }
            }
        }
        let env = |key: &str, default: &str| {
            variable(key)
                .or_else(|| dotenv.get(key).cloned())
                .unwrap_or_else(|| default.into())
        };
        let url = env(
            "DATABASE_URL",
            &format!("sqlite:///{}", root.join("short_links.db").display()),
        );
        let database = url
            .strip_prefix("sqlite+aiosqlite:///")
            .or_else(|| url.strip_prefix("sqlite:///"))
            .filter(|p| {
                !p.is_empty() && *p != ":memory:" && !p.starts_with("file:") && !p.contains('?')
            })
            .ok_or("DATABASE_URL 必须是持久化 SQLite 文件地址")?
            .to_owned();
        let tz_name = env("TZ", "Asia/Shanghai");
        let tz = tz_name
            .parse()
            .map_err(|_| "TZ 必须是有效时区，例如 Asia/Shanghai")?;
        let code_length = env("RANDOM_CODE_LENGTH", "6")
            .parse::<usize>()
            .map_err(|_| "RANDOM_CODE_LENGTH 必须是 4–16")?;
        if !(4..=16).contains(&code_length) {
            return Err("RANDOM_CODE_LENGTH 必须是 4–16".into());
        }
        let trusted = env("TRUSTED_PROXY_COUNT", "0")
            .parse()
            .map_err(|_| "TRUSTED_PROXY_COUNT 必须是非负整数")?;
        let port = env("PORT", "5201")
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("PORT 必须是 1–65535")?;
        let config = Self {
            password: env("ADMIN_PASSWORD", ""),
            app_name: env("APP_NAME", "Link WithU"),
            base_url: env("BASE_URL", "http://localhost:5201"),
            code_length,
            tz,
            tz_name,
            trusted,
            database,
            frontend: env(
                "FRONTEND_DIST",
                &root.parent().unwrap().join("web/dist").to_string_lossy(),
            )
            .into(),
            port,
            geoip_url: match env("GEOIP_API_URL", "https://ipwho.is/").trim() {
                "" => None,
                value => {
                    let url = url::Url::parse(value).map_err(|_| "GEOIP_API_URL 地址无效")?;
                    if !matches!(url.scheme(), "http" | "https")
                        || url.host_str().is_none()
                        || !url.username().is_empty()
                        || url.password().is_some()
                        || url.query().is_some()
                        || url.fragment().is_some()
                    {
                        return Err(
                            "GEOIP_API_URL 必须是无认证、查询参数或片段的 HTTP/HTTPS 地址".into(),
                        );
                    }
                    Some(url)
                }
            },
        };
        validation::site(&json!({"app_name": config.app_name,"base_url":config.base_url,"random_code_length":config.code_length,"timezone":config.tz_name}))
            .map_err(|e|format!("站点环境变量无效: {}",e.1))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv_precedence_defaults_and_legacy_database_urls() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("api");
        std::fs::create_dir(&root).unwrap();
        let c = Config::load_from(root.clone(), |_| None).unwrap();
        assert_eq!(c.app_name, "Link WithU");
        assert_eq!(c.code_length, 6);
        assert_eq!(c.port, 5201);
        assert!(c.password.is_empty());
        assert_eq!(c.database, root.join("short_links.db").to_str().unwrap());
        assert_eq!(c.frontend, dir.path().join("web/dist"));
        std::fs::write(
            dir.path().join(".env"),
            "APP_NAME=Root\nBASE_URL=https://root.example.org\nRANDOM_CODE_LENGTH=8\n",
        )
        .unwrap();
        std::fs::write(root.join(".env"), "APP_NAME=API\nADMIN_PASSWORD='local test only'\nPORT=5203\nDATABASE_URL=sqlite+aiosqlite:///legacy.db\n").unwrap();
        let c = Config::load_from(root.clone(), |_| None).unwrap();
        assert_eq!(c.app_name, "API");
        assert_eq!(c.base_url, "https://root.example.org");
        assert_eq!(c.code_length, 8);
        assert_eq!(c.database, "legacy.db");
        assert_eq!(c.port, 5203);
        assert_eq!(c.password, "local test only");
        let c = Config::load_from(root, |key| match key {
            "APP_NAME" => Some("Environment".into()),
            "PORT" => Some("5204".into()),
            "FRONTEND_DIST" => Some("custom/dist".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.app_name, "Environment");
        assert_eq!(c.port, 5204);
        assert_eq!(c.frontend, PathBuf::from("custom/dist"));
    }

    #[test]
    fn invalid_dotenv_and_non_persistent_database_urls_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("api");
        std::fs::create_dir(&root).unwrap();
        for url in [
            "sqlite:///",
            "sqlite:///:memory:",
            "sqlite:///file::memory:",
            "sqlite:///file:db.sqlite",
            "sqlite:///db?mode=memory",
            "mysql://localhost/db",
        ] {
            assert!(
                Config::load_from(root.clone(), |key| (key == "DATABASE_URL")
                    .then(|| url.into()))
                .is_err()
            );
        }
        std::fs::write(root.join(".env"), "INVALID LINE\n").unwrap();
        assert!(Config::load_from(root, |_| None).is_err());
    }
}
