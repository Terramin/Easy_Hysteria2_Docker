#!/bin/bash
# Обновление GeoIP/GeoSite баз из Loyalsoldier/v2ray-rules-dat.
# Формат .dat (protobuf v2ray) совместим с Hysteria 2.
#
# Использование:
#   geo_update.sh            # обновить оба файла
#   geo_update.sh geoip      # только geoip.dat
#   geo_update.sh geosite    # только geosite.dat
#
# Переменные окружения:
#   GEO_DIR       — куда положить файлы (по умолчанию /etc/hysteria в контейнере,
#                   иначе каталог со скриптом)
#   GEO_BASE_URL  — основной базовый URL (по умолчанию GitHub releases latest)
#   GEO_CDN_BASE  — дополнительный базовый URL (по умолчанию jsDelivr @release)
#   GEOIP_URL     — полный URL для geoip.dat (переопределяет списки, можно через запятую)
#   GEOSITE_URL   — полный URL для geosite.dat (переопределяет списки, можно через запятую)
#
# По умолчанию источники опрашиваются по очереди, пока один не скачается и не пройдёт проверку:
#   1) https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/<file>
#   2) https://cdn.jsdelivr.net/gh/Loyalsoldier/v2ray-rules-dat@release/<file>

set -e

usage() {
    echo "Использование: $0 [geoip|geosite]" >&2
}

case "${1:-}" in
    "")        TARGETS=(geoip geosite) ;;
    geoip)     TARGETS=(geoip) ;;
    geosite)   TARGETS=(geosite) ;;
    -h|--help) usage; exit 0 ;;
    *)         echo "Ошибка: неизвестный аргумент: $1" >&2; usage; exit 1 ;;
esac

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

GEO_DIR="${GEO_DIR:-}"
if [ -z "$GEO_DIR" ]; then
    if [ -f /etc/hysteria/config.yaml ] && [ -d /etc/hysteria ]; then
        GEO_DIR=/etc/hysteria
    else
        GEO_DIR="$SCRIPT_DIR"
    fi
fi

if [ ! -d "$GEO_DIR" ]; then
    echo "Ошибка: каталог не существует: $GEO_DIR" >&2
    exit 1
fi
if [ ! -w "$GEO_DIR" ]; then
    echo "Ошибка: нет прав на запись в $GEO_DIR" >&2
    exit 1
fi

BASE="${GEO_BASE_URL:-https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download}"
CDN_BASE="${GEO_CDN_BASE:-https://cdn.jsdelivr.net/gh/Loyalsoldier/v2ray-rules-dat@release}"

# Список URL-источников: сначала основной, затем дополнительный (failover).
urls_from() {
    local explicit="$1" file="$2"
    local -a out=()
    if [ -n "$explicit" ]; then
        IFS=',' read -ra out <<< "$explicit"
    else
        out=("$BASE/$file" "$CDN_BASE/$file")
    fi
    # Убираем пустые элементы и пробелы
    local -a clean=() item
    for item in "${out[@]}"; do
        item="$(echo "$item" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
        [ -n "$item" ] && clean+=("$item")
    done
    printf '%s\n' "${clean[@]}"
}

mapfile -t GEOIP_URLS < <(urls_from "${GEOIP_URL:-}" geoip.dat)
mapfile -t GEOSITE_URLS < <(urls_from "${GEOSITE_URL:-}" geosite.dat)

if ! command -v curl >/dev/null 2>&1; then
    echo "Ошибка: нужен curl" >&2
    exit 1
fi

# Минимальная проверка, что скачался именно .dat, а не HTML-страница ошибки.
validate_dat() {
    local file="$1" size first
    [ -f "$file" ] && [ -s "$file" ] || return 1
    size="$(wc -c < "$file" | tr -d '[:space:]')"
    if [ "$size" -lt 100000 ]; then
        echo "Ошибка: файл слишком маленький ($size байт)" >&2
        return 1
    fi
    # proto: первое поле GeoIPList/GeoSiteList — length-delimited (0x0A)
    first="$(od -An -tu1 -N1 "$file" | tr -d '[:space:]')"
    if [ "$first" != "10" ]; then
        echo "Ошибка: похоже, это не protobuf .dat (первый байт $first)" >&2
        return 1
    fi
    return 0
}

for kind in "${TARGETS[@]}"; do
    if [ "$kind" = "geoip" ]; then
        urls=("${GEOIP_URLS[@]}")
    else
        urls=("${GEOSITE_URLS[@]}")
    fi
    target="$GEO_DIR/$kind.dat"
    tmp="$(mktemp "${TMPDIR:-/tmp}/${kind}.dat.XXXXXX")"

    ok=0
    for url in "${urls[@]}"; do
        echo "Скачиваем $kind: $url"
        if curl -fsSL --retry 2 --retry-delay 2 --connect-timeout 10 --max-time 300 \
                -o "$tmp" "$url" && validate_dat "$tmp"; then
            ok=1
            break
        fi
        echo "Источник не сработал, пробуем следующий..." >&2
    done

    if [ "$ok" -eq 0 ]; then
        rm -f "$tmp"
        echo "Ошибка: ни один источник не отдал корректный $kind.dat, старый файл не тронут" >&2
        exit 1
    fi

    if [ -f "$target" ]; then
        cp -p "$target" "${target}.bak"
        echo "Резервная копия: ${target}.bak"
    fi

    mv -f "$tmp" "$target"
    echo "Обновлён: $target ($(wc -c < "$target" | tr -d '[:space:]') байт)"
done

echo "Готово. Hysteria читает базы только при старте — перезапустите сервер/контейнер, чтобы применить."
