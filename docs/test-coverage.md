# 测试覆盖

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

运行全部检查见 [开发手册](dev-manual.md)。需要验证指定的生产二进制时，可设置 `LINK_WITHU_BIN` 为其绝对路径，再运行 Cargo 集成测试或 Playwright。未指定时，Cargo 使用本轮编译的二进制，Playwright 使用 `cargo run`。

本地审查另用 LLVM 的 `instrument-coverage`、`llvm-profdata` 和 `llvm-cov` 统计 `api/src` 的源码覆盖率，排除第三方依赖及独立集成测试文件，包含源码内的单元测试。行覆盖率用于发现遗漏，不代表所有系统错误、断电或真实服务器环境均已模拟；GitHub／Docker Hub 的实际发布只会在推送版本标签后执行。
