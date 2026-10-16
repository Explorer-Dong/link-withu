"""Validated API and portable backup contracts."""

import re
from datetime import UTC, datetime
from urllib.parse import urlsplit
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from pydantic import BaseModel, ConfigDict, Field, field_validator

SQLITE_MAX_ID = (1 << 63) - 1


def normalize_url(value: str) -> str:
    value = value.strip()
    host_with_port = re.match(r"^[^/?#:@]+:\d+(?:[/?#]|$)", value)
    if host_with_port or not re.match(r"^[a-zA-Z][a-zA-Z\d+.-]*:", value):
        value = "https://" + value
    try:
        parsed = urlsplit(value)
        valid = (
            parsed.scheme in {"http", "https"}
            and parsed.hostname
            and not parsed.username
            and not parsed.password
        )
        _ = parsed.port
    except ValueError:
        valid = False
    if not valid or any(char.isspace() or ord(char) < 32 for char in value):
        raise ValueError("请输入有效的 HTTP / HTTPS 地址，不支持带账号密码的地址")
    return value


class LinkFields(BaseModel):
    original_url: str = Field(min_length=1, max_length=2048)
    title: str = Field(default="", max_length=120)
    note: str = Field(default="", max_length=2000)
    tags: list[str] = Field(default_factory=list, max_length=20)

    @field_validator("original_url")
    @classmethod
    def valid_url(cls, value):
        normalized = normalize_url(value)
        if len(normalized) > 2048:
            raise ValueError("链接不能超过 2048 个字符")
        return normalized

    @field_validator("tags")
    @classmethod
    def valid_tags(cls, values):
        values = list(
            dict.fromkeys(
                v.strip().removeprefix("#")
                for v in values
                if v.strip().removeprefix("#")
            )
        )
        if any(len(v) > 40 or any(ord(c) < 32 for c in v) for v in values):
            raise ValueError("标签不能超过 40 个字符或包含控制字符")
        return values

    @field_validator("title", "note")
    @classmethod
    def trim_text(cls, value):
        return value.strip()


class ShortLinkCreate(LinkFields):
    short_code: str | None = None

    @field_validator("short_code")
    @classmethod
    def valid_code(cls, value):
        value = (value or "").strip()
        if not value:
            return None
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,32}", value):
            raise ValueError("短码需为 1–32 位英文字母、数字、横线或下划线")
        return value


class ShortLinkUpdate(LinkFields):
    pass


class ShortLinkResponse(LinkFields):
    model_config = ConfigDict(from_attributes=True)
    id: int
    short_code: str
    short_url: str
    is_custom: bool
    created_at: datetime
    visit_count: int = 0


class ShortLinkListResponse(BaseModel):
    total: int
    items: list[ShortLinkResponse]


class VisitStatsResponse(BaseModel):
    short_code: str
    short_url: str
    original_url: str
    total_visits: int
    visits: list[dict]


class AdminLoginRequest(BaseModel):
    password: str = Field(max_length=1024)


class AdminLoginResponse(BaseModel):
    token: str
    expires_at: int


class SiteSettings(BaseModel):
    app_name: str = Field(default="Link WithU", min_length=1, max_length=60)
    base_url: str = Field(default="http://localhost:5201", max_length=2048)
    timezone: str = "Asia/Shanghai"
    random_code_length: int = Field(default=6, ge=4, le=16)

    @field_validator("app_name")
    @classmethod
    def valid_name(cls, value):
        if not value.strip():
            raise ValueError("站点名称不能为空")
        return value.strip()

    @field_validator("base_url")
    @classmethod
    def valid_base(cls, value):
        value = normalize_url(value).rstrip("/")
        if len(value) > 2048:
            raise ValueError("公开地址不能超过 2048 个字符")
        parsed = urlsplit(value)
        if parsed.path or parsed.query or parsed.fragment:
            raise ValueError("公开地址只包含协议、域名和端口")
        return value

    @field_validator("timezone")
    @classmethod
    def valid_timezone(cls, value):
        try:
            ZoneInfo(value)
        except (ZoneInfoNotFoundError, ValueError):
            raise ValueError("请输入有效时区，例如 Asia/Shanghai")
        return value


class BackupLink(LinkFields):
    id: int = Field(gt=0, le=SQLITE_MAX_ID)
    short_code: str = Field(min_length=1)
    is_custom: bool
    created_at: datetime

    @field_validator("short_code")
    @classmethod
    def valid_code(cls, value):
        # Earlier versions accepted Unicode letters and digits in custom codes.
        if (
            not value
            or len(value) > 32
            or not all(c.isalnum() or c in "-_" for c in value)
        ):
            raise ValueError("备份包含无效短码")
        return value

    @field_validator("created_at")
    @classmethod
    def utc_naive(cls, value):
        return value.astimezone(UTC).replace(tzinfo=None) if value.tzinfo else value


class BackupVisit(BaseModel):
    id: int = Field(gt=0, le=SQLITE_MAX_ID)
    short_link_id: int = Field(gt=0, le=SQLITE_MAX_ID)
    visited_at: datetime
    ip: str | None = Field(default=None, max_length=45)
    user_agent: str | None = Field(default=None, max_length=8192)
    referrer_domain: str | None = Field(default=None, max_length=253)
    device: str = Field(default="unknown", max_length=20)
    browser: str = Field(default="unknown", max_length=30)
    is_bot: bool = False

    @field_validator("visited_at")
    @classmethod
    def utc_naive(cls, value):
        return value.astimezone(UTC).replace(tzinfo=None) if value.tzinfo else value


class BackupData(BaseModel):
    format: str = Field(pattern=r"^link-withu-backup$")
    version: int = Field(ge=1, le=1)
    exported_at: datetime
    settings: SiteSettings
    links: list[BackupLink]
    visits: list[BackupVisit]
