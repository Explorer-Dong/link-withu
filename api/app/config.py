from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict

_API_ROOT = Path(__file__).resolve().parent.parent
_PROJECT_ROOT = _API_ROOT.parent


class Settings(BaseSettings):
    app_name: str = "Link WithU"
    database_url: str = "sqlite+aiosqlite:///./short_links.db"
    base_url: str = "http://localhost:5201"
    random_code_length: int = 6
    admin_password: str = ""
    # 反向代理链中可信代理的层数（如 nginx/CDN）；0 表示不信任任何转发头
    trusted_proxy_count: int = 0
    frontend_dist: Path | None = None

    model_config = SettingsConfigDict(
        env_file=(_PROJECT_ROOT / ".env", _API_ROOT / ".env"),
        env_file_encoding="utf-8",
    )


settings = Settings()
