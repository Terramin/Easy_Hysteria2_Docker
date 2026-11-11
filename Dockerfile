FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    openssl \
    ca-certificates \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Установка Hysteria 2
RUN curl -fsSL https://get.hy2.sh | bash

RUN mkdir -p /etc/hysteria

WORKDIR /etc/hysteria

COPY config.yaml /etc/hysteria/config.yaml
COPY auth.sh /etc/hysteria/auth.sh
COPY entrypoint.sh /entrypoint.sh
COPY hy2user /usr/local/bin/hy2user

RUN chmod +x /entrypoint.sh /etc/hysteria/auth.sh /usr/local/bin/hy2user

EXPOSE 443/udp 9999/tcp

ENTRYPOINT ["/entrypoint.sh"]