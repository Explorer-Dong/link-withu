# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:24-alpine AS web
WORKDIR /proj/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM python:3.13-slim
COPY --from=ghcr.io/astral-sh/uv:0.12.3 /uv /bin/uv
WORKDIR /proj/api
ENV PYTHONUNBUFFERED=1 \
    TZ=Asia/Shanghai \
    UV_LINK_MODE=copy \
    FRONTEND_DIST=/proj/web/dist \
    DATABASE_URL=sqlite+aiosqlite:////proj/data/short_links.db
COPY api/pyproject.toml api/uv.lock ./
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --frozen --no-dev --no-install-project
COPY api/app ./app
COPY --from=web /proj/web/dist /proj/web/dist
RUN mkdir -p /proj/data
LABEL org.opencontainers.image.title="Link WithU" \
      org.opencontainers.image.source="https://github.com/Explorer-Dong/link-withu" \
      org.opencontainers.image.licenses="MIT"
EXPOSE 5201
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/proj/api/.venv/bin/python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:5201/api/health', timeout=3)"]
CMD ["/proj/api/.venv/bin/uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "5201", "--no-proxy-headers"]
