"""SQL aggregates over all matching visits, with timezone-aware calendar boundaries."""

import csv
import io
from datetime import UTC, datetime, time, timedelta
from math import ceil
from zoneinfo import ZoneInfo

from fastapi import APIRouter, Depends, HTTPException, Query
from fastapi.responses import StreamingResponse
from sqlalchemy import case, exists, func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.database import AsyncSessionLocal, get_db
from app.models import LinkTag, ShortLink, Visit
from app.services.admin_auth import verify_admin_token
from app.services.site_settings import get_site_settings

router = APIRouter(
    prefix="/api/analytics",
    tags=["analytics"],
    dependencies=[Depends(verify_admin_token)],
)


def link_scope(code="", tag=""):
    filters = []
    if code:
        filters.append(ShortLink.short_code == code)
    if tag:
        filters.append(
            exists()
            .where(LinkTag.short_link_id == ShortLink.id, LinkTag.name == tag)
            .correlate(ShortLink)
        )
    return filters


def calendar(site, days):
    zone = ZoneInfo(site.timezone)
    now = datetime.now(UTC)
    today = now.astimezone(zone).date()
    start = today - timedelta(days=days - 1)
    dates = [start + timedelta(days=i) for i in range(days)]

    def midnight(day):
        return (
            datetime.combine(day, time.min, zone).astimezone(UTC).replace(tzinfo=None)
        )

    return now.replace(tzinfo=None), today, dates, midnight


@router.get("")
async def analytics(
    db: AsyncSession = Depends(get_db),
    days: int = Query(0, ge=0, le=90),
    short_code: str = Query("", max_length=32),
    tag: str = Query("", max_length=40),
    exclude_bots: bool = False,
):
    site = await get_site_settings(db)
    now, today, dates, midnight = calendar(site, days or 1)
    scoped = link_scope(short_code, tag)
    if short_code and not await db.scalar(
        select(ShortLink.id).where(ShortLink.short_code == short_code)
    ):
        raise HTTPException(404, "短链不存在")
    common = [*scoped, Visit.visited_at <= now]
    if exclude_bots:
        common.append(Visit.is_bot == 0)
    bucket_days = 1
    if days == 0:
        first_visit = await db.scalar(
            select(func.min(Visit.visited_at))
            .join(ShortLink)
            .where(*scoped, Visit.visited_at <= now)
        )
        first_day = (
            first_visit.replace(tzinfo=UTC).astimezone(ZoneInfo(site.timezone)).date()
            if first_visit
            else today
        )
        # Bound query size and chart points, while aggregating every historical visit.
        span_days = (today - first_day).days + 1
        bucket_days = max(1, ceil(span_days / 90))
        dates = [
            first_day + timedelta(days=i) for i in range(0, span_days, bucket_days)
        ]
    start = midnight(dates[0])
    query = (
        select(Visit)
        .join(ShortLink, Visit.short_link_id == ShortLink.id)
        .where(*common)
    )
    period = [*common, Visit.visited_at >= start]
    counters = (
        await db.execute(
            select(
                func.count(Visit.id),
                func.coalesce(
                    func.sum(case((Visit.visited_at >= midnight(today), 1), else_=0)), 0
                ),
                func.coalesce(
                    func.sum(
                        case(
                            (
                                Visit.visited_at >= midnight(today - timedelta(days=6)),
                                1,
                            ),
                            else_=0,
                        )
                    ),
                    0,
                ),
                func.coalesce(
                    func.sum(
                        case(
                            (
                                Visit.visited_at
                                >= midnight(today - timedelta(days=29)),
                                1,
                            ),
                            else_=0,
                        )
                    ),
                    0,
                ),
            )
            .select_from(Visit)
            .join(ShortLink)
            .where(*common)
        )
    ).one()
    day_bucket = case(
        *[
            (
                (Visit.visited_at >= midnight(day))
                & (
                    Visit.visited_at
                    < midnight(
                        min(
                            day + timedelta(days=bucket_days), today + timedelta(days=1)
                        )
                    )
                ),
                day.isoformat(),
            )
            for day in dates
        ],
        else_=None,
    )
    trend = dict(
        (
            await db.execute(
                select(day_bucket.label("day"), func.count(Visit.id))
                .select_from(Visit)
                .join(ShortLink)
                .where(*period)
                .group_by("day")
            )
        ).all()
    )

    async def breakdown(column, unknown):
        key = func.coalesce(column, unknown)
        rows = (
            await db.execute(
                select(key, func.count(Visit.id))
                .select_from(Visit)
                .join(ShortLink)
                .where(*period)
                .group_by(key)
                .order_by(func.count(Visit.id).desc(), key)
                .limit(12)
            )
        ).all()
        return [{"name": name, "count": n} for name, n in rows]

    ranked = (
        await db.execute(
            select(
                ShortLink.short_code,
                ShortLink.title,
                ShortLink.original_url,
                func.count(Visit.id).label("n"),
            )
            .join(Visit)
            .where(*period)
            .group_by(ShortLink.id)
            .order_by(func.count(Visit.id).desc(), ShortLink.id.desc())
            .limit(10)
        )
    ).all()
    per_link = (
        query.where(Visit.visited_at >= start)
        .with_only_columns(Visit.short_link_id, func.count(Visit.id).label("n"))
        .group_by(Visit.short_link_id)
        .subquery()
    )
    groups = (
        await db.execute(
            select(
                LinkTag.name,
                func.count(LinkTag.short_link_id),
                func.coalesce(func.sum(per_link.c.n), 0),
            )
            .join(ShortLink, ShortLink.id == LinkTag.short_link_id)
            .outerjoin(per_link, per_link.c.short_link_id == ShortLink.id)
            .where(*scoped)
            .group_by(LinkTag.name)
            .order_by(func.coalesce(func.sum(per_link.c.n), 0).desc(), LinkTag.name)
        )
    ).all()
    bot_count = await db.scalar(
        select(func.count(Visit.id))
        .join(ShortLink)
        .where(
            *scoped,
            Visit.visited_at >= start,
            Visit.visited_at <= now,
            Visit.is_bot == 1,
        )
    )
    period_total = sum(trend.values())
    return {
        "timezone": site.timezone,
        "days": days,
        "bucket_days": bucket_days,
        "range_start": dates[0].isoformat(),
        "range_end": today.isoformat(),
        "exclude_bots": exclude_bots,
        "summary": {
            "links": await db.scalar(select(func.count(ShortLink.id)).where(*scoped)),
            "total_visits": counters[0],
            "today": counters[1],
            "last_7_days": counters[2],
            "last_30_days": counters[3],
            "period_visits": period_total,
            "suspected_bots": bot_count,
        },
        "trend": [
            {
                "date": d.isoformat(),
                "end_date": min(d + timedelta(days=bucket_days - 1), today).isoformat(),
                "count": trend.get(d.isoformat(), 0),
            }
            for d in dates
        ],
        "top_links": [
            {"short_code": code, "title": title, "original_url": url, "count": n}
            for code, title, url, n in ranked
        ],
        "referrers": await breakdown(Visit.referrer_domain, "直接访问 / 未知"),
        "devices": await breakdown(Visit.device, "unknown"),
        "browsers": await breakdown(Visit.browser, "unknown"),
        "tags": [
            {"name": name, "link_count": links, "count": n} for name, links, n in groups
        ],
    }


def csv_row(values):
    output = io.StringIO(newline="")
    # Spreadsheet applications must treat user-controlled values as text.
    safe = []
    for value in values:
        value = "" if value is None else str(value)
        if value.lstrip().startswith(("=", "+", "-", "@")) or value.startswith(
            ("\t", "\r", "\n")
        ):
            value = "'" + value
        safe.append(value)
    csv.writer(output).writerow(safe)
    return output.getvalue()


@router.get("/export")
async def export_csv(
    db: AsyncSession = Depends(get_db),
    kind: str = Query("visits", pattern="^(visits|links)$"),
    days: int | None = Query(None, ge=0, le=90),
    short_code: str = Query("", max_length=32),
    tag: str = Query("", max_length=40),
    exclude_bots: bool = False,
):
    filters = link_scope(short_code, tag)
    if kind == "links":
        query = select(ShortLink).where(*filters).order_by(ShortLink.id)
    else:
        filters.append(Visit.visited_at <= datetime.now(UTC).replace(tzinfo=None))
        if days:
            site = await get_site_settings(db)
            _, _, dates, midnight = calendar(site, days)
            filters.append(Visit.visited_at >= midnight(dates[0]))
        if exclude_bots:
            filters.append(Visit.is_bot == 0)
        query = (
            select(Visit, ShortLink.short_code)
            .join(ShortLink)
            .where(*filters)
            .order_by(Visit.id)
        )

    async def generate():
        yield "\ufeff"
        async with AsyncSessionLocal() as session:
            if kind == "links":
                yield csv_row(
                    [
                        "short_code",
                        "original_url",
                        "title",
                        "note",
                        "tags",
                        "created_at_utc",
                    ]
                )
                rows = await session.stream_scalars(
                    query.execution_options(yield_per=500)
                )
                async for link in rows:
                    yield csv_row(
                        [
                            link.short_code,
                            link.original_url,
                            link.title,
                            link.note,
                            ";".join(t.name for t in link.tags),
                            link.created_at.isoformat() + "Z",
                        ]
                    )
            else:
                yield csv_row(
                    [
                        "short_code",
                        "visited_at_utc",
                        "ip",
                        "referrer_domain",
                        "device",
                        "browser",
                        "suspected_bot",
                        "user_agent",
                    ]
                )
                rows = await session.stream(query.execution_options(yield_per=500))
                async for visit, code in rows:
                    yield csv_row(
                        [
                            code,
                            visit.visited_at.isoformat() + "Z",
                            visit.ip,
                            visit.referrer_domain,
                            visit.device,
                            visit.browser,
                            visit.is_bot,
                            visit.user_agent,
                        ]
                    )

    return StreamingResponse(
        generate(),
        media_type="text/csv; charset=utf-8",
        headers={
            "Content-Disposition": f'attachment; filename="link-withu-{kind}.csv"',
            "Cache-Control": "no-store",
        },
    )
