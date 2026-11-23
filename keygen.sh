#!/bin/bash
# Генерация клиентского URI Hysteria 2 по имени пользователя.
#
# Формат вывода:
#   hysteria2://user:password@IP:PORT/?insecure=1&sni=<host из masquerade>#Hysteria2
#
# Использование:
#   keygen.sh <username>
#
# Откуда берутся данные:
#   - users.txt: $USERS_FILE, иначе /etc/hysteria/users.txt, иначе ./users.txt
#   - IP:        SERVER_IP из env (или из server.env рядом со скриптом)
#   - порт:      SERVER_PORT из env (или из server.env), если нет — 443
#   - SNI:       хост из masquerade.proxy.url в config.yaml, если нет — SERVER_IP.
#                Переопределяется переменной SNI.
#   - config:    $CONFIG_FILE, иначе /etc/hysteria/config.yaml, иначе ./config.yaml

set -e

usage() {
    echo "Использование: $0 <username>" >&2
}

USERNAME="$1"

if [ -z "$USERNAME" ]; then
    usage
    exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# --- Чтение переменных окружения (env имеет приоритет над server.env) ------
clean_value() {
    local v="$1"
    local sq="'"
    v="${v%$'\r'}"
    v="${v#\"}"; v="${v%\"}"
    v="${v#$sq}"; v="${v%$sq}"
    printf '%s' "$v" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

env_value() {
    local key="$1" v
    v="$(printenv "$key" 2>/dev/null || true)"
    if [ -z "$v" ] && [ -f "$SCRIPT_DIR/server.env" ]; then
        v="$(sed -n "s/^[[:space:]]*${key}[[:space:]]*=[[:space:]]*//p" "$SCRIPT_DIR/server.env" | head -n 1)"
    fi
    if [ -z "$v" ] && [ -f /etc/hysteria/server.env ]; then
        v="$(sed -n "s/^[[:space:]]*${key}[[:space:]]*=[[:space:]]*//p" /etc/hysteria/server.env | head -n 1)"
    fi
    clean_value "$v"
}

SERVER_HOST="$(env_value SERVER_IP)"
if [ -z "$SERVER_HOST" ]; then
    echo "Ошибка: SERVER_IP не задан в env/server.env" >&2
    exit 1
fi

PORT="$(env_value SERVER_PORT)"
[ -z "$PORT" ] && PORT=443
if [[ ! "$PORT" =~ ^[0-9]+$ ]] || [ "$PORT" -lt 1 ] || [ "$PORT" -gt 65535 ]; then
    echo "Ошибка: некорректный SERVER_PORT: $PORT" >&2
    exit 1
fi

# IPv6 в authority нужно брать в квадратные скобки
AUTHORITY_HOST="$SERVER_HOST"
if [[ "$SERVER_HOST" == *:* ]] && [[ "$SERVER_HOST" != \[*\] ]]; then
    AUTHORITY_HOST="[$SERVER_HOST]"
fi

# --- Путь к users.txt ------------------------------------------------------
if [ -n "${USERS_FILE:-}" ]; then
    USERS_PATH="$USERS_FILE"
elif [ -f /etc/hysteria/users.txt ]; then
    USERS_PATH=/etc/hysteria/users.txt
else
    USERS_PATH="$SCRIPT_DIR/users.txt"
fi

if [ ! -f "$USERS_PATH" ]; then
    echo "Ошибка: не найден users.txt: $USERS_PATH" >&2
    exit 1
fi

# --- SNI: хост из masquerade.proxy.url в config.yaml -----------------------
CONFIG_PATH="${CONFIG_FILE:-}"
if [ -z "$CONFIG_PATH" ]; then
    if [ -f /etc/hysteria/config.yaml ]; then
        CONFIG_PATH=/etc/hysteria/config.yaml
    else
        CONFIG_PATH="$SCRIPT_DIR/config.yaml"
    fi
fi

MASQ_URL=""
if [ -f "$CONFIG_PATH" ]; then
    MASQ_URL="$(awk '
        /^masquerade:[[:space:]]*$/ { in_masq = 1; next }
        in_masq && /^[^[:space:]#]/ { in_masq = 0 }
        in_masq && /^[[:space:]]*url:[[:space:]]*/ {
            sub(/^[[:space:]]*url:[[:space:]]*/, "")
            sub(/[[:space:]]+$/, "")
            print
            exit
        }
    ' "$CONFIG_PATH")"
fi

SNI_HOST="${SNI:-}"
if [ -z "$SNI_HOST" ] && [ -n "$MASQ_URL" ]; then
    MASQ_URL="$(clean_value "$MASQ_URL")"
    SNI_HOST="${MASQ_URL#*://}"      # убираем http:// или https://
    SNI_HOST="${SNI_HOST%%[/?#]*}"   # убираем путь/query/fragment
    SNI_HOST="${SNI_HOST##*@}"       # убираем userinfo
    if [[ "$SNI_HOST" =~ ^(.*):[0-9]+$ ]]; then
        SNI_HOST="${BASH_REMATCH[1]}" # убираем порт
    fi
fi

if [ -z "$SNI_HOST" ]; then
    SNI_HOST="$SERVER_HOST"
fi
SNI_HOST="$(clean_value "$SNI_HOST")"

if [ -z "$SNI_HOST" ]; then
    echo "Ошибка: не удалось определить SNI" >&2
    exit 1
fi

# --- Находим пароль пользователя -------------------------------------------
PASSWORD=""
FOUND=0
while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

    stored_user="${line%%:*}"
    stored_pass="${line#*:}"
    stored_user="$(clean_value "$stored_user")"

    if [ "${stored_user,,}" = "${USERNAME,,}" ]; then
        PASSWORD="$stored_pass"
        FOUND=1
        break
    fi
done < "$USERS_PATH"

if [ "$FOUND" -eq 0 ]; then
    echo "Ошибка: пользователь '$USERNAME' не найден в $USERS_PATH" >&2
    exit 1
fi

if [ -z "$PASSWORD" ]; then
    echo "Ошибка: у пользователя '$USERNAME' пустой пароль" >&2
    exit 1
fi

# --- Percent-encoding (RFC 3986) -------------------------------------------
# Оставляем только unreserved-символы, остальные кодируем как %XX.
urlencode() {
    local LC_ALL=C
    local s="$1" out="" i c hex
    for (( i = 0; i < ${#s}; i++ )); do
        c="${s:i:1}"
        case "$c" in
            [A-Za-z0-9._~-]) out+="$c" ;;
            *)
                printf -v hex '%%%02X' "'$c"
                out+="$hex"
                ;;
        esac
    done
    printf '%s' "$out"
}

ENC_USER="$(urlencode "$USERNAME")"
ENC_PASS="$(urlencode "$PASSWORD")"
ENC_HOST="$(urlencode "$AUTHORITY_HOST")"
# Квадратные скобки IPv6-адреса должны остаться символами authority
ENC_HOST="${ENC_HOST//%5B/\[}"
ENC_HOST="${ENC_HOST//%5D/]}"
ENC_SNI="$(urlencode "$SNI_HOST")"

printf 'hysteria2://%s:%s@%s:%s/?insecure=1&sni=%s#Hysteria2\n' \
    "$ENC_USER" "$ENC_PASS" "$ENC_HOST" "$PORT" "$ENC_SNI"
