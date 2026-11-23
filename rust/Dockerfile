# --- Сборка Rust-обвязки -------------------------------------------------
FROM rust:1.99-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

# --- Образ с официальным hysteria и Rust-бинарником ----------------------
FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        curl \
        openssl \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Официальный сервер Hysteria 2 (нужна версия 2.14+ для ACL-выражений)
RUN curl -fsSL https://get.hy2.sh | bash

RUN mkdir -p /etc/hysteria

COPY --from=builder /build/target/release/hy2ctl /usr/local/bin/hy2ctl
RUN chmod +x /usr/local/bin/hy2ctl \
    && ln -s /usr/local/bin/hy2ctl /usr/local/bin/hy2auth

COPY config.yaml /etc/hysteria/config.yaml

WORKDIR /etc/hysteria

EXPOSE 443/udp 9999/tcp

ENTRYPOINT ["/usr/local/bin/hy2ctl", "start"]
