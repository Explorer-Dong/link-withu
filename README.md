<h1 align="center">Link WithU</h1>

<p align="center">
  <strong>
    一款简约的短链管理器
  </strong>
</p>

<p align="center">
  <a href="https://hub.docker.com/repository/docker/xwithu/link-withu">
    <img src="https://img.shields.io/docker/v/xwithu/link-withu?color=blue" alt="Docker Image Version"/>
  </a>
</p>

<p align="center">
  <a href="#快速开始"><strong>快速开始</strong></a>
  &middot;
  <a href="./docs/manual.md"><strong>用户手册</strong></a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/link-withu-dark.png">
    <source media="(prefers-color-scheme: light)" srcset=".github/assets/link-withu.png">
    <img alt="Link WithU" src=".github/assets/link-withu.png">
  </picture>
</p>

## 快速开始

基于 Docker 部署：

```bash
docker run -d \
  --name link-withu \
  -p 5201:5201 \
  -e ADMIN_PASSWORD='请替换为你的管理员密码' \
  -e BASE_URL='http://localhost:5201' \
  -e DATABASE_URL='sqlite+aiosqlite:////proj/data/short_links.db' \
  -e FRONTEND_DIST='/proj/web/dist' \
  -v ./data:/proj/data \
  xwithu/link-withu:latest
```

基于 Docker Compose 部署：

```bash
# 下载 docker-compose.yml 文件
wget https://raw.githubusercontent.com/Explorer-Dong/link-withu/refs/heads/main/docker-compose.yml

# 设置管理员密码
echo "ADMIN_PASSWORD=<your_admin_password>" > .env

# 启动 Link Withu
docker compose up -d
```

阅读 [用户手册](./docs/manual.md) 查看更详细的使用指南。
