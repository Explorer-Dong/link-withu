"""Pydantic 请求/响应模型"""
from app.schemas.link import (
    ShortLinkCreate,
    ShortLinkResponse,
    ShortLinkListResponse,
    VisitStatsResponse,
)

__all__ = [
    "ShortLinkCreate",
    "ShortLinkResponse",
    "ShortLinkListResponse",
    "VisitStatsResponse",
]
