# Hysteria 2 — Docker

Docker-образ [Hysteria 2](https://github.com/apernet/hysteria) с автоматической генерацией сертификатов и управлением пользователями.

## Возможности

- Автоматическая генерация самоподписанных сертификатов при старте
- Авторизация через `users.txt` (command-based auth)
- CLI-утилита `hy2user` для управления пользователями
- BBR конгестийный контроль
- Masquerade (маскировка под HTTPS)
- Traffic Stats на порту 9999

## Структура проекта

```text
Hysteria2/
├── Dockerfile
├── config.yaml
├── entrypoint.sh
├── auth.sh
├── hy2user
├── tune_network.sh
├── server.env
├── .gitignore
└── .dockerignore
```

---

## 1. `server.env`

```bash
SERVER_IP=ваш_IP_адрес
```

Для изменения IP позже достаточно поменять это значение.

---

## 2. `config.yaml`

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
    url: https://yahoo.com/
    rewriteHost: true

congestion:
  type: bbr
  bbrProfile: conservative

trafficStats:
  listen: :9999
  secret: "miepoa6xoonal4LaSahY0"

quic:
  initStreamReceiveWindow: 8388608
  maxStreamReceiveWindow: 8388608
  initConnReceiveWindow: 20971520
  maxConnReceiveWindow: 20971520
  maxIdleTimeout: 30s
  keepAlivePeriod: 10s
  udpIdleTimeout: 60s
  maxIncomingStreams: 512
  disablePathMTUDiscovery: true
```

---

## 3. `entrypoint.sh`

```bash
#!/bin/bash
set -e

cd /etc/hysteria

rm -f ca.crt server.crt server.csr extfile.cnf ca.srl

if [ -z "$SERVER_IP" ]; then
    echo "SERVER_IP is not set"
    exit 1
fi

# Root CA
openssl genrsa -out ca.key 2048

openssl req -new -x509 -days 3650 \
    -key ca.key \
    -subj "/CN=Hysteria Root CA" \
    -out ca.crt

# Server key + CSR
openssl req -newkey rsa:2048 -nodes \
    -keyout server.key \
    -subj "/CN=${SERVER_IP}" \
    -out server.csr

printf "subjectAltName=IP:${SERVER_IP}" > extfile.cnf

# Подпись сертификата
openssl x509 -req -days 3650 \
    -in server.csr \
    -CA ca.crt \
    -CAkey ca.key \
    -CAcreateserial \
    -out server.crt \
    -extfile extfile.cnf

chmod 600 server.key ca.key

# Создаём users.txt если его нет
if [ ! -f /etc/hysteria/users.txt ]; then
    cat > /etc/hysteria/users.txt << 'EOF'
# username:password
devuser1:iphusae2aigh7leem1ohshegh3oo
EOF
    chmod 600 /etc/hysteria/users.txt
fi

chmod +x /etc/hysteria/auth.sh

exec hysteria server -c /etc/hysteria/config.yaml
```

---

## 4. `auth.sh`

```bash
#!/bin/bash
# Вызывается Hysteria: auth.sh <addr> <auth> <tx>
# auth приходит в формате "username:password"

USERS_FILE="/etc/hysteria/users.txt"

if [ $# -lt 2 ]; then
    exit 1
fi

AUTH="$2"
USERNAME="${AUTH%%:*}"
PASSWORD="${AUTH#*:}"

if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ] || [ "$USERNAME" = "$PASSWORD" ]; then
    exit 1
fi

if [ ! -f "$USERS_FILE" ]; then
    exit 1
fi

while IFS= read -r line || [ -n "$line" ]; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

    stored_user="${line%%:*}"
    stored_pass="${line#*:}"

    if [ "${stored_user,,}" = "${USERNAME,,}" ] && [ "$stored_pass" = "$PASSWORD" ]; then
        echo "$stored_user"
        exit 0
    fi
done < "$USERS_FILE"

exit 1
```

---

## 5. `Dockerfile`

```dockerfile
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
```

---

## 6. Исправление CRLF

Если файлы создавались в Windows/WSL:

```bash
sed -i 's/\r$//' entrypoint.sh auth.sh
```

Или:

```bash
sudo apt install dos2unix
dos2unix entrypoint.sh auth.sh
```

---

## 7. Сборка образа

```bash
sudo docker build --no-cache -t hysteria2 .
```

---

## 8. Удаление старого контейнера

```bash
sudo docker rm -f hysteria2
```

---

## 9. Запуск контейнера

```bash
sudo docker run -d \
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

С volume для `users.txt` (изменения сохраняются после пересборки):

```bash
sudo docker run -d \
  --name hysteria2 \
  --restart unless-stopped \
  --ulimit rtprio=99 \
  --cap-add NET_ADMIN \
  --cap-add SYS_NICE \
  --env-file server.env \
  -p 443:443/udp \
  -p 9999:9999/tcp \
  -v $(pwd)/users.txt:/etc/hysteria/users.txt \
  hysteria2
```

---

## 10. Управление пользователями

### Через `hy2user` (внутри контейнера)

```bash
# Добавить пользователя (пароль генерируется автоматически)
docker exec -it hysteria2 hy2user add client1

# Добавить с указанием пароля
docker exec -it hysteria2 hy2user add client2 MyStrongPass123

# Сменить пароль
docker exec -it hysteria2 hy2user pass client1

# Сменить на конкретный пароль
docker exec -it hysteria2 hy2user pass client1 NewPass456

# Удалить пользователя
docker exec -it hysteria2 hy2user del client2

# Список всех пользователей
docker exec -it hysteria2 hy2user list
```

### Через `users.txt` напрямую

Изменения применяются **мгновенно**, перезапуск не нужен.

```bash
# Добавить
echo "newuser:SuperSecurePass123" >> users.txt

# Сменить пароль
sed -i 's/^user1:.*/user1:новый_пароль/' users.txt

# Удалить
sed -i '/^user3:/d' users.txt

# Посмотреть всех
cat users.txt
```

---

## 11. Проверка работы

```bash
# Статус контейнера
sudo docker ps

# Логи
sudo docker logs -f hysteria2

# Порт UDP
sudo ss -lunp | grep 443

# Порты Docker
sudo docker port hysteria2
```

Ожидаемый вывод в логах:

```text
server mode
listening UDP on [::]:443
```

---

## 12. Firewall (UFW)

```bash
sudo ufw allow 443/udp
sudo ufw reload
sudo ufw status
```

---

## 13. Оптимизация сети

На хосте выполнить:

```bash
sudo bash tune_network.sh
```

Включает BBR и оптимизирует параметры сети.

---

## 14. Пример клиентского URI

```text
hysteria2://user1:пароль@ВАШ_IP:443/?insecure=1&sni=ВАШ_IP#Hysteria2
```

> `%23` = `#` в URL-кодировке. `insecure=1` обязателен для self-signed сертификата.

---

## 15. Смена IP сервера

1. Отредактируйте `server.env`
2. Перезапустите контейнер:

```bash
sudo docker restart hysteria2
```

---

## 16. Полезные команды

```bash
# Логи
sudo docker logs -f hysteria2

# Войти внутрь контейнера
sudo docker exec -it hysteria2 bash

# Остановить / Запустить / Перезапустить
sudo docker stop hysteria2
sudo docker start hysteria2
sudo docker restart hysteria2
```

---

## Порт

- **443/udp** — основной порт Hysteria 2
- **9999/tcp** — traffic stats (опционально)
