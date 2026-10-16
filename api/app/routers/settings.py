"""Site configuration and validated, transactional portable backup/restore."""

import json
from datetime import UTC, datetime
from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException, Request
from fastapi.responses import Response
from pydantic import ValidationError
from sqlalchemy import delete, func, insert, select
from sqlalchemy.ext.asyncio import AsyncSession

from app import __version__
from app.config import settings
from app.database import engine, get_db
from app.models import AppSetting, LinkTag, ShortLink, Visit
from app.schemas.link import BackupData, SiteSettings
from app.services.admin_auth import verify_admin_token
from app.services.site_settings import get_site_settings, save_site_settings

router = APIRouter(
    prefix="/api/admin", tags=["settings"], dependencies=[Depends(verify_admin_token)]
)
MAX_BACKUP_BYTES = 50 * 1024 * 1024


@router.get("/settings")
async def read_settings(db: AsyncSession = Depends(get_db, scope="function")):
    path = engine.url.database
    size = Path(path).stat().st_size if path and Path(path).is_file() else 0
    return {
        "site": (await get_site_settings(db)).model_dump(),
        "version": __version__,
        "database_bytes": size,
        "trusted_proxy_count": settings.trusted_proxy_count,
        "max_restore_bytes": MAX_BACKUP_BYTES,
        "links": await db.scalar(select(func.count(ShortLink.id))),
        "visits": await db.scalar(select(func.count(Visit.id))),
    }


@router.put("/settings", response_model=SiteSettings)
async def update_settings(
    body: SiteSettings, db: AsyncSession = Depends(get_db, scope="function")
):
    await save_site_settings(db, body)
    return await get_site_settings(db)


@router.get("/backup")
async def backup(db: AsyncSession = Depends(get_db, scope="function")):
    # get_db holds one read snapshot across links, tags, visits, and settings.
    site = await get_site_settings(db)
    now = datetime.now(UTC)
    data = {
        "format": "link-withu-backup",
        "version": 1,
        "exported_at": now.isoformat(),
        "settings": site.model_dump(),
    }
    content = bytearray()

    def append(value):
        chunk = value.encode("utf-8")
        if len(content) + len(chunk) > MAX_BACKUP_BYTES:
            raise HTTPException(413, "便携备份超过 50 MiB，请停机备份挂载的 data 目录")
        content.extend(chunk)

    def encode(value):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))

    append(encode(data)[:-1] + ',"links":[')
    first = True
    links = await db.stream_scalars(
        select(ShortLink).order_by(ShortLink.id).execution_options(yield_per=500)
    )
    async for link in links:
        append(
            ("" if first else ",")
            + encode(
                {
                    "id": link.id,
                    "short_code": link.short_code,
                    "original_url": link.original_url,
                    "title": link.title,
                    "note": link.note,
                    "tags": sorted(t.name for t in link.tags),
                    "is_custom": bool(link.is_custom),
                    "created_at": link.created_at.isoformat() + "Z",
                }
            )
        )
        first = False
    append('],"visits":[')
    first = True
    visits = await db.stream_scalars(
        select(Visit).order_by(Visit.id).execution_options(yield_per=500)
    )
    async for visit in visits:
        append(
            ("" if first else ",")
            + encode(
                {
                    "id": visit.id,
                    "short_link_id": visit.short_link_id,
                    "visited_at": visit.visited_at.isoformat() + "Z",
                    "ip": visit.ip,
                    "user_agent": visit.user_agent,
                    "referrer_domain": visit.referrer_domain,
                    "device": visit.device,
                    "browser": visit.browser,
                    "is_bot": bool(visit.is_bot),
                }
            )
        )
        first = False
    append("]}")
    return Response(
        bytes(content),
        media_type="application/json",
        headers={
            "Content-Disposition": f'attachment; filename="link-withu-{now:%Y%m%d-%H%M%S}.json"',
            "Cache-Control": "no-store",
        },
    )


@router.post("/restore")
async def restore(
    request: Request, db: AsyncSession = Depends(get_db, scope="function")
):
    if request.headers.get("x-confirm-restore") != "RESTORE":
        raise HTTPException(400, "请确认覆盖恢复操作")
    content = bytearray()
    async for chunk in request.stream():
        content.extend(chunk)
        if len(content) > MAX_BACKUP_BYTES:
            raise HTTPException(413, "恢复文件不能超过 50 MiB")
    try:
        body = BackupData.model_validate_json(content)
    except (ValidationError, ValueError):
        raise HTTPException(400, "备份格式无效或版本不兼容，现有数据未修改")
    ids = {link.id for link in body.links}
    codes = {link.short_code for link in body.links}
    if (
        len(ids) != len(body.links)
        or len(codes) != len(body.links)
        or len({v.id for v in body.visits}) != len(body.visits)
    ):
        raise HTTPException(400, "备份包含重复 ID 或短码，现有数据未修改")
    if any(visit.short_link_id not in ids for visit in body.visits):
        raise HTTPException(400, "备份存在无对应短链的访问记录，现有数据未修改")
    # All validation precedes deletion; get_db commits the entire replacement atomically.
    for model in [Visit, LinkTag, ShortLink, AppSetting]:
        await db.execute(delete(model))

    async def batches(model, rows):
        for start in range(0, len(rows), 500):
            await db.execute(insert(model), rows[start : start + 500])

    await batches(ShortLink, [link.model_dump(exclude={"tags"}) for link in body.links])
    await batches(
        LinkTag,
        [
            {"short_link_id": link.id, "name": name}
            for link in body.links
            for name in link.tags
        ],
    )
    await batches(Visit, [visit.model_dump() for visit in body.visits])
    await save_site_settings(db, body.settings)
    return {"links": len(body.links), "visits": len(body.visits), "message": "恢复完成"}
