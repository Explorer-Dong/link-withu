from sqlalchemy import event, text
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine
from sqlalchemy.orm import declarative_base

from app.config import settings

engine = create_async_engine(
    settings.database_url,
    echo=False,
)

AsyncSessionLocal = async_sessionmaker(
    engine,
    class_=AsyncSession,
    expire_on_commit=False,
    autocommit=False,
    autoflush=False,
)

Base = declarative_base()


@event.listens_for(engine.sync_engine, "connect")
def sqlite_setup(connection, _):
    if engine.dialect.name == "sqlite":
        cursor = connection.cursor()
        cursor.execute("PRAGMA foreign_keys=ON")
        cursor.execute("PRAGMA busy_timeout=10000")
        cursor.close()


async def get_db():
    """依赖：获取数据库会话"""
    async with AsyncSessionLocal() as session:
        try:
            yield session
            await session.commit()
        except Exception:
            await session.rollback()
            raise
        finally:
            await session.close()


async def init_db():
    """初始化数据库表"""
    async with engine.begin() as conn:
        # Upgrade pre-1.0 SQLite databases without losing links or visit history.
        if engine.dialect.name == "sqlite":
            await conn.execute(text("BEGIN IMMEDIATE"))
            tables = {
                row[0]
                for row in (
                    await conn.execute(
                        text("SELECT name FROM sqlite_master WHERE type='table'")
                    )
                )
            }
            additions = {
                "short_links": {
                    "title": "VARCHAR(120) NOT NULL DEFAULT ''",
                    "note": "VARCHAR(2000) NOT NULL DEFAULT ''",
                },
                "visits": {
                    "referrer_domain": "VARCHAR(253)",
                    "device": "VARCHAR(20) NOT NULL DEFAULT 'unknown'",
                    "browser": "VARCHAR(30) NOT NULL DEFAULT 'unknown'",
                    "is_bot": "INTEGER NOT NULL DEFAULT 0",
                },
            }
            backfill_visitors = False
            for table, fields in additions.items():
                if table not in tables:
                    continue
                existing = {
                    row[1]
                    for row in await conn.execute(text(f"PRAGMA table_info({table})"))
                }
                for name, definition in fields.items():
                    if name not in existing:
                        if table == "visits" and name == "device":
                            backfill_visitors = True
                        await conn.execute(
                            text(f"ALTER TABLE {table} ADD COLUMN {name} {definition}")
                        )
            if backfill_visitors:
                from app.services.visitor import classify_agent

                last_id = 0
                while True:
                    rows = (
                        await conn.execute(
                            text(
                                "SELECT id, user_agent FROM visits WHERE id > :last_id ORDER BY id LIMIT 500"
                            ),
                            {"last_id": last_id},
                        )
                    ).all()
                    if not rows:
                        break
                    await conn.execute(
                        text(
                            "UPDATE visits SET device=:device, browser=:browser, is_bot=:is_bot WHERE id=:visit_id"
                        ),
                        [
                            dict(visit_id=row.id, **classify_agent(row.user_agent))
                            for row in rows
                        ],
                    )
                    last_id = rows[-1].id
        await conn.run_sync(Base.metadata.create_all)
        if engine.dialect.name == "sqlite":
            await conn.execute(
                text(
                    "CREATE INDEX IF NOT EXISTS ix_visits_link_time ON visits(short_link_id, visited_at)"
                )
            )
            await conn.execute(
                text(
                    "CREATE INDEX IF NOT EXISTS ix_visits_visited_at ON visits(visited_at)"
                )
            )
            await conn.execute(
                text("CREATE INDEX IF NOT EXISTS ix_visits_is_bot ON visits(is_bot)")
            )
