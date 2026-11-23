# Hysteria 2 — Docker-образ

[![License: GPL v3.0](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

Docker-образы [Hysteria 2](https://github.com/apernet/hysteria) с автоматической генерацией
сертификатов, аутентификацией через `users.txt`, ACL на базе GeoIP/GeoSite, управлением
пользователями и генерацией клиентских ссылок.

В репозитории две реализации одной и той же обвязки:

| Каталог | Что внутри |
|---|---|
| [`base/`](base/) | Bash-скрипты + Dockerfile (основная версия) |
| [`rust/`](rust/) | Та же логика одним бинарником на Rust (multi-stage сборка) |

Сам прокси-сервер — официальный `hysteria` (требуется **2.14+** для ACL-выражений).

---

## Возможности

- Самоподписанный сертификат создаётся при каждом старте (`SERVER_IP` → CN/SAN)
- Пользователи в `users.txt`, команды `hy2user` / `hy2ctl users` для управления
- `keygen` / `hy2ctl keygen` — готовый клиентский URI `hysteria2://...`
- ACL: блокировка `.ru`/`.рф`/`.su`, GeoIP RU, GeoSite-категорий и CIDR
- Protocol sniffing, BBR, masquerade под HTTPS, Traffic Stats API на `:9999`
- GeoIP/GeoSite базы **не запекаются** в образ — скачиваются при старте (GitHub → jsDelivr)
- Общая защита от Windows-переводов строк: `.gitattributes` + `sed -i 's/\r$//'` в образах

---

## Быстрый старт

### Bash-версия

```bash
cd base
# server.env: SERVER_IP=<PUBLIC_IP>, при необходимости SERVER_PORT=443
docker build --no-cache -t hysteria2 .
docker run -d --name hysteria2 --restart unless-stopped \
  --ulimit rtprio=99 --cap-add NET_ADMIN --cap-add SYS_NICE \
  --env-file server.env \
  -p 443:443/udp -p 9999:9999/tcp hysteria2
```

### Rust-версия

```bash
cd rust
docker build --no-cache -t hysteria2-rust .
# docker run — такой же, образ hysteria2-rust
```

Подробности — в [`base/README.md`](base/README.md) и [`rust/README.md`](rust/README.md).

---

## Порты

| Порт | Назначение |
|---|---|
| `443/udp` | основной трафик Hysteria 2 |
| `9999/tcp` | Traffic Stats API |

---

## Клиентская ссылка

```bash
docker exec hysteria2 keygen <username>          # base
docker exec hysteria2-rust hy2ctl keygen <username>  # rust
```

IP берётся из `SERVER_IP`, порт — из `SERVER_PORT` (по умолчанию 443),
SNI — из `masquerade.proxy.url` в конфиге.

---

## Структура

```text
.
├── base/     # Dockerfile + bash-скрипты
├── rust/     # Rust-бинарник + multi-stage Dockerfile
└── LICENSE   # GNU GPL v3.0
```

---

## Лицензия

[GNU General Public License v3.0](LICENSE).
