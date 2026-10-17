# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:24-alpine AS web
WORKDIR /proj/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.99.1-slim-bookworm AS toolchain
RUN rustup component add rustfmt clippy
WORKDIR /proj/api

FROM toolchain AS dev
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-watch --version 8.5.3 --locked
CMD ["cargo", "watch", "-w", "src", "-w", "Cargo.toml", "-w", "Cargo.lock", "-x", "run --locked"]

FROM toolchain AS build
ARG TARGETARCH
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=link-withu-target-${TARGETARCH},target=/proj/api/target \
    cargo build --release --locked && cp target/release/link-withu /usr/local/bin/link-withu

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
