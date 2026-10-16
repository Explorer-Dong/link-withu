from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import Depends, FastAPI, HTTPException
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from sqlalchemy.ext.asyncio import AsyncSession

from app import __version__
from app.config import settings
from app.database import get_db, init_db
from app.routers import admin, analytics, links, redirect
from app.routers import settings as settings_router
from app.services.site_settings import get_site_settings


@asynccontextmanager
async def lifespan(app: FastAPI):
    await init_db()
    yield


app = FastAPI(title=settings.app_name, version=__version__, lifespan=lifespan)
for router in [
    admin.router,
    links.router,
    analytics.router,
    settings_router.router,
    redirect.router,
]:
    app.include_router(router)


@app.get("/api/health")
def health():
    return {"status": "ok", "version": __version__}


@app.get("/api/site")
async def site_info(db: AsyncSession = Depends(get_db, scope="function")):
    site = await get_site_settings(db)
    return {"app_name": site.app_name, "version": __version__}


if settings.frontend_dist and settings.frontend_dist.is_dir():
    frontend_dist: Path = settings.frontend_dist
    assets = frontend_dist / "assets"
    if assets.is_dir():
        app.mount("/assets", StaticFiles(directory=assets), name="assets")

    @app.get("/favicon.svg", include_in_schema=False)
    def favicon():
        return FileResponse(frontend_dist / "favicon.svg")

    @app.get("/{path:path}", include_in_schema=False)
    def frontend(path: str):
        # Only known client routes receive the SPA shell; invalid APIs stay 404.
        if path not in {"", "login", "links", "analytics", "settings"}:
            raise HTTPException(404, "页面不存在")
        return FileResponse(frontend_dist / "index.html")
