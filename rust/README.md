# Hysteria 2 — Rust-обвязка

Вариант контейнера, где вместо набора bash-скриптов используется один бинарник `hy2ctl` на Rust.
Сам прокси-сервер по-прежнему официальный `hysteria` (устанавливается через `get.hy2.sh`,
нужна версия **2.14+** для ACL-выражений).

Базовая версия на bash лежит рядом в `../base/`.

---

## Что делает `hy2ctl`

| Команда | Аналог в `base/` | Назначение |
|---|---|---|
| `start` | `entrypoint.sh` | докачка geo-баз, генерация сертификатов, `users.txt`, запуск сервера |
| `auth <addr> <auth> <tx>` | `auth.sh` | command-auth (вызывается через симлинк `/usr/local/bin/hy2auth`) |
| `keygen <username>` | `keygen.sh` | клиентский URI `hysteria2://...` |
| `geo-update [geoip\|geosite]` | `geo_update.sh` | обновление GeoIP/GeoSite с failover |
| `users add\|pass\|del\|list` | `hy2user` | управление пользователями |

Команда `start` — entrypoint по умолчанию; она выполняется до запуска `hysteria` и при
невозможности скачать базы завершает контейнер с ошибкой.

---

## Переменные окружения

| Переменная | Где используется | По умолчанию |
|---|---|---|
| `SERVER_IP` | `start`, `keygen` | обязательна |
| `SERVER_PORT` | `keygen` | `443` |
| `SNI` | `keygen` | хост из `masquerade.proxy.url` конфига, иначе `SERVER_IP` |
| `USERS_FILE` | `auth`, `keygen`, `users` | `/etc/hysteria/users.txt`, иначе `./users.txt` |
| `CONFIG_FILE` | `keygen`, `start` | `/etc/hysteria/config.yaml`, иначе `./config.yaml` |
| `GEO_DIR` | `start`, `geo-update` | `/etc/hysteria` в контейнере, иначе каталог бинарника |
| `GEO_BASE_URL` | `start`, `geo-update` | GitHub releases latest |
| `GEO_CDN_BASE` | `start`, `geo-update` | jsDelivr `@release` |
| `GEOIP_URL` / `GEOSITE_URL` | `start`, `geo-update` | список через запятую, переопределяет источники |

`server.env` читается как запасной источник для `SERVER_IP` и `SERVER_PORT`, если переменной
окружения нет.

---

## Сборка и запуск

```bash
cd "D:\Table\BackupFile\Sync\development\Docker Image\Hysteria2\rust"

# server.env: SERVER_IP=<PUBLIC_IP>, при необходимости SERVER_PORT=443
docker build --no-cache -t hysteria2-rust .
docker rm -f hysteria2-rust

docker run -d \
  --name hysteria2-rust \
  --restart unless-stopped \
  --ulimit rtprio=99 \
  --cap-add NET_ADMIN \
  --cap-add SYS_NICE \
  --env-file server.env \
  -p 443:443/udp \
  -p 9999:9999/tcp \
  -v "$(pwd)/users.txt:/etc/hysteria/users.txt" \
  hysteria2-rust
```

Geo-базы в образ не попадают (`.dockerignore`) — `start` скачивает их при запуске.

---

## Примеры

```bash
# Клиентская ссылка
docker exec hysteria2-rust hy2ctl keygen devuser1

# Обновить базы вручную
docker exec hysteria2-rust hy2ctl geo-update
docker restart hysteria2-rust

# Пользователи
docker exec -it hysteria2-rust hy2ctl users add client1
docker exec -it hysteria2-rust hy2ctl users pass client1
docker exec -it hysteria2-rust hy2ctl users del client1
docker exec -it hysteria2-rust hy2ctl users list

# Проверка конфига
docker logs -f hysteria2-rust
```

---

## Отличия от bash-версии

- Один бинарник вместо набора `.sh`, меньше зависимостей в рантайме (нужны только
  `openssl`, `ca-certificates` и сам `hysteria`).
- `auth` — тот же бинарник через симлинк `hy2auth`, прописанный в `config.yaml`.
- Скачивание баз идёт через `ureq` (rustls), без вызова `curl`.
- Логика percent-encoding, SNI и пользователей полностью на Rust.
- Конфиг, порты и поведение фактически совпадают с `base/`.

---

## Лицензия

GNU General Public License v3.0, полный текст в файле LICENSE в корне репозитория.
