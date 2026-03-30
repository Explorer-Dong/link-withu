# build frontend
FROM node:20-alpine AS FE
WORKDIR /code/frontend
COPY frontend/ ./
RUN npm install
RUN npm run build

# run backend
FROM python:3.11-alpine
WORKDIR /code/backend
ENV PYTHONUNBUFFERED=1
ENV FRONTEND_DIST=/code/frontend/dist
COPY --from=FE /code/frontend/dist ../frontend/dist
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
COPY backend/ ./
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --no-dev --no-install-project --no-editable --locked
EXPOSE 5201
CMD ["uv", "run", "fastapi", "run", "app/main.py", "--port", "5201"]
