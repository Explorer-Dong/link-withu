"""Pydantic 请求/响应模型"""

from app.schemas.link import (
    AdminLoginRequest,
    AdminLoginResponse,
    ShortLinkCreate,
    ShortLinkListResponse,
    ShortLinkResponse,
    ShortLinkUpdate,
    VisitStatsResponse,
)

__all__ = [
    "AdminLoginRequest",
    "AdminLoginResponse",
    "ShortLinkCreate",
    "ShortLinkListResponse",
    "ShortLinkResponse",
    "ShortLinkUpdate",
    "VisitStatsResponse",
]
