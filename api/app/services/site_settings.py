from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import settings
from app.models import AppSetting
from app.schemas.link import SiteSettings


async def get_site_settings(db: AsyncSession) -> SiteSettings:
    defaults = SiteSettings(
        app_name=settings.app_name,
        base_url=settings.base_url,
        random_code_length=settings.random_code_length,
    )
    values = dict((await db.execute(select(AppSetting.key, AppSetting.value))).all())
    # TZ is deployment configuration; ignore timezone values in older databases.
    return SiteSettings.model_validate(
        {**defaults.model_dump(), **values, "timezone": settings.tz}
    )


async def save_site_settings(db: AsyncSession, value: SiteSettings):
    # Backups retain timezone as metadata, but restores must use the current TZ.
    for key, item in value.model_dump(exclude={"timezone"}).items():
        row = await db.get(AppSetting, key)
        if row:
            row.value = str(item)
        else:
            db.add(AppSetting(key=key, value=str(item)))
    await db.flush()


def short_url(code: str, site: SiteSettings) -> str:
    return f"{site.base_url}/r/{code}"
