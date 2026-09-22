# 用户手册

本文介绍 Link WithU 的完整使用方法。

## 管理员选项

页面右上方点击“管理员登录”，输入配置的环境变量 `ADMIN_PASSWORD`。登录成功后，浏览器会保存 24 小时有效的管理员令牌，在有效期内查看统计、修改短链的目标链接和删除短链都不需要再次输入密码。修改短链时短码保持不变，仅允许更新原始链接。

## 环境变量

可被 docker-compose.yml 文件同目录的 `.env` 覆盖：

| 环境变量 | 数据类型 | 默认值 | 变量说明 |
| --- | --- | --- | --- |
| `ADMIN_PASSWORD` | `str` | `admin` | 管理员登录密码，登录后 24 小时内免密 |
| `BASE_URL` | `str` | `http://localhost:5201` | 短链的公开访问地址，生产环境改为对应的域名服务，例如：`https://go.example.com` |
| `RANDOM_CODE_LENGTH` | `int` | `6` | 随机短码长度 |
| `TRUSTED_PROXY_COUNT` | `int` | `0` | 前置可信反代层数。直接部署填 0；使用 Nginx 反代服务需 +1 并配置 `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`；使用 CDN 需再 +1 |
| `APP_NAME` | `str` | `Link WithU` | 管理面板名称 |
