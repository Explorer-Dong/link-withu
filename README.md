<h1 align="center">Link WithU</h1>

<p align="center">
  <strong>
    一款简约的短链管理器
  </strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.99.1-D34516?logo=rust&logoColor=white" alt="Rust 1.99.1"/>
  </a>
  <a href="https://hub.docker.com/r/xwithu/link-withu/tags">
    <img src="https://img.shields.io/docker/v/xwithu/link-withu?color=blue&label=docker" alt="Docker Image Version"/>
  </a>
  <a href="https://hub.docker.com/r/xwithu/link-withu">
    <img src="https://img.shields.io/docker/pulls/xwithu/link-withu?color=blue&label=pulls" alt="Docker Pulls"/>
  </a>
</p>

- **极简的管理界面**：零广告，卡片式设计，不被冗余信息淹没。
- **直观的访问统计**：什么时候、怎么访问、来自哪里，一目了然。
- **极致的内存占用**：后端由 Rust 驱动，服务在静默状态平均内存占用仅 2MB。
- **便捷的部署方案**：支持 Docker 一键私有化部署与全量数据的导出与导入。

## 快速开始

基于 Docker 部署：

```bash
docker run -d \
  --name link-withu \
  -p 5201:5201 \
  -e ADMIN_PASSWORD='<your_password>' \
  -e BASE_URL='<your_server_ip>' \
  -v ./data:/proj/data \
  xwithu/link-withu:latest
```

基于 Docker Compose 部署：

```bash
# 下载 docker-compose.yml 文件
wget https://raw.githubusercontent.com/Explorer-Dong/link-withu/refs/heads/main/docker-compose.yml

# 下载并配置环境变量
wget https://raw.githubusercontent.com/Explorer-Dong/link-withu/refs/heads/main/.env.example -O .env
# 配置密码、域名等参数

# 启动 Link Withu
docker compose up -d
```

环境变量选项：

| 环境变量 | 数据类型 | 默认值 | 变量说明 |
| --- | --- | --- | --- |
| `ADMIN_PASSWORD` | `str` | 无 | 管理员登录密码，生产环境必填 |
| `BASE_URL` | `str` | `http://localhost:5201` | 短链公开地址，例如 `https://go.example.com` |
| `RANDOM_CODE_LENGTH` | `int` | `6` | 随机短码长度，4–16 |
| `TRUSTED_PROXY_COUNT` | `int` | `0` | 可信反代层数。Nginx 反代设为 1，并配置 `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`；使用 CDN 再加 1 |
| `APP_NAME` | `str` | `Link WithU` | 管理面板名称 |
| `TZ` | `str` | `Asia/Shanghai` | 统计时区，修改后重新创建容器生效 |

## 界面截图

### 桌面端

<div align="center">
<table>
<tr>
<td align="center"><b>访问统计</b></td>
<td align="center"><b>链接管理</b></td>
<td align="center"><b>设置</b></td>
</tr>
<tr>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/analytics-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/analytics.png">
<img src=".github/assets/analytics.png" alt="访问统计" width="400">
</picture></td>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/link-withu-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/link-withu.png">
<img src=".github/assets/link-withu.png" alt="链接管理" width="400">
</picture></td>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/settings-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/settings.png">
<img src=".github/assets/settings.png" alt="设置" width="400">
</picture></td>
</tr>
</table>
</div>

### 手机端

<div align="center">
<table>
<tr>
<td align="center"><b>访问统计</b></td>
<td align="center"><b>链接管理</b></td>
<td align="center"><b>设置</b></td>
</tr>
<tr>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/analytics-mobile-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/analytics-mobile.png">
<img src=".github/assets/analytics-mobile.png" alt="手机端访问统计" width="180">
</picture></td>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/links-mobile-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/links-mobile.png">
<img src=".github/assets/links-mobile.png" alt="手机端链接管理" width="180">
</picture></td>
<td><picture>
<source media="(prefers-color-scheme: dark)" srcset=".github/assets/settings-mobile-dark.png">
<source media="(prefers-color-scheme: light)" srcset=".github/assets/settings-mobile.png">
<img src=".github/assets/settings-mobile.png" alt="手机端设置" width="180">
</picture></td>
</tr>
</table>
</div>

## 参与贡献

一键启动容器化开发环境：

```bash
docker compose -f docker-compose.dev.yml up --build
```

- 前端 `http://127.0.0.1:5173`，支持热重载。
- 后端 `http://127.0.0.1:5201`，支持热重载。接口文档 `http://127.0.0.1:5201/docs`。

技术栈：前端 Vue，后端 Axum，数据库 SQLite。

## 开源协议

本项目遵循 [MIT](./LICENSE) 协议。
