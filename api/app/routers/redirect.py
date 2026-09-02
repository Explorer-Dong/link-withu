"""重定向：短链跳转（根路径 /r/{short_code}）"""
from fastapi import APIRouter, Depends, HTTPException, Request
from fastapi.responses import RedirectResponse
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import get_db
from app.models import ShortLink, Visit
from app.services.client_ip import get_client_ip

router = APIRouter(tags=["redirect"])


@router.get("/r/{short_code}")
async def redirect(
    short_code: str,
    request: Request,
    db: AsyncSession = Depends(get_db),
):
    """根据短码跳转到原始 URL，并记录访问"""
    result = await db.execute(
        select(ShortLink).where(ShortLink.short_code == short_code)
    )
    link = result.scalar_one_or_none()
    if not link:
        raise HTTPException(status_code=404, detail="短链不存在")

    visit = Visit(
        short_link_id=link.id,
        ip=get_client_ip(request),
        user_agent=request.headers.get("user-agent"),
    )
    db.add(visit)
    await db.flush()

    return RedirectResponse(url=link.original_url, status_code=302)
