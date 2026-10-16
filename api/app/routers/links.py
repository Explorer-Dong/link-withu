"""Private link management. Redirects remain public."""

from fastapi import APIRouter, Depends, HTTPException, Query, Response
from sqlalchemy import exists, func, or_, select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.models import LinkTag, ShortLink, Visit
from app.schemas import (
    ShortLinkCreate,
    ShortLinkListResponse,
    ShortLinkResponse,
    ShortLinkUpdate,
    VisitStatsResponse,
)
from app.services import ShortCodeService
from app.services.admin_auth import verify_admin_token
from app.services.site_settings import get_site_settings, short_url

router = APIRouter(
    prefix="/api/links", tags=["links"], dependencies=[Depends(verify_admin_token)]
)


def response(link, site, count=0):
    return ShortLinkResponse(
        id=link.id,
        short_code=link.short_code,
        original_url=link.original_url,
        title=link.title,
        note=link.note,
        tags=sorted(t.name for t in link.tags),
        short_url=short_url(link.short_code, site),
        is_custom=bool(link.is_custom),
        created_at=link.created_at,
        visit_count=count,
    )


async def find_link(db, code):
    link = (
        await db.execute(select(ShortLink).where(ShortLink.short_code == code))
    ).scalar_one_or_none()
    if not link:
        raise HTTPException(404, "短链不存在")
    return link


@router.post("", response_model=ShortLinkResponse, status_code=201)
async def create_short_link(body: ShortLinkCreate, db: AsyncSession = Depends(get_db)):
    site = await get_site_settings(db)
    for _ in range(5):
        code = body.short_code or await ShortCodeService.get_available_random(
            db, site.random_code_length
        )
        link = ShortLink(
            short_code=code,
            original_url=body.original_url,
            title=body.title,
            note=body.note,
            is_custom=int(bool(body.short_code)),
            tags=[LinkTag(name=t) for t in body.tags],
        )
        try:
            async with db.begin_nested():
                db.add(link)
                await db.flush()
            return response(link, site)
        except IntegrityError:
            if body.short_code:
                raise HTTPException(409, "该短码已被使用")
    raise HTTPException(503, "生成短码失败，请重试")


@router.get("", response_model=ShortLinkListResponse)
async def list_short_links(
    db: AsyncSession = Depends(get_db),
    skip: int = Query(0, ge=0),
    limit: int = Query(10, ge=1, le=100),
    q: str = Query("", max_length=200),
    tag: str = Query("", max_length=40),
    sort: str = Query("newest", pattern="^(newest|oldest|visits)$"),
):
    filters = []
    if q.strip():
        match = (
            "%"
            + q.strip().replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")
            + "%"
        )
        filters.append(
            or_(
                ShortLink.title.ilike(match, escape="\\"),
                ShortLink.note.ilike(match, escape="\\"),
                ShortLink.original_url.ilike(match, escape="\\"),
                ShortLink.short_code.ilike(match, escape="\\"),
                exists().where(
                    LinkTag.short_link_id == ShortLink.id,
                    LinkTag.name.ilike(match, escape="\\"),
                ),
            )
        )
    if tag:
        filters.append(
            exists().where(LinkTag.short_link_id == ShortLink.id, LinkTag.name == tag)
        )
    count = (
        select(Visit.short_link_id, func.count(Visit.id).label("n"))
        .group_by(Visit.short_link_id)
        .subquery()
    )
    query = (
        select(ShortLink, func.coalesce(count.c.n, 0))
        .outerjoin(count, count.c.short_link_id == ShortLink.id)
        .where(*filters)
    )
    ordering = {
        "newest": ShortLink.id.desc(),
        "oldest": ShortLink.id.asc(),
        "visits": func.coalesce(count.c.n, 0).desc(),
    }
    rows = (
        await db.execute(
            query.order_by(ordering[sort], ShortLink.id.desc())
            .offset(skip)
            .limit(limit)
        )
    ).all()
    total = await db.scalar(select(func.count(ShortLink.id)).where(*filters))
    site = await get_site_settings(db)
    return {"total": total, "items": [response(link, site, n) for link, n in rows]}


@router.get("/tags")
async def list_tags(db: AsyncSession = Depends(get_db)):
    rows = (
        await db.execute(
            select(LinkTag.name, func.count(LinkTag.short_link_id))
            .group_by(LinkTag.name)
            .order_by(LinkTag.name)
        )
    ).all()
    return [{"name": name, "link_count": n} for name, n in rows]


@router.get("/stats/{short_code}", response_model=VisitStatsResponse)
async def get_visit_stats(
    short_code: str,
    db: AsyncSession = Depends(get_db),
    skip: int = Query(0, ge=0),
    limit: int = Query(50, ge=1, le=200),
    exclude_bots: bool = False,
):
    link = await find_link(db, short_code)
    filters = [Visit.short_link_id == link.id]
    if exclude_bots:
        filters.append(Visit.is_bot == 0)
    total = await db.scalar(select(func.count(Visit.id)).where(*filters))
    visits = (
        (
            await db.execute(
                select(Visit)
                .where(*filters)
                .order_by(Visit.visited_at.desc(), Visit.id.desc())
                .offset(skip)
                .limit(limit)
            )
        )
        .scalars()
        .all()
    )
    site = await get_site_settings(db)
    return {
        "short_code": link.short_code,
        "short_url": short_url(link.short_code, site),
        "original_url": link.original_url,
        "total_visits": total,
        "visits": [
            {
                "visited_at": v.visited_at.isoformat() + "Z",
                "ip": v.ip,
                "user_agent": v.user_agent,
                "referrer_domain": v.referrer_domain,
                "device": v.device,
                "browser": v.browser,
                "is_bot": bool(v.is_bot),
            }
            for v in visits
        ],
    }


@router.put("/{short_code}", response_model=ShortLinkResponse)
async def update_short_link(
    short_code: str, body: ShortLinkUpdate, db: AsyncSession = Depends(get_db)
):
    link = await find_link(db, short_code)
    link.original_url, link.title, link.note = body.original_url, body.title, body.note
    link.tags = [LinkTag(name=t) for t in body.tags]
    await db.flush()
    count = await db.scalar(
        select(func.count(Visit.id)).where(Visit.short_link_id == link.id)
    )
    return response(link, await get_site_settings(db), count)


@router.delete("/{short_code}", status_code=204)
async def delete_short_link(short_code: str, db: AsyncSession = Depends(get_db)):
    await db.delete(await find_link(db, short_code))
    await db.flush()
    return Response(status_code=204)
