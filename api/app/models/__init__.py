"""数据模型"""

from app.models.short_link import AppSetting, LinkTag, ShortLink, Visit  # noqa: F401

__all__ = ["ShortLink", "Visit"]
