#!/bin/bash
# Вызывается Hysteria: auth.sh <addr> <auth> <tx>
# auth приходит в формате "username:password"

USERS_FILE="/etc/hysteria/users.txt"

if [ $# -lt 2 ]; then
    exit 1
fi

AUTH="$2"

# Разделяем username:password (берём только первое :)
USERNAME="${AUTH%%:*}"
PASSWORD="${AUTH#*:}"

if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ] || [ "$USERNAME" = "$PASSWORD" ]; then
    exit 1
fi

if [ ! -f "$USERS_FILE" ]; then
    exit 1
fi

# Ищем точное совпадение username:password (регистр username не важен)
while IFS= read -r line || [ -n "$line" ]; do
    # защита от CRLF (файл редактировали в Windows)
    line="${line%$'\r'}"
    # пропускаем пустые строки и комментарии
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

    stored_user="${line%%:*}"
    stored_pass="${line#*:}"
    # у пароля тоже отрезаем возможный CR
    stored_pass="${stored_pass%$'\r'}"

    if [ "${stored_user,,}" = "${USERNAME,,}" ] && [ "$stored_pass" = "$PASSWORD" ]; then
        # Успех: выводим id пользователя (для логов и trafficStats)
        echo "$stored_user"
        exit 0
    fi
done < "$USERS_FILE"

exit 1
