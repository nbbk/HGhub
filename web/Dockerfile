FROM node:22-bookworm-slim AS frontend
WORKDIR /build
RUN npm install --global pnpm@10.13.1
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
ENV VITE_WEB=true
RUN pnpm build

FROM rust:1.90-bookworm AS backend
WORKDIR /build
COPY server ./server
RUN cargo build --manifest-path server/Cargo.toml --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && groupadd --gid 10001 hghub && useradd --uid 10001 --gid 10001 --no-create-home hghub
WORKDIR /app
COPY --from=backend /build/server/target/release/hghub-web /usr/local/bin/hghub-web
COPY --from=frontend /build/dist /app/dist
COPY LICENSE /app/LICENSE
ENV HGHUB_WEB_BIND=0.0.0.0:8080 HGHUB_WEB_STATIC_DIR=/app/dist
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD curl --fail --silent http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["hghub-web"]
