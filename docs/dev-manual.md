# 开发手册

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

dry run:

```bash
docker build --target debug-context --no-cache --progress=plain .
```

预览构建：

```bash
docker compose -f docker-compose.dev.yml up -d --build
```

生产构建：

```bash
docker build -t link-withu:<v0.1.0> -t link-withu:latest .
```
