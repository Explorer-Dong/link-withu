from pathlib import Path
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

_API_ROOT = Path(__file__).resolve().parent.parent
_PROJECT_ROOT = _API_ROOT.parent


class Settings(BaseSettings):
    app_name: str = "Link WithU"
    database_url: str = f"sqlite+aiosqlite:///{_API_ROOT / 'short_links.db'}"
    base_url: str = "http://localhost:5201"
    random_code_length: int = Field(default=6, ge=4, le=16)
    tz: str = "Asia/Shanghai"
    admin_password: str = ""
    # 反向代理链中可信代理的层数（如 nginx/CDN）；0 表示不信任任何转发头
    trusted_proxy_count: int = Field(default=0, ge=0)
    frontend_dist: Path | None = None

    @field_validator("tz")
    @classmethod
    def valid_timezone(cls, value):
        try:
            ZoneInfo(value)
        except (ZoneInfoNotFoundError, ValueError):
            raise ValueError("TZ 必须是有效时区，例如 Asia/Shanghai")
        return value

    model_config = SettingsConfigDict(
        env_file=(_PROJECT_ROOT / ".env", _API_ROOT / ".env"),
        env_file_encoding="utf-8",
    )


settings = Settings()
