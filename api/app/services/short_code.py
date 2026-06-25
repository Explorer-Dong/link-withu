"""短码生成与校验服务"""

import random
import string

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import settings
from app.models import ShortLink


class ShortCodeService:
    """短码服务：随机生成、唯一性校验"""

    CHARS = string.ascii_letters + string.digits  # a-zA-Z0-9

    @classmethod
    def generate_random(cls, length: int | None = None) -> str:
        """生成随机短码"""
        length = length or settings.random_code_length
        return "".join(random.choices(cls.CHARS, k=length))

    @classmethod
    async def get_available_random(cls, db: AsyncSession, length: int | None = None) -> str:
        """生成一个未占用的随机短码"""
        for _ in range(50):  # 最多尝试 50 次
            code = cls.generate_random(length)
            exists = await cls.exists(db, code)
            if not exists:
                return code
        raise ValueError("无法生成唯一短码，请稍后重试")

    @classmethod
    async def exists(cls, db: AsyncSession, short_code: str) -> bool:
        """检查短码是否已存在"""
        result = await db.execute(select(ShortLink.id).where(ShortLink.short_code == short_code))
        return result.scalar_one_or_none() is not None
