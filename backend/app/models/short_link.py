"""短链与访问记录模型"""

from datetime import datetime

from sqlalchemy import Column, DateTime, ForeignKey, Integer, String, Text
from sqlalchemy.orm import relationship

from app.database import Base


class ShortLink(Base):
    """短链"""

    __tablename__ = "short_links"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    short_code = Column(String(32), unique=True, index=True, nullable=False, comment="短码")
    original_url = Column(String(2048), nullable=False, comment="原始 URL")
    is_custom = Column(Integer, default=0, comment="是否自定义短码 0=随机 1=自定义")
    created_at = Column(DateTime, default=datetime.utcnow)

    visits = relationship("Visit", back_populates="short_link", cascade="all, delete-orphan")


class Visit(Base):
    """访问记录（用于统计）"""

    __tablename__ = "visits"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    short_link_id = Column(
        Integer, ForeignKey("short_links.id", ondelete="CASCADE"), nullable=False
    )
    visited_at = Column(DateTime, default=datetime.utcnow)
    ip = Column(String(45), nullable=True, comment="IP")
    user_agent = Column(Text, nullable=True, comment="User-Agent")

    short_link = relationship("ShortLink", back_populates="visits")
