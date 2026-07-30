# Release-сборка с LTO происходит тут (CI/сервер), не на ноутбуке.
# ponytail: без cargo-chef; добавить, когда docker-билды в CI станут медленными.
# Сборочный и рабочий образы обязаны быть одного выпуска Debian: бинарь,
# собранный на более новой glibc, в старой не запустится вовсе
# («libc.so.6: version GLIBC_2.38 not found»). Меняешь один — меняй оба.
FROM rust:1-slim-bookworm AS server-build
# коммит попадает в /api/version — видно, что именно сейчас развёрнуто
ARG GIT_SHA=unknown
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY server ./server
RUN GIT_SHA=$GIT_SHA cargo build --release --bin api --bin ops

FROM node:22-slim AS web-build
WORKDIR /app/web
COPY web/package.json web/pnpm-lock.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY web .
RUN pnpm build

# Итог: один процесс, ~15-30 MB RSS — хватит самого дешёвого VPS
FROM debian:bookworm-slim
# корневые сертификаты: reqwest (обмен кода OAuth, healthcheck) проверяет TLS
# по системному хранилищу — без них исходящий HTTPS в контейнере не работает
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server-build /app/target/release/api /usr/local/bin/api
# операторские команды (docker compose exec app ops ...)
COPY --from=server-build /app/target/release/ops /usr/local/bin/ops
COPY --from=web-build /app/web/dist ./web/dist
EXPOSE 8080
CMD ["api"]
