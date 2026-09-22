"""短链相关 API"""
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.models import ShortLink, Visit
from app.schemas import ShortLinkCreate, ShortLinkUpdate, ShortLinkResponse, ShortLinkListResponse, VisitStatsResponse
from app.services import ShortCodeService
from app.services.admin_auth import verify_admin_token
from app.config import settings

router = APIRouter(prefix="/api/links", tags=["links"])


def _short_url(short_code: str) -> str:
    base = settings.base_url.rstrip("/")
    return f"{base}/r/{short_code}"


@router.post("", response_model=ShortLinkResponse)
async def create_short_link(
    body: ShortLinkCreate,
    db: AsyncSession = Depends(get_db),
):
    """创建短链（支持自定义短码或随机生成）"""
    if body.short_code:
        # 自定义短码：检查是否已存在
        code = body.short_code
        if await ShortCodeService.exists(db, code):
            raise HTTPException(status_code=400, detail="该短码已被使用")
        is_custom = 1
    else:
        code = await ShortCodeService.get_available_random(db)
        is_custom = 0

    link = ShortLink(
        short_code=code,
        original_url=body.original_url,
        is_custom=is_custom,
    )
    db.add(link)
    await db.flush()
    await db.refresh(link)
    return ShortLinkResponse(
        id=link.id,
        short_code=link.short_code,
        original_url=link.original_url,
        short_url=_short_url(link.short_code),
        is_custom=bool(link.is_custom),
        created_at=link.created_at,
        visit_count=0,
    )


@router.get("", response_model=ShortLinkListResponse)
async def list_short_links(
    db: AsyncSession = Depends(get_db),
    skip: int = Query(0, ge=0),
    limit: int = Query(20, ge=1, le=100),
):
    """短链列表（含访问次数）"""
    # 总数
    total_result = await db.execute(select(func.count(ShortLink.id)))
    total = total_result.scalar() or 0

    # 列表 + 每个 link 的访问次数
    result = await db.execute(
        select(ShortLink)
        .order_by(ShortLink.created_at.desc())
        .offset(skip)
        .limit(limit)
    )
    links = result.scalars().all()

    # 批量查访问次数
    link_ids = [l.id for l in links]
    count_result = await db.execute(
        select(Visit.short_link_id, func.count(Visit.id))
        .where(Visit.short_link_id.in_(link_ids))
        .group_by(Visit.short_link_id)
    )
    count_map = dict(count_result.all())

    items = [
        ShortLinkResponse(
            id=link.id,
            short_code=link.short_code,
            original_url=link.original_url,
            short_url=_short_url(link.short_code),
            is_custom=bool(link.is_custom),
            created_at=link.created_at,
            visit_count=count_map.get(link.id, 0),
        )
        for link in links
    ]
    return ShortLinkListResponse(total=total, items=items)


@router.get("/stats/{short_code}", response_model=VisitStatsResponse)
async def get_visit_stats(
    short_code: str,
    db: AsyncSession = Depends(get_db),
    _: None = Depends(verify_admin_token),
):
    """获取某短链的访问统计"""
    result = await db.execute(
        select(ShortLink).where(ShortLink.short_code == short_code)
    )
    link = result.scalar_one_or_none()
    if not link:
        raise HTTPException(status_code=404, detail="短链不存在")

    total_result = await db.execute(
        select(func.count(Visit.id)).where(Visit.short_link_id == link.id)
    )
    total_visits = total_result.scalar() or 0

    result = await db.execute(
        select(Visit)
        .where(Visit.short_link_id == link.id)
        .order_by(Visit.visited_at.desc())
        .limit(500)
    )
    visits = result.scalars().all()

    return VisitStatsResponse(
        short_code=link.short_code,
        short_url=_short_url(link.short_code),
        original_url=link.original_url,
        total_visits=total_visits,
        visits=[
            {
                "visited_at": v.visited_at.isoformat() if v.visited_at else None,
                "ip": v.ip,
                "user_agent": (v.user_agent or "")[:200],
            }
            for v in visits
        ],
    )


@router.put("/{short_code}", response_model=ShortLinkResponse)
async def update_short_link(
    short_code: str,
    body: ShortLinkUpdate,
    db: AsyncSession = Depends(get_db),
    _: None = Depends(verify_admin_token),
):
    """修改短链（仅支持修改原始链接，短码不可改）"""
    result = await db.execute(select(ShortLink).where(ShortLink.short_code == short_code))
    link = result.scalar_one_or_none()
    if not link:
        raise HTTPException(status_code=404, detail="短链不存在")

    link.original_url = body.original_url
    await db.flush()
    await db.refresh(link)

    count_result = await db.execute(
        select(func.count(Visit.id)).where(Visit.short_link_id == link.id)
    )
    return ShortLinkResponse(
        id=link.id,
        short_code=link.short_code,
        original_url=link.original_url,
        short_url=_short_url(link.short_code),
        is_custom=bool(link.is_custom),
        created_at=link.created_at,
        visit_count=count_result.scalar() or 0,
    )


@router.delete("/{short_code}", status_code=204)
async def delete_short_link(
    short_code: str,
    db: AsyncSession = Depends(get_db),
    _: None = Depends(verify_admin_token),
):
    result = await db.execute(select(ShortLink).where(ShortLink.short_code == short_code))
    link = result.scalar_one_or_none()
    if not link:
        raise HTTPException(status_code=404, detail="短链不存在")
    await db.delete(link)

