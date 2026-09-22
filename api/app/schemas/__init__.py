"""Pydantic 请求/响应模型"""
from app.schemas.link import (
    AdminLoginRequest,
    AdminLoginResponse,
    ShortLinkCreate,
    ShortLinkUpdate,
    ShortLinkResponse,
    ShortLinkListResponse,
    VisitStatsResponse,
)

__all__ = [
    "AdminLoginRequest",
    "AdminLoginResponse",
    "ShortLinkCreate",
    "ShortLinkUpdate",
    "ShortLinkResponse",
    "ShortLinkListResponse",
    "VisitStatsResponse",
]
