# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:24-alpine AS web
WORKDIR /proj/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.99.1-slim-bookworm AS toolchain
WORKDIR /proj/api

FROM toolchain AS dev
RUN rustup component add rustfmt clippy
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-watch --version 8.5.3 --locked
CMD ["cargo", "watch", "-w", "src", "-w", "Cargo.toml", "-w", "Cargo.lock", "-x", "run --locked"]

FROM toolchain AS chef
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-chef --version 0.1.78 --locked

FROM chef AS planner
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS dependencies
COPY --from=planner /proj/api/recipe.json recipe.json
# Keep dependency artifacts in an image layer so remote BuildKit caches retain them.
RUN cargo chef cook --release --locked --recipe-path recipe.json

FROM dependencies AS build
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src ./src
RUN cargo build --release --locked && cp target/release/link-withu /usr/local/bin/link-withu

FROM debian:bookworm-slim
WORKDIR /proj/api
ENV TZ=Asia/Shanghai \
    MALLOC_ARENA_MAX=2 \
    FRONTEND_DIST=/proj/web/dist \
    DATABASE_URL=sqlite:////proj/data/short_links.db
COPY --from=build /usr/local/bin/link-withu /usr/local/bin/link-withu
COPY --from=web /proj/web/dist /proj/web/dist
RUN mkdir -p /proj/data
LABEL org.opencontainers.image.title="Link WithU" \
      org.opencontainers.image.source="https://github.com/Explorer-Dong/link-withu" \
      org.opencontainers.image.licenses="MIT"
EXPOSE 5201
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/link-withu", "--healthcheck"]
CMD ["/usr/local/bin/link-withu"]
