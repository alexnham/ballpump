# syntax=docker/dockerfile:1
FROM node:24-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:bookworm AS backend
WORKDIR /build/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo build --release --locked --bin ballpump

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ballpump \
    && useradd --uid 10001 --gid ballpump --no-create-home ballpump \
    && mkdir -p /app/data \
    && chown ballpump:ballpump /app/data
WORKDIR /app
COPY --from=backend /build/backend/target/release/ballpump /usr/local/bin/ballpump
COPY --from=frontend /build/frontend/dist ./frontend/dist
ENV SOURCE=demo \
    BIND_ADDR=0.0.0.0:3000 \
    FRONTEND_DIST=/app/frontend/dist \
    CHECKPOINT_PATH=/app/data/cursor \
    RUST_LOG=ballpump=info,helius_laserstream=warn
USER ballpump
EXPOSE 3000
STOPSIGNAL SIGINT
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3000/health > /dev/null || exit 1
ENTRYPOINT ["ballpump"]
