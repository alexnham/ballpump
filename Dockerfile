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
# Refresh source mtimes so Cargo cannot reuse artifacts from a newer cached build.
RUN --mount=type=cache,id=ballpump-cargo-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=ballpump-cargo-target,sharing=locked,target=/build/backend/target \
    touch src/*.rs && cargo build --release --locked --bin ballpump && cp target/release/ballpump /build/ballpump

FROM backend AS backend-tests
COPY backend/tests ./tests
COPY fixtures /build/fixtures
RUN rustup component add clippy
RUN --mount=type=cache,id=ballpump-cargo-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=ballpump-cargo-target,sharing=locked,target=/build/backend/target \
    touch tests/*.rs && cargo test --release --locked && cargo clippy --release --locked --all-targets -- -D warnings

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ballpump \
    && useradd --uid 10001 --gid ballpump --no-create-home ballpump \
    && mkdir -p /app/data \
    && chown ballpump:ballpump /app/data
WORKDIR /app
COPY --from=backend /build/ballpump /usr/local/bin/ballpump
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
