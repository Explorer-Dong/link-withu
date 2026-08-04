# 用户手册

本文介绍 Link WithU 的完整使用方法。

## 管理员选项

页面右上方点击“管理员登录”，输入配置的环境变量 `ADMIN_PASSWORD`。登录成功后，浏览器会保存 24 小时有效的管理员令牌，在有效期内查看统计和删除短链都不需要再次输入密码。

## 环境变量

| 变量名 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `ADMIN_PASSWORD` | 是 | 无 | 管理员密码。用于登录管理员，登录后 24 小时内可查看统计和删除短链。 |
| `BASE_URL` | 否 | `http://localhost:5201` | 生成短链时使用的公开访问地址。部署到域名后应改成实际域名，例如 `https://link.example.com`。 |
| `DATABASE_URL` | 否 | `sqlite+aiosqlite:///./short_links.db` | 后端数据库连接地址。Docker Compose 中默认使用 `/proj/data/short_links.db` 以便通过数据卷持久化。 |
| `FRONTEND_DIST` | 否 | 无 | 前端构建产物目录。官方 Docker 镜像内使用 `/proj/web/dist`；本地后端开发时不配置即可。 |
| `RANDOM_CODE_LENGTH` | 否 | `6` | 随机短码长度。 |
