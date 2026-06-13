# ============================================================
# Stage 1 — Builder (Rust + dépendances système)
# ============================================================
FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libpq-dev \
        git \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Pré-requis : exporter GITLAB_TOKEN avant le build, puis passer :
#   --secret id=gitlab_token,env=GITLAB_TOKEN
# Le token n'est jamais baked dans l'image.
RUN --mount=type=secret,id=gitlab_token \
    printf "machine gitlab.lucas-sanchez.fr\nlogin oauth2\npassword %s\n" \
        "$(cat /run/secrets/gitlab_token)" > /root/.netrc && \
    chmod 600 /root/.netrc && \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    cargo build --release --bin zenth_dht && \
    rm /root/.netrc

# ============================================================
# Stage 2 — Runtime minimal (glibc natif — évite les symboles C23 manquants sur musl)
# ============================================================
FROM debian:bookworm-slim

#    libpq5          : client PostgreSQL requis par Diesel
#    ca-certificates : validation TLS
#    tini            : init PID 1 — gestion propre SIGTERM/SIGCHLD
RUN apt-get update && apt-get install -y --no-install-recommends \
        libpq5 \
        ca-certificates \
        tini \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r -g 1001 appgroup \
    && useradd -r -u 1001 -g appgroup \
       -M -s /sbin/nologin appuser

WORKDIR /app
RUN chown appuser:appgroup /app

COPY --from=builder --chown=appuser:appgroup /build/target/release/zenth_dht /app/zenth_dht
RUN chmod 500 /app/zenth_dht

USER appuser

# PGSSLMODE=require : valeur par défaut pour la production.
# En dev (docker-compose), elle est surchargée par sslmode=disable dans l'URL.
ENV RUST_LOG=info \
    PGSSLMODE=require

EXPOSE 8081

HEALTHCHECK NONE

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/app/zenth_dht"]
