#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

PG_CONTAINER="bal-pg"
PG_IMAGE="${BAL_PG_IMAGE:-postgres:16-alpine}"
PG_VOLUME="${BAL_PG_VOLUME:-bal-pg-data}"
PG_USER="${BAL_PG_USER:-bal}"
PG_DB="${BAL_PG_DB:-baldb}"
# The password MUST come from the environment; never hardcode it.
PG_PASSWORD="${BAL_PG_PASSWORD:?BAL_PG_PASSWORD must be set (see .env.example)}"

# Publish the port on the loopback interface only by default. Set
# BAL_PG_BIND_ADDR=0.0.0.0 to expose PostgreSQL on every interface (not
# recommended: the database would be reachable from the network).
PG_BIND_ADDR="${BAL_PG_BIND_ADDR:-127.0.0.1}"
PG_PORT=5432

# Address the local clients use to reach the container. When the port is
# published on all interfaces, loopback is still the safest connect address.
case "$PG_BIND_ADDR" in
    0.0.0.0|::|"") PG_CONNECT_ADDR="127.0.0.1" ;;
    *) PG_CONNECT_ADDR="$PG_BIND_ADDR" ;;
esac
PG_DSN="${BAL_SERVER_PG_DSN:-postgres://${PG_USER}:${PG_PASSWORD}@${PG_CONNECT_ADDR}:${PG_PORT}/${PG_DB}}"

# Print the host IP the container port is currently published on.
# An empty value means Docker publishes it on all interfaces.
pg_container_bind_addr() {
    docker inspect --format '{{range $port, $bindings := .HostConfig.PortBindings}}{{range $bindings}}{{.HostIp}}{{end}}{{end}}' "$PG_CONTAINER" 2>/dev/null
}

warn_if_public() {
    case "$PG_BIND_ADDR" in
        127.0.0.1|::1|localhost) ;;
        *)
            echo_w "WARNING: PostgreSQL is published on ${PG_BIND_ADDR}:${PG_PORT} and is reachable from the network."
            echo_w "         Set BAL_PG_BIND_ADDR=127.0.0.1 unless remote access is really required."
            ;;
    esac
}

# Fail closed when an existing container publishes the port elsewhere.
# Docker cannot change port bindings of a running/stopped container.
require_expected_bind_addr() {
    local current
    current=$(pg_container_bind_addr)
    [ "$current" = "$PG_BIND_ADDR" ] && return 0

    echo_e "Container '$PG_CONTAINER' publishes port ${PG_PORT} on '${current:-0.0.0.0}', expected '$PG_BIND_ADDR'."
    echo_e "Docker cannot change port bindings of an existing container."
    echo_e "Recreate it while keeping its data with:"
    echo_e "    $(basename "${BASH_SOURCE[0]}") --recreate"
    exit 1
}

# Recreate the container so that the new port binding takes effect.
# The data volume is reused (or copied into BAL_PG_VOLUME when it changes).
recreate_container() {
    local mount_type old_volume
    mount_type=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{.Type}}{{end}}{{end}}' "$PG_CONTAINER" 2>/dev/null || true)
    old_volume=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{.Name}}{{end}}{{end}}' "$PG_CONTAINER" 2>/dev/null || true)

    if [ "$mount_type" = "bind" ]; then
        echo_e "Container '$PG_CONTAINER' uses a bind mount for its data; refusing to recreate it automatically."
        exit 1
    fi

    echo_i "Removing container '$PG_CONTAINER' (its data volume is preserved)..."
    docker rm -f "$PG_CONTAINER" >/dev/null

    if [ -n "$old_volume" ] && [ "$old_volume" != "$PG_VOLUME" ]; then
        echo_i "Migrating data from volume '$old_volume' to '$PG_VOLUME'..."
        docker volume create "$PG_VOLUME" >/dev/null
        docker run --rm --entrypoint sh \
            -v "$old_volume:/from:ro" \
            -v "$PG_VOLUME:/to" \
            "$PG_IMAGE" -c 'cp -a /from/. /to/'
    fi
}

if [ "${1:-}" = "--recreate" ]; then
    if docker ps -a --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
        recreate_container
    fi
fi

warn_if_public

if docker ps --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    echo_i "PostgreSQL container '$PG_CONTAINER' is already running"
    require_expected_bind_addr
elif docker ps -a --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    require_expected_bind_addr
    echo_i "Starting existing PostgreSQL container '$PG_CONTAINER'..."
    docker start "$PG_CONTAINER" >/dev/null 2>&1
else
    echo_i "Creating PostgreSQL container '$PG_CONTAINER'..."
    docker run -d --name "$PG_CONTAINER" \
        -e POSTGRES_USER="$PG_USER" \
        -e POSTGRES_PASSWORD="$PG_PASSWORD" \
        -e POSTGRES_DB="$PG_DB" \
        -p "${PG_BIND_ADDR}:${PG_PORT}:5432" \
        -v "${PG_VOLUME}:/var/lib/postgresql/data" \
        "$PG_IMAGE" >/dev/null
fi

echo_i "Waiting for PostgreSQL..."
until docker exec "$PG_CONTAINER" pg_isready -U "$PG_USER" >/dev/null 2>&1; do
    sleep 1
done

# The container is created with a single database, so create the requested one
# if it does not exist yet (e.g. a second instance with its own database).
if ! docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d postgres -tAc \
    "SELECT 1 FROM pg_database WHERE datname = '$PG_DB'" | grep -q 1; then
    echo_i "Creating database '$PG_DB'..."
    docker exec "$PG_CONTAINER" createdb -U "$PG_USER" "$PG_DB"
fi

# Do not echo the DSN: it embeds the password.
echo_s "PostgreSQL ready (user=$PG_USER, db=$PG_DB, host=${PG_CONNECT_ADDR}:${PG_PORT})"
export BAL_SERVER_PG_DSN="$PG_DSN"
