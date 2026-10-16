# 用户手册

本文介绍 Link WithU 的使用方法。

## 管理员选项

输入环境变量 `ADMIN_PASSWORD` 配置的密码登录，24 小时内免密。首页查看访问统计，链接管理页创建、搜索和编辑短链，每页显示 10 条。修改链接时短码保持不变，短链跳转无需登录。

设置页可修改站点名称、公开地址和随机短码长度，切换外观、退出登录，以及导出全部数据或从备份恢复。恢复会覆盖现有数据，需输入 `RESTORE` 确认，备份文件最大 50 MiB。

## 环境变量

可通过 docker-compose.yml 文件同目录的 `.env` 配置：

| 环境变量 | 数据类型 | 默认值 | 变量说明 |
| --- | --- | --- | --- |
| `ADMIN_PASSWORD` | `str` | 无 | 管理员登录密码，生产环境必填 |
| `BASE_URL` | `str` | `http://localhost:5201` | 短链公开地址，例如 `https://go.example.com` |
| `RANDOM_CODE_LENGTH` | `int` | `6` | 随机短码长度，4–16 |
| `TRUSTED_PROXY_COUNT` | `int` | `0` | 可信反代层数。Nginx 反代设为 1，并配置 `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`；使用 CDN 再加 1 |
| `APP_NAME` | `str` | `Link WithU` | 管理面板名称 |
| `TZ` | `str` | `Asia/Shanghai` | 统计时区，修改后重新创建容器生效 |

站点名称、公开地址和随机短码长度以设置页保存的值为准；时区只读取 `TZ`。

## 数据存储

开发环境数据库位于 `api/short_links.db`；生产容器通过 `./data:/proj/data` 保存数据库。备份数据库文件前先停止对应容器。
