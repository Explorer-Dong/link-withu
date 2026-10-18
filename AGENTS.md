# 项目指令

- 保持轻量、自部署、单容器运行和本地统计。
- 后端使用 Rust、Axum、rusqlite 和 SQLite；前端使用 Vue 3、Vue Router 和 Vite。
- 开发、配置、测试及发布指令统一维护在本文件，不再依赖仓库中的 `docs/` 目录。
- README 保留产品介绍、部署方法和使用入口；功能、配置或命令变化时同步更新相关内容。
- 保留用户已有改动。未经明确要求，不推送 Git 分支或标签，不发布 GitHub Release 或 Docker Hub 镜像。
- 不提交 `.env`、密码、令牌、数据库、备份、编译产物或测试产物。

## 代码位置

- `api/src/main.rs`：服务入口、认证、数据库工作线程、请求调度及静态资源。
- `api/src/api.rs`、`analytics.rs`：业务接口、访问记录、统计及导出。
- `api/src/config.rs`、`validation.rs`：配置加载、字段校验及客户端分类。
- `api/src/db.rs`、`schema.sql`：数据库连接、表结构、迁移及旧数据回填。
- `api/src/restore.rs`：备份流式解析、暂存校验及覆盖恢复。
- `api/src/models.rs`、`openapi.rs`：接口模型及 OpenAPI 文档。
- `web/src/views/`：登录、访问统计、链接管理及设置页面。
- `web/src/state.js`、`api/links.js`、`router/index.js`：共享状态、请求及路由。
- `api/tests/`、`web/tests/`：后端集成测试及浏览器测试。
- `.github/scripts/`、`.github/workflows/`：镜像验证、内存测量及 CI 发布。

## 开发运行

- 命令在项目根目录执行。使用 Node.js 24，Rust 工具链以 `rust-toolchain.toml` 为准。
- Rust 工具链变更时，同步更新 Dockerfile 和 `.github/workflows/checks.yml`。
- 复制 `.env.example` 为 `.env` 后设置本地密码；示例密码仅用于开发。

容器开发：

```bash
docker compose -f docker-compose.dev.yml up --build
```

- 前端 `http://127.0.0.1:5173`，支持热重载。
- 后端 `http://127.0.0.1:5201`，通过 `cargo-watch` 自动编译并重启。
- 前端通过 Vite 代理 `/api` 和 `/r`，容器内 `API_PROXY=http://api:5201`。
- Cargo 依赖、编译产物及前端依赖分别使用独立卷；数据库保存在宿主机 `api/short_links.db`。

本地运行：

```bash
npm ci --prefix web
npm run build --prefix web
cargo run --manifest-path api/Cargo.toml --locked
```

- 后端提供 `web/dist` 中的前端资源；前端改动后重新构建。
- 本地前端热重载使用 `npm run dev --prefix web`，默认代理 `http://localhost:5201`；通过 `API_PROXY` 修改代理目标。
- API 文档位于 `/docs`、`/redoc`，接口定义位于 `/openapi.json`；这些是服务路由，与已删除的仓库 `docs/` 目录无关。

## 配置

- 本地配置优先级：进程环境变量 > `api/.env` > 项目根目录 `.env` > 默认值。
- Compose 从项目根目录 `.env` 取值，容器环境以对应 Compose 文件的 `environment` 配置为准。

| 环境变量 | 默认值 | 约束 |
| --- | --- | --- |
| `ADMIN_PASSWORD` | 无 | 生产必填；未配置时不能登录或使用管理接口 |
| `BASE_URL` | `http://localhost:5201` | 短链公开地址，只包含 HTTP／HTTPS 协议、域名和可选端口 |
| `RANDOM_CODE_LENGTH` | `6` | 整数，4–16 |
| `TRUSTED_PROXY_COUNT` | `0` | 非负整数，仅填写真实可信反代层数 |
| `APP_NAME` | `Link WithU` | 去除首尾空白后非空，最多 60 个字符 |
| `TZ` | `Asia/Shanghai` | 有效 IANA 时区；修改后重启服务，Compose 部署需重新创建容器 |
| `PORT` | `5201` | 后端监听端口，1–65535；变更时同步调整端口映射与代理 |
| `DATABASE_URL` | 指向 `api/short_links.db` 的 SQLite URL | 持久化文件；兼容 `sqlite:///` 和旧版 `sqlite+aiosqlite:///` 前缀 |
| `FRONTEND_DIST` | 项目根目录下的 `web/dist` | 后端静态资源目录 |
| `API_PROXY` | `http://localhost:5201` | 仅用于 Vite 开发代理 |

- 设置页保存的站点名称、公开地址和随机短码长度覆盖环境变量默认值；统计时区始终读取 `TZ`。
- `DATABASE_URL` 拒绝内存数据库、`file:` URI 和查询参数。
- Nginx 反代使用 `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`，并按实际代理链配置 `TRUSTED_PROXY_COUNT`；部署 CDN 时重新核对完整代理链。

## 功能与数据约束

- 保留独立登录、访问统计 `/`、链接管理 `/links` 和设置 `/settings` 页面，以及桌面、手机和浅色／深色／跟随系统外观。
- 管理接口使用 Bearer 令牌；会话有效期 24 小时，修改管理员密码后旧令牌失效。
- 公共跳转 `/r/{short_code}` 无需登录，返回 302 和空响应体，成功响应前持久化访问记录。
- 创建及编辑地址只接受 HTTP／HTTPS，不接受包含账号密码的 URL；保留 Unicode 地址支持。
- 新建自定义短码只接受 1–32 位英文字母、数字、横线和下划线；旧备份中的 Unicode 短码继续兼容。
- 编辑保留短码；删除链接时一并删除关联访问记录和标签。
- 链接列表每页 10 条，访问明细每页 50 条；保留搜索、标签筛选、排序及机器人筛选。
- 搜索、排序、标签、站点名称及会话请求均需防止迟到响应覆盖新状态。
- 统计按配置时区分桶，处理夏令时，排除未来访问；来源只保留域名。
- CSV 导出保留 UTF-8 BOM、正确转义、公式防护和下载响应头。
- 修改 API 时同步维护接口模型、OpenAPI 注解、`api/src/openapi.rs` 和对应测试；保留 API、文档、跳转与 SPA 静态路由边界。
- SQLite 读写通过独立工作线程执行；保留有界队列、事务、失败回滚和断连清理，避免在异步请求线程执行阻塞数据库操作。
- 迁移保持幂等，兼容旧表结构和数据，按批次回填历史访问分类。
- 保留 SQLite 表结构、生产数据挂载路径和 JSON 备份格式的旧版兼容性。
- JSON 备份包含全部链接、访问记录及设置，格式标识为 `link-withu-backup`，版本为 `1`，上限为 50 MiB。
- 备份导出及恢复保持流式处理，不将全部访问历史或完整备份载入内存。
- 覆盖恢复前，前端要求输入 `RESTORE`，接口要求 `X-Confirm-Restore: RESTORE`。
- 恢复先逐条解析、校验到磁盘暂存库，通过后在事务中原子替换正式数据；无效数据、上传中断、写入或提交失败均保留原数据并清理临时文件。
- 同时只允许一次恢复；上传及暂存校验期间仍可正常跳转。
- 测试、冒烟及性能测量使用临时数据库或独立容器和卷，不使用实际部署数据。

## 生产部署

```bash
docker compose up -d
```

- 生产使用 `xwithu/link-withu:vMAJOR.MINOR.PATCH` 或 `xwithu/link-withu:latest`，配置独立的 `ADMIN_PASSWORD` 和实际公开地址。
- 生产容器只运行 Rust 服务并提供已构建前端，默认监听 5201。
- 数据挂载为 `./data:/proj/data`，数据库为 `/proj/data/short_links.db`。
- 升级前导出 JSON 备份；直接复制 SQLite 数据文件前停止对应服务。
- `/api/health` 用于健康检查；二进制支持 `--healthcheck` 和 `--version`；保留 SIGTERM 优雅退出。

构建生产镜像：

```bash
docker build -t link-withu:v1.0.1 .
```

## 检查与测试

```bash
npm ci --prefix web
cargo fmt --manifest-path api/Cargo.toml -- --check
cargo clippy --manifest-path api/Cargo.toml --all-targets --locked -- -D warnings
npm run format:check --prefix web
npm run build --prefix web
cargo test --manifest-path api/Cargo.toml --locked
npm exec --prefix web -- playwright install chromium
npm run test:e2e --prefix web
python3 -m unittest discover -s .github/scripts -p 'test_*.py'
python3 -m compileall -q .github/scripts
```

- 后端测试启动独立服务并使用临时数据库，覆盖 API、旧库升级、并发写入和恢复回滚。
- 浏览器测试前构建前端；Playwright 使用独立的 5202 端口，避免占用该端口。
- Linux 缺少浏览器系统依赖时，在 `web/` 下执行 `npx playwright install --with-deps --only-shell chromium`。
- 修改功能时补充正常流程、失败后的状态及下表对应项；新增 API 时更新 `generated_openapi_versions_routes_and_spa_boundaries` 的操作清单。
- 修改 Dockerfile、依赖、发布流程或脚本时，验证真实生产镜像启动与冒烟流程。
- `.github/scripts/smoke.py` 要求 `EXPECTED_VERSION` 和 `ADMIN_PASSWORD`，通过 `SMOKE_BASE_URL` 指向本轮独立测试容器。
- `.github/scripts/memory.py` 用于本地 Linux Docker Engine 的 cgroup v2 内存比较，参数为 `--baseline`、`--candidate` 和 `--output`；使用独立容器和匿名数据卷。
- 报告验证结果时区分格式检查、编译、自动化测试、生产镜像测试和实际发布，明确未执行的检查。

## 发布约定

Git 标签、GitHub Release 名称和 Docker 版本标签统一使用 `vMAJOR.MINOR.PATCH`（例如 `v1.0.1`）。`api/Cargo.toml`、`api/Cargo.lock`、`web/package.json` 和 `web/package-lock.json` 中的包版本保留标准数字格式，并同步更新。

- 仅接受稳定版本标签，不使用预发布、构建元数据或带多余前导零的版本。
- `web/package-lock.json` 的顶层版本及 `packages[""].version` 必须一致；`api/Cargo.lock` 更新 `link-withu` 包版本。
- Actions secrets 必须配置 `DOCKERHUB_USERNAME` 和 `DOCKERHUB_TOKEN`；缺少凭据时拒绝发布。
- `.github/scripts/check_release.py` 校验版本和凭据，输出完整 `tag` 与数字 `version`；镜像标签使用 `tag`。
- `.github/scripts/verify_images.py` 校验两个架构的已测试摘要及版本标签与 `latest` 的一致性；发布阶段直接合并摘要，不重新构建。
- 发布工作流仅在推送版本标签后执行；创建新 GitHub Release 时名称与标签一致，已存在的 Release 保留原标题和说明。

发布时，完整检查与镜像构建并行执行。amd64 使用 `ubuntu-24.04`，arm64 使用 `ubuntu-24.04-arm`，分别在原生 runner 上构建和测试。每个架构只构建一次，通过容器测试后上传独立的 `ci-运行编号-重试编号-架构` 候选标签，并记录不可变摘要；所有检查及两个架构都通过后，才按这些摘要合并发布版本标签和 `latest`。候选标签不用于生产部署。

Docker 使用固定版本的 `cargo-chef` 将 Rust 依赖编译为独立缓存层，并按架构保存到 Actions 缓存。仅业务源码变化时可复用依赖；首次构建、依赖或工具链变更时需要重新编译。日常 Checks 会取消同分支的过时任务，普通发布保持串行且不自动取消；显式强制更新版本标签时，新任务会取消当前发布任务。

修改 Dockerfile、发布脚本或依赖清单时，main 和 PR 还会运行 Image checks，实际构建并启动两个架构的生产镜像。main 的成功构建会预热默认分支缓存，后续不同版本标签的发布可复用该缓存；只有版本标签发布会上传 Docker Hub 候选镜像。也可在 Actions 页面手动运行 Image checks。

## 测试覆盖

测试按用户功能和数据完整性组织。增加功能时，同步补充正常流程、失败后的状态及本表中的对应项；新增 API 还需更新 `generated_openapi_versions_routes_and_spa_boundaries` 的操作清单。

| 功能 | 自动化验证 | 用例位置 |
| --- | --- | --- |
| 登录与会话 | 正确／错误密码、过期与伪造令牌、密码变更、认证头大小写、缺少管理员密码、受保护 API、前端会话失效与安全跳转 | `api/tests/api.rs`、`api/tests/review.rs`、两组浏览器用例 |
| 站点名称 | 保存后立即更新标题与登录页、页面切换、刷新、备份恢复、迟到的首次请求、服务端规范化结果、读取失败的默认名称 | `web/tests/workspace.spec.js`、`web/tests/review.spec.js` |
| 创建短链 | 自定义／随机短码、重复短码、公开地址与长度设置、URL 协议和端口、Unicode 地址、字段类型与长度限制、失败重试 | `api/tests/api.rs`、`api/tests/review.rs`、两组浏览器用例 |
| 编辑与删除 | 保留短码、更新地址／名称／备注／标签、删除关联访问记录、取消与关闭弹窗、保存失败重试、删除后清除失效复制区 | `api/tests/api.rs`、两组浏览器用例 |
| 搜索、标签与排序 | 名称／备注／地址／短码／标签搜索、通配符转义、标签去重与筛选、清除筛选、三种排序、旧请求不能覆盖新结果 | `api/tests/api.rs`、两组浏览器用例 |
| 链接分页 | 每页 10 条、前后翻页、筛选后回首页、末页删除、搜索防抖与排序并发 | `web/tests/workspace.spec.js` |
| 复制链接 | 创建结果及列表复制正确 URL、权限失败提示 | `web/tests/review.spec.js` |
| 公共跳转 | 无需登录、302 地址及空响应体、Unicode、未知短码、并发持久化访问、可信反代 IP 与无效 IP 回退 | `api/tests/api.rs`、`api/tests/review.rs` |
| 访问统计 | 全部／7／30／90 天、累计与分段计数、超过 90 天的分桶、排除未来数据、时区和夏令时、短链和标签筛选、排名与标签分组 | `api/tests/api.rs`、`api/src/analytics.rs`、两组浏览器用例 |
| 来源与客户端分类 | 只保留来源域名、异常来源、桌面／手机／平板／未知设备、常见浏览器与机器人分类 | `api/tests/api.rs`、`api/tests/review.rs` |
| 访问明细 | 全部历史、每页 50 条、翻页、机器人筛选后重置页码、失败后重试 | `api/tests/api.rs`、`web/tests/review.spec.js` |
| CSV 导出 | 链接／访问两种导出、标签和短码范围、时间及机器人筛选、UTF-8 BOM、引号／换行、公式防护、下载响应头 | `api/tests/api.rs`、`api/tests/review.rs` |
| JSON 备份 | 完整记录与设置、旧字段默认值、大历史流式导出、50 MiB 限制、浏览器下载与失败重试 | `api/tests/restore.rs`、两组浏览器用例 |
| 覆盖恢复 | 确认头与确认输入、文件预览和取消、超限／无效文件、重复记录／孤立访问／非法日期、字段缺失、成员乱序、写入及提交失败回滚、上传中断清理、并发恢复拒绝、上传期间仍能跳转 | `api/tests/restore.rs`、`api/tests/review.rs`、两组浏览器用例 |
| 设置与外观 | 名称／公开地址／随机长度保存及失败重试、标签缓存请求乱序、浅色／深色／跟随系统、退出登录 | `api/tests/api.rs`、两组浏览器用例 |
| 配置与旧库 | `.env` 优先级、环境变量覆盖、默认值、旧数据库 URL、拒绝内存 URI、启动校验、旧表结构补齐、分类回填、重复迁移、SQLite 完整性 | `api/src/config.rs`、`api/tests/migration.rs` |
| 服务与文档 | 健康检查成功／失败、版本命令、SIGTERM 退出、请求限流及断连恢复、16 个 API 操作清单、Swagger／ReDoc、静态文件类型和路径穿越防护 | `api/tests/api.rs`、`api/tests/migration.rs`、`api/tests/review.rs` |
| 生产镜像 | amd64／arm64 的真实容器启动、前端资源、认证、跳转、统计、备份恢复、原生健康检查；启动期间连接重置／中断／超时重试、持续失败超时、错误版本仍失败 | `.github/scripts/smoke.py`、`.github/scripts/test_smoke.py`、发布工作流 |
| 发布版本 | 必须使用稳定的 `vMAJOR.MINOR.PATCH`，四个版本文件一致，缺失凭据时拒绝执行，Actions 输出格式 | `.github/scripts/test_release.py` |
| 发布镜像完整性 | 必须同时包含两个架构，拒绝缺失／非法摘要、错误平台、重复架构、未经测试的镜像，以及版本与 latest 不一致 | `.github/scripts/test_verify_images.py` |

浏览器测试在桌面和手机视口各运行一遍，每个用例均检查未捕获的 JavaScript 异常。测试使用临时数据库；生产镜像冒烟及内存脚本使用独立容器和数据卷。

需要验证指定的生产二进制时，可设置 `LINK_WITHU_BIN` 为其绝对路径，再运行 Cargo 集成测试或 Playwright。未指定时，Cargo 使用本轮编译的二进制，Playwright 使用 `cargo run`。

本地审查另用 LLVM 的 `instrument-coverage`、`llvm-profdata` 和 `llvm-cov` 统计 `api/src` 的源码覆盖率，排除第三方依赖及独立集成测试文件，包含源码内的单元测试。行覆盖率用于发现遗漏，不代表所有系统错误、断电或真实服务器环境均已模拟；GitHub／Docker Hub 的实际发布只会在推送版本标签后执行。
