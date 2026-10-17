# 开发手册

一键启动容器化开发环境：

```bash
docker compose -f docker-compose.dev.yml up --build
```

- 前端 <http://127.0.0.1:5173>，支持热重载。
- 后端 <http://127.0.0.1:5201>，通过 `cargo-watch` 自动编译和重启。接口文档 <http://127.0.0.1:5201/docs>。

后端使用 Rust、Axum 和 SQLite，前端使用 Vue。开发容器将 Cargo 依赖和编译产物保存在独立卷中，数据库仍保存在 `api/short_links.db`。

## 本地运行

安装 Rust 后，项目会按 `rust-toolchain.toml` 选择工具链。在项目根目录配置 `.env`，先构建前端，再启动后端：

```bash
npm ci --prefix web
npm run build --prefix web
cargo run --manifest-path api/Cargo.toml --locked
```

本地后端读取项目根目录和 `api/` 下的 `.env`，环境变量优先。默认端口为 5201，可通过 `PORT` 修改。

## 检查与测试

```bash
cargo fmt --manifest-path api/Cargo.toml -- --check
cargo clippy --manifest-path api/Cargo.toml --all-targets --locked -- -D warnings
npm run format:check --prefix web
npm run build --prefix web
cargo test --manifest-path api/Cargo.toml --locked
npm exec --prefix web -- playwright install chromium
npm run test:e2e --prefix web
python3 -m unittest discover -s .github/scripts -p 'test_*.py'
```

后端测试会启动独立服务并使用临时数据库，覆盖 API、旧库升级、并发写入和恢复回滚。浏览器测试覆盖桌面端和手机端；运行前需构建前端。

功能与用例的对应关系见 [测试覆盖](test-coverage.md)。新增功能需同时维护该清单及正常、失败路径的测试。

构建生产镜像：

```bash
docker build -t link-withu:v1.0.1 .
```

生产镜像只运行 Rust 服务。SQLite 表结构、数据挂载路径和 JSON 备份格式与旧版兼容；恢复时先逐条校验到磁盘暂存库，通过后再原子替换正式数据。

## 发布约定

Git 标签、GitHub Release 名称和 Docker 版本标签统一使用 `vMAJOR.MINOR.PATCH`（例如 `v1.0.1`）。`api/Cargo.toml`、`api/Cargo.lock`、`web/package.json` 和 `web/package-lock.json` 中的包版本保留标准数字格式，并同步更新。发布流程会先运行检查，再构建并验证 amd64、arm64 镜像。
