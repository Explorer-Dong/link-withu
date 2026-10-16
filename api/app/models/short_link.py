"""短链与访问记录模型"""

from datetime import datetime, timezone

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Text
from sqlalchemy.orm import relationship

from app.database import Base


def utcnow():
    return datetime.now(timezone.utc).replace(tzinfo=None)


class ShortLink(Base):
    """短链"""

    __tablename__ = "short_links"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    short_code = Column(
        String(32), unique=True, index=True, nullable=False, comment="短码"
    )
    original_url = Column(String(2048), nullable=False, comment="原始 URL")
    title = Column(String(120), nullable=False, default="", server_default="")
    note = Column(String(2000), nullable=False, default="", server_default="")
    is_custom = Column(Integer, default=0, comment="是否自定义短码 0=随机 1=自定义")
    created_at = Column(DateTime, default=utcnow)

    visits = relationship(
        "Visit", back_populates="short_link", cascade="all, delete-orphan"
    )
    tags = relationship("LinkTag", cascade="all, delete-orphan", lazy="selectin")


class Visit(Base):
    """访问记录（用于统计）"""

    __tablename__ = "visits"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    short_link_id = Column(
        Integer,
        ForeignKey("short_links.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
    )
    visited_at = Column(DateTime, default=utcnow, index=True)
    ip = Column(String(45), nullable=True, comment="IP")
    user_agent = Column(Text, nullable=True, comment="User-Agent")
    referrer_domain = Column(String(253), nullable=True)
    device = Column(
        String(20), nullable=False, default="unknown", server_default="unknown"
    )
    browser = Column(
        String(30), nullable=False, default="unknown", server_default="unknown"
    )
    is_bot = Column(Integer, nullable=False, default=0, server_default="0", index=True)

    short_link = relationship("ShortLink", back_populates="visits")


class LinkTag(Base):
    __tablename__ = "link_tags"

    short_link_id = Column(
        Integer, ForeignKey("short_links.id", ondelete="CASCADE"), primary_key=True
    )
    name = Column(String(40), primary_key=True, index=True)


class AppSetting(Base):
    __tablename__ = "app_settings"

    key = Column(String(40), primary_key=True)
    value = Column(Text, nullable=False)
