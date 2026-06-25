# CLAUDE.md

## Web

```bash
cd web
npm i
npm run dev
```

## API

```bash
cd api
uv venv
uv sync
source .venv/bin/activate
uv run uvicorn app.main:app --reload --port 5201
```

## Docker

```bash
docker compose -f docker-compose.dev.yml up -d --build
```
