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

# Базы GeoIP/GeoSite НЕ запекаются в образ — их скачивает entrypoint при старте.
# Благодаря .dockerignore файлы geoip.dat/geosite.dat не попадают даже в build context.
RUN mkdir -p /etc/hysteria

WORKDIR /etc/hysteria

COPY config.yaml /etc/hysteria/config.yaml
COPY auth.sh /etc/hysteria/auth.sh
COPY entrypoint.sh /entrypoint.sh
COPY hy2user /usr/local/bin/hy2user
COPY keygen.sh /usr/local/bin/keygen
COPY geo_update.sh /usr/local/bin/geo-update

RUN chmod +x /entrypoint.sh /etc/hysteria/auth.sh /usr/local/bin/hy2user /usr/local/bin/keygen /usr/local/bin/geo-update

# Скрипты часто правят в Windows: страховка от CRLF в shebang (/bin/bash^M)
RUN sed -i 's/\r$//' /entrypoint.sh /etc/hysteria/auth.sh /etc/hysteria/config.yaml /usr/local/bin/hy2user /usr/local/bin/keygen /usr/local/bin/geo-update

EXPOSE 443/udp 9999/tcp

ENTRYPOINT ["/entrypoint.sh"]
