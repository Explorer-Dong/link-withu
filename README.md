# 短链服务

短链生成（支持自定义短码或随机生成）与访问统计，技术栈：FastAPI + Vue 3 + SQLite。

## 功能

- **短链生成**：输入长链接，可填写自定义短码或留空自动随机生成
- **访问统计**：查看每个短链的访问次数与最近访问记录（时间、IP、User-Agent）
- **跳转**：访问 `/r/{短码}` 自动跳转到原始 URL 并记录一次访问

## 一键启动（Docker，端口 5201）

```bash
docker-compose up -d --build
```

访问：<http://localhost:5201>

- 首次构建会安装前端依赖并打包，可能需要几分钟。
- 数据持久化在 Docker volume `short-link-data`，数据库文件位于容器内 `/code/data/short_links.db`。

## 本地开发

### 方式一：只启动后端，挂载前端静态页（无需前端开发服务器）

先构建前端，再启动后端即可，后端会自动挂载 `frontend/dist` 目录：

```bash
# 1. 构建前端
cd frontend && npm install && npm run build && cd ..

# 2. 启动后端
cd backend
uv venv
uv sync
uv run uvicorn app.main:app --reload --port 5201
```

访问 <http://localhost:5201> 即可同时使用前端页面和后端 API。

### 方式二：前后端分离开发（支持热更新）

#### 后端

使用 [uv](https://docs.astral.sh/uv/) 管理 Python 虚拟环境（无需 pip/venv）：

```bash
cd backend
uv venv
uv sync
uv run uvicorn app.main:app --reload --port 5201
```

- 未安装 uv 时，可执行：`curl -LsSf https://astral.sh/uv/install.sh | sh`（Git Bash 下可用）。
- 生成并锁定依赖：`uv lock`（可选，便于 Docker 使用 `--locked` 构建）。

#### 前端

```bash
cd frontend
npm install
npm run dev
```

前端开发服务器默认 5173，已配置代理将 `/api` 和 `/r` 转发到 `http://localhost:5201`。

## API 说明

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/links` | 创建短链，body: `{ "original_url": "https://...", "short_code": "可选" }` |
| GET  | `/api/links` | 短链列表，query: `skip`, `limit` |
| GET  | `/api/links/stats/{short_code}` | 某短链的访问统计 |
| GET  | `/r/{short_code}` | 跳转到原始 URL（计一次访问） |
| GET  | `/api/health` | 健康检查 |

## 项目结构

```
├── backend/           # FastAPI 后端（uv 管理依赖）
│   ├── app/
│   │   ├── config.py
│   │   ├── database.py
│   │   ├── main.py
│   │   ├── models/
│   │   ├── routers/
│   │   ├── schemas/
│   │   └── services/
│   ├── pyproject.toml
│   └── uv.lock        # 可选，由 uv lock 生成
├── frontend/          # Vue 3 前端
├── Dockerfile         # 多阶段构建：前端 build + 后端 + 静态资源
├── docker-compose.yml # 一键启动，端口 5201
└── README.md
```

## 环境变量

- `BASE_URL`：短链前缀（如 `http://localhost:5201` 或生产域名），用于返回给前端的短链 URL。
- `DATABASE_URL`：数据库连接（默认 SQLite），Docker 中可设为 `sqlite+aiosqlite:////code/data/short_links.db` 以配合 volume。
- `FRONTEND_DIST`：前端构建产物目录路径（默认 `frontend/dist`），Docker 中自动设为 `/code/frontend/dist`。
