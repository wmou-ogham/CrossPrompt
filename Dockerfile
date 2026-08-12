# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --ignore-scripts
COPY frontend/ ./
RUN npm run check && npm run build

FROM rust:1.86-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release --locked --bin cross-prompt --bin hash-password

FROM backend AS password-tool
ENTRYPOINT ["/build/target/release/hash-password"]

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 crossprompt \
    && useradd --system --uid 10001 --gid crossprompt --home-dir /app crossprompt \
    && mkdir -p /app/static /data \
    && chown -R crossprompt:crossprompt /app /data
COPY --from=backend /build/target/release/cross-prompt /usr/local/bin/cross-prompt
COPY --from=frontend /build/frontend/dist /app/static
USER crossprompt
WORKDIR /app
VOLUME ["/data"]
EXPOSE 8080
ENV CROSSPROMPT_DATABASE_URL=sqlite:///data/crossprompt.db \
    CROSSPROMPT_FRONTEND_DIR=/app/static \
    CROSSPROMPT_BIND=0.0.0.0:8080
HEALTHCHECK --interval=30s --timeout=4s --start-period=10s --retries=3 CMD ["curl", "--fail", "--silent", "http://127.0.0.1:8080/healthz"]
ENTRYPOINT ["/usr/local/bin/cross-prompt"]
