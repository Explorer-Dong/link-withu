"""短链相关 Schema"""
from datetime import datetime
from pydantic import BaseModel, field_validator


def _normalize_url(v: str) -> str:
    v = v.strip()
    if not v:
        raise ValueError("URL 不能为空")
    if not (v.startswith("http://") or v.startswith("https://")):
        v = "https://" + v
    return v


class ShortLinkCreate(BaseModel):
    """创建短链请求"""
    original_url: str
    short_code: str | None = None  # 不传则随机生成

    @field_validator("original_url")
    @classmethod
    def url_must_be_valid(cls, v: str) -> str:
        return _normalize_url(v)

    @field_validator("short_code")
    @classmethod
    def code_format(cls, v: str | None) -> str | None:
        if v is None or v == "":
            return None
        v = v.strip()
        if not v:
            return None
        if len(v) > 32:
            raise ValueError("短码长度不能超过 32")
        if not all(c.isalnum() or c in "-_" for c in v):
            raise ValueError("短码只能包含字母、数字、横线、下划线")
        return v


class ShortLinkUpdate(BaseModel):
    """修改短链请求（仅支持修改原始链接，短码不可改）"""
    original_url: str

    @field_validator("original_url")
    @classmethod
    def url_must_be_valid(cls, v: str) -> str:
        return _normalize_url(v)


class ShortLinkResponse(BaseModel):
    """短链响应"""
    id: int
    short_code: str
    original_url: str
    short_url: str
    is_custom: bool
    created_at: datetime
    visit_count: int = 0

    class Config:
        from_attributes = True


class ShortLinkListResponse(BaseModel):
    """短链列表响应"""
    total: int
    items: list[ShortLinkResponse]


class VisitStatsResponse(BaseModel):
    """访问统计响应"""
    short_code: str
    short_url: str
    original_url: str
    total_visits: int
    visits: list[dict]  # [{visited_at, ip, user_agent}, ...]


class AdminLoginRequest(BaseModel):
    password: str


class AdminLoginResponse(BaseModel):
    token: str
    expires_at: int
