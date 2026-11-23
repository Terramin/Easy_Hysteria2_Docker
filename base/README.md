# Hysteria 2 — Docker

Docker-образ [Hysteria 2](https://github.com/apernet/hysteria) с автоматической генерацией
сертификатов, аутентификацией через `users.txt`, ACL на базе GeoIP/GeoSite и утилитами для
управления пользователями и выдачи клиентских ссылок.

Требуется Hysteria **2.14+** — в конфиге используется новый ACL-движок на выражениях (`acl.rules`).

---

## Возможности

- Самоподписанный сертификат создаётся при каждом старте контейнера
- Command-based авторизация: `users.txt` в формате `username:password`
- `hy2user` — добавление/смена пароля/удаление/список пользователей
- `keygen` — готовый клиентский URI `hysteria2://...` по имени пользователя
- ACL: блокировка `.ru`/`.рф`/`.su`, GeoIP RU, GeoSite-категорий и отдельных CIDR
- Protocol sniffing (HTTP/TLS/QUIC SNI) для корректной работы доменных правил ACL
- BBR, bandwidth-лимиты, masquerade под HTTPS, Traffic Stats API на `:9999`
- GeoIP/GeoSite базы **не запекаются** в образ — контейнер скачивает их при старте

---

## Структура проекта

```text
Hysteria2/
├── base/               # текущая реализация (Docker + bash) — весь рабочий проект
│   ├── Dockerfile          # сборка образа
│   ├── config.yaml         # конфиг сервера
│   ├── entrypoint.sh       # докачка geo-баз, генерация сертификатов и запуск hysteria
│   ├── auth.sh             # command-auth: проверка users.txt
│   ├── hy2user             # CLI управления пользователями
│   ├── keygen.sh           # генерация клиентского URI
│   ├── geo_update.sh       # обновление geoip.dat / geosite.dat
│   ├── tune_network.sh     # оптимизация сети на хосте (BBR, sysctl)
│   ├── server.env          # SERVER_IP / SERVER_PORT (не попадает в образ)
│   ├── users.txt           # пользователи (не попадает в образ)
│   ├── geoip.dat           # база GeoIP (не попадает в образ, скачивается при старте)
│   ├── geosite.dat         # база GeoSite (не попадает в образ, скачивается при старте)
│   ├── .dockerignore
│   ├── .gitignore
│   └── .gitattributes      # принудительные LF для скриптов и конфигов
└── rust/               # Rust-обвязка (аналог этого проекта на Rust)
```

Все команды ниже выполняются из каталога `base/`. Сборка использует `base/.dockerignore`, поэтому
`server.env`, `users.txt`, ключи/сертификаты и geo-базы не попадают в build context.
При старте контейнера `entrypoint.sh` скачивает недостающие базы и только потом запускает Hysteria.

---

## 1. Переменные окружения (`server.env`)

```bash
SERVER_IP=<публичный_IP_адрес_сервера>
# SERVER_PORT=443   # необязательно; если не задан, keygen использует 443
```

- `SERVER_IP` обязателен: `entrypoint.sh` использует его для `CN` и `subjectAltName=IP:...`,
  а `keygen` — как адрес подключения в клиентской ссылке.
- `SERVER_PORT` нужен только `keygen`. Порт самого сервера задаётся в `config.yaml` (`listen: :443`).
  Если публикуете другой порт, задайте `SERVER_PORT` и не забудьте `-p` в `docker run`.

Файл передаётся через `--env-file server.env`, поэтому после смены IP/порта контейнер нужно
пересоздать (см. [Обновление](#9-обновление-и-смена-ipпорта)).

---

## 2. `config.yaml`

Рабочий конфиг (секрет замените своим):

```yaml
listen: :443

tls:
  cert: /etc/hysteria/server.crt
  key: /etc/hysteria/server.key

auth:
  type: command
  command: /etc/hysteria/auth.sh

bandwidth:
  up: 80 Mbps
  down: 80 Mbps

masquerade:
  type: proxy
  proxy:
    url: https://api.audio.com/
    rewriteHost: true

congestion:
  type: bbr
  bbrProfile: conservative

trafficStats:
  listen: :9999
  secret: "REPLACE_WITH_YOUR_SECRET"

sniff:
  enable: true
  timeout: 2s
  rewriteDomain: false
  tcpPorts: 80,443
  udpPorts: 443

udpIdleTimeout: 60s

quic:
  initStreamReceiveWindow: 8388608
  maxStreamReceiveWindow: 8388608
  initConnReceiveWindow: 20971520
  maxConnReceiveWindow: 20971520
  maxIdleTimeout: 30s
  maxIncomingStreams: 512
  disablePathMTUDiscovery: true

outbounds:
  - name: direct
    type: direct

acl:
  geoip: /etc/hysteria/geoip.dat
  geosite: /etc/hysteria/geosite.dat
  rules:
    - outbound: reject
      match: suffix("ru") || suffix("xn--p1ai") || suffix("su")

    - outbound: reject
      match: geoip("ru") || geosite("category-ru")
    - outbound: reject
      match: geosite("yandex") || geosite("vk") || geosite("mailru")
    - outbound: reject
      match: geosite("sberbank") || geosite("tbank-ru") || suffix("vtb.ru") || suffix("vtb.com")
    - outbound: reject
      match: geosite("kinopoisk") || suffix("ivi.ru") || suffix("ivi.tv") || geosite("okko") || suffix("more.tv")

    - outbound: reject
      match: >-
        suffix("max.ru") || suffix("rutube.ru") || suffix("vkplay.ru") ||
        suffix("wink.ru") || suffix("premier.one") || suffix("start.ru") ||
        suffix("amediateka.ru") || suffix("dzen.ru")

    - outbound: reject
      match: >-
        cidr("158.160.96.0/20") || cidr("158.160.97.0/24") ||
        cidr("158.160.98.0/24") || cidr("158.160.99.0/24") ||
        cidr("158.160.100.0/22") || cidr("158.160.101.0/24") ||
        cidr("158.160.102.0/24") || cidr("158.160.103.0/24") ||
        cidr("158.160.104.0/24") || cidr("158.160.37.0/24") ||
        cidr("158.160.57.0/24") || cidr("176.57.65.0/24")

    - outbound: direct
```

Важные детали ACL в Hysteria 2.14:

- `acl.rules` с полем `match` — выражения на языке [expr](https://expr-lang.org/).
- Встроенные outbound'ы: `direct`, `reject`, `default`; свои объявляются в `outbounds`.
- `match: all` **невалиден**. Последнее правило пишется без `match` — оно означает «всё остальное».
- Имена в `geosite(...)` должны существовать в `geosite.dat`, иначе сервер не стартует.
  Проверить: `hysteria acl test -c config.yaml --no-resolve yandex.ru:443`.
- `udpIdleTimeout` — поле верхнего уровня; внутри `quic` оно игнорируется. `keepAlivePeriod`
  в Hysteria 2 не поддерживается.

Первое правило `suffix("ru")` блокирует **все** `.ru`-домены, поэтому последующие правила для
российских сервисов фактически не достигаются. Если это нежелательно — сузьте или уберите его.

---

## 3. `users.txt`

```text
# username:password
devuser1:iphusae2aigh7leem1ohshegh3oo
```

- Пароль не должен содержать `:` и пробелы; имя — только `[a-zA-Z0-9_-]`.
- Имя пользователя сравнивается без учёта регистра.
- Если `users.txt` отсутствует, `entrypoint.sh` создаст файл с пользователем `devuser1`.
- Изменения применяются мгновенно, перезапуск не нужен.
- Чтобы пользователи не терялись при пересоздании контейнера, монтируйте файл как volume
  (см. ниже). Без volume при новом контейнере снова появится только `devuser1`.

---

## 4. Сборка образа

Сборка из каталога с `Dockerfile`:

```bash
cd "D:\Table\BackupFile\Sync\development\Docker Image\Hysteria2\base"
docker build --no-cache -t hysteria2 .
```

Образ включает Hysteria 2 (последняя версия через `get.hy2.sh`), `bash`, `curl`, `openssl`,
`ca-certificates` и все скрипты. **Баз GeoIP/GeoSite в образе нет** — `geoip.dat` и `geosite.dat`
исключены через `.dockerignore`; при старте `entrypoint.sh` проверяет их наличие и скачивает
недостающие (см. раздел 12). Это делает образ меньше и позволяет обновлять базы без пересборки.

> Скрипты в образе принудительно переводятся в LF (`sed -i 's/\r$//'`), а `.gitattributes`
> закрепляет `eol=lf`. Это защищает от ошибки `/bin/bash^M: bad interpreter` при правках в Windows.

---

## 5. Запуск контейнера

Удалите старый контейнер, если он есть:

```bash
docker rm -f hysteria2
```

Базовый запуск:

```bash
docker run -d \
  --name hysteria2 \
  --restart unless-stopped \
  --ulimit rtprio=99 \
  --cap-add NET_ADMIN \
  --cap-add SYS_NICE \
  --env-file server.env \
  -p 443:443/udp \
  -p 9999:9999/tcp \
  hysteria2
```

С сохранением пользователей на хосте:

```bash
docker run -d \
  --name hysteria2 \
  --restart unless-stopped \
  --ulimit rtprio=99 \
  --cap-add NET_ADMIN \
  --cap-add SYS_NICE \
  --env-file server.env \
  -p 443:443/udp \
  -p 9999:9999/tcp \
  -v "$(pwd)/users.txt:/etc/hysteria/users.txt" \
  hysteria2
```

Порты:

- `443/udp` — основной порт Hysteria (`listen: :443` в конфиге)
- `9999/tcp` — Traffic Stats API (секрет из `trafficStats.secret`)

Если изменили `listen` в конфиге или задали `SERVER_PORT`, обновите и флаг `-p` в `docker run`.

---

## 6. Управление пользователями

Внутри контейнера доступна утилита `hy2user`:

```bash
docker exec -it hysteria2 hy2user add client1                 # пароль сгенерируется
docker exec -it hysteria2 hy2user add client2 MyStrongPass123 # свой пароль
docker exec -it hysteria2 hy2user pass client1                # новый пароль
docker exec -it hysteria2 hy2user pass client1 NewPass456
docker exec -it hysteria2 hy2user del client2
docker exec -it hysteria2 hy2user list
docker exec -it hysteria2 hy2user help
```

Или прямо в `users.txt` на хосте (при смонтированном volume):

```bash
echo "newuser:SuperSecurePass123" >> users.txt
sed -i 's/^user1:.*/user1:new_password/' users.txt
sed -i '/^user3:/d' users.txt
cat users.txt
```

---

## 7. Клиентский URI (`keygen.sh` / `keygen`)

Скрипт печатает готовую ссылку для клиента по имени пользователя:

```bash
# внутри контейнера
docker exec hysteria2 keygen devuser1

# на хосте, рядом с users.txt, server.env и config.yaml
bash keygen.sh devuser1
```

Откуда берутся значения:

| Параметр | Источник |
|---|---|
| пользователь/пароль | `USERS_FILE`, иначе `/etc/hysteria/users.txt`, иначе `./users.txt` |
| адрес `@IP` | всегда `SERVER_IP` из env (или из `server.env`) |
| порт | всегда `SERVER_PORT` из env (или из `server.env`); по умолчанию **443** |
| `sni` | хост из `masquerade.proxy.url` в `config.yaml`; при отсутствии — `SERVER_IP` |
| переопределения | `SNI` (явный SNI), `CONFIG_FILE` (путь к конфигу) |

Пример при `SERVER_IP=<IP>`, `SERVER_PORT` не задан и `url: https://api.audio.com/`:

```text
hysteria2://devuser1:iphusae2aigh7leem1ohshegh3oo@<IP>:443/?insecure=1&sni=api.audio.com#Hysteria2
```

Пароль и хост кодируются по RFC 3986 (кириллица → `%D0%BF...`), как в `hysteria share`.
`insecure=1` обязателен: сертификат самоподписанный. Ссылка выводится в stdout, ошибки — в stderr.

---

## 8. Проверка работы

```bash
docker ps
docker logs -f hysteria2
docker port hysteria2
ss -lunp | grep 443
```

Ожидаемые строки в логах:

```text
server mode
server up and running   {"listen": ":443"}
traffic stats server up and running   {"listen": ":9999"}
```

Проверка ACL без запуска сервера:

```bash
hysteria acl test -c config.yaml --no-resolve yandex.ru:443
```

---

## 9. Обновление и смена IP/порта

1. Отредактируйте `server.env` (`SERVER_IP`, при необходимости `SERVER_PORT`).
2. Если меняли `config.yaml`, пересоберите образ: `docker build --no-cache -t hysteria2 .`
3. Пересоздайте контейнер: `docker rm -f hysteria2` и заново `docker run ...`.
4. Сертификаты генерируются заново при каждом старте — клиентские ссылки остаются валидными
   благодаря `insecure=1`.

---

## 10. Диагностика: бесконечный перезапуск

Контейнер с `--restart unless-stopped` перезапускается при любом выходе процесса. Основные причины:

- **CRLF в `.sh`** → `/bin/bash^M: bad interpreter`. Лечится `sed -i 's/\r$//' ...`
  (уже есть в `Dockerfile`) и `.gitattributes`.
- **Неизвестный `geosite(...)`** → `GeoSite name ... not found`. Проверяйте имена по `geosite.dat`.
- **`match: all`** → `expected bool, but got func(...)`. В 2.14 последнее правило без `match`.
- **Не задан `SERVER_IP`** → `SERVER_IP is not set`.

Посмотреть точную причину:

```bash
docker logs --tail 100 hysteria2
```

---

## 11. Оптимизация сети на хосте

```bash
sudo bash tune_network.sh
```

Скрипт включает BBR, `fq` qdisc, увеличивает буферы `rmem_max`/`wmem_max` и добавляет
параметры в `/etc/sysctl.conf` (идемпотентно). Требует `sudo`.

---

## 12. Обновление GeoIP/GeoSite

Скрипт скачивает свежие `geoip.dat` и `geosite.dat` из
[Loyalsoldier/v2ray-rules-dat](https://github.com/Loyalsoldier/v2ray-rules-dat) (формат совместим
с Hysteria 2), проверяет размер и protobuf-заголовок, сохраняет старые файлы как `.bak` и
атомарно заменяет исходные.

**В образе баз нет:** `entrypoint.sh` при каждом старте проверяет наличие `/etc/hysteria/geoip.dat`
и `/etc/hysteria/geosite.dat` и скачивает недостающие **до** генерации сертификатов и запуска
Hysteria. Если скачать не удалось, контейнер завершается с ошибкой и не поднимает «полуработающий»
сервер. Вручную обновлять базы нужно только для принудительного обновления без перезапуска.

Источники опрашиваются по очереди, пока один не сработает:

1. `https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/<file>`
2. `https://cdn.jsdelivr.net/gh/Loyalsoldier/v2ray-rules-dat@release/<file>` — дополнительный CDN-зеркало

```bash
# На хосте — обновляет файлы рядом со скриптом
bash geo_update.sh

# Только одна база
bash geo_update.sh geoip
bash geo_update.sh geosite

# Внутри контейнера (установлен как /usr/local/bin/geo-update, пишет в /etc/hysteria)
docker exec hysteria2 geo-update
```

Переменные окружения: `GEO_DIR` (каталог назначения), `GEO_BASE_URL`, `GEO_CDN_BASE`,
`GEOIP_URL` и `GEOSITE_URL` (полные URL; можно задать несколько через запятую — они будут
опрашиваться в указанном порядке).

Hysteria загружает базы только при старте, поэтому после обновления перезапустите контейнер:

```bash
docker restart hysteria2
```

Пересобирать образ из-за обновления баз больше не нужно: при старте они скачиваются, если их нет.
Для принудительного обновления уже скачанных баз выполните `docker exec hysteria2 geo-update`,
затем `docker restart hysteria2`. `.dockerignore` гарантирует, что локальные `geoip.dat`/`geosite.dat`
не попадут в образ.

---

## 13. Полезные команды

```bash
docker logs -f hysteria2              # логи
docker exec -it hysteria2 bash        # войти в контейнер
docker stop hysteria2                 # остановить
docker start hysteria2                # запустить
docker restart hysteria2              # перезапустить
docker exec hysteria2 keygen devuser1 # клиентская ссылка
docker exec hysteria2 geo-update      # обновить geoip/geosite внутри контейнера
```

---

## Примечания по безопасности

- Замените секрет `trafficStats.secret` и сгенерированные пароли на свои.
- `server.env`, `users.txt` и ключи исключены из Git и образа; не коммитьте их.
- `insecure=1` отключает проверку сертификата — используйте только с доверенным сервером.
- Traffic Stats API слушает на `:9999`; при необходимости ограничьте доступ файрволом.
