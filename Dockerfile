# build web frontend
FROM node:24-alpine AS web
WORKDIR /proj/web
COPY web/ ./
RUN npm i
RUN npm run build

# run api backend
FROM python:3.13-alpine
WORKDIR /proj/api
ENV PYTHONUNBUFFERED=1
ENV FRONTEND_DIST=/proj/web/dist
ENV DATABASE_URL=sqlite+aiosqlite:////proj/data/short_links.db
RUN mkdir -p /proj/data
COPY --from=web /proj/web/dist ../web/dist
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
COPY api/ ./
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --no-dev --no-install-project --no-editable --locked
EXPOSE 5201
CMD ["uv", "run", "fastapi", "run", "app/main.py", "--port", "5201"]
