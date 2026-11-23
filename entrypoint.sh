#!/bin/bash
set -e

cd /etc/hysteria

# Базы GeoIP/GeoSite не запечены в образ: скачиваем недостающие ДО старта сервера.
# Hysteria читает их при запуске, поэтому это необходимо сделать до exec hysteria.
if [ -x /usr/local/bin/geo-update ]; then
    for db in geoip geosite; do
        if [ ! -s "/etc/hysteria/${db}.dat" ]; then
            echo "База ${db}.dat не найдена — скачиваем..."
            if ! /usr/local/bin/geo-update "$db"; then
                echo "Не удалось получить ${db}.dat, запуск прерван" >&2
                exit 1
            fi
        fi
    done
else
    echo "Ошибка: /usr/local/bin/geo-update не найден в образе" >&2
    exit 1
fi

# Генерация сертификатов (оставляем как было)
rm -f ca.crt server.crt server.csr extfile.cnf ca.srl

if [ -z "$SERVER_IP" ]; then
    echo "SERVER_IP is not set"
    exit 1
fi

openssl genrsa -out ca.key 2048

openssl req -new -x509 -days 3650 \
    -key ca.key \
    -subj "/CN=Hysteria Root CA" \
    -out ca.crt

openssl req -newkey rsa:2048 -nodes \
    -keyout server.key \
    -subj "/CN=${SERVER_IP}" \
    -out server.csr

printf "subjectAltName=IP:${SERVER_IP}" > extfile.cnf

openssl x509 -req -days 3650 \
    -in server.csr \
    -CA ca.crt \
    -CAkey ca.key \
    -CAcreateserial \
    -out server.crt \
    -extfile extfile.cnf

chmod 600 server.key ca.key

# Создаём users.txt если его нет (можно с начальными пользователями)
if [ ! -f /etc/hysteria/users.txt ]; then
    cat > /etc/hysteria/users.txt << 'EOF'
# username:password
devuser1:iphusae2aigh7leem1ohshegh3oo
EOF
    chmod 600 /etc/hysteria/users.txt
fi

# Права на auth.sh
chmod +x /etc/hysteria/auth.sh

exec hysteria server -c /etc/hysteria/config.yaml
