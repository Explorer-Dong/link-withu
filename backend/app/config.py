from pathlib import Path

from pydantic_settings import BaseSettings

# 项目根目录：backend/ 的上一级
_PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent


class Settings(BaseSettings):
    app_name: str = "短链服务"
    database_url: str = "sqlite+aiosqlite:///./short_links.db"
    base_url: str = "http://localhost:5201"  # 短链前缀，部署时改为实际域名
    random_code_length: int = 6
    # 前端构建产物目录，默认指向 frontend/dist；Docker 中可通过环境变量覆盖
    frontend_dist: Path = _PROJECT_ROOT / "frontend" / "dist"

    class Config:
        env_file = ".env"
        env_file_encoding = "utf-8"


settings = Settings()
