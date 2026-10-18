#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

PG_CONTAINER="bal-pg"
PG_USER="${BAL_PG_USER:-bal}"
PG_DB="${BAL_PG_DB:-baldb}"
# The password MUST come from the environment; never hardcode it.
PG_PASSWORD="${BAL_PG_PASSWORD:?BAL_PG_PASSWORD must be set (see .env.example)}"
PG_DSN="${BAL_SERVER_PG_DSN:-postgres://${PG_USER}:${PG_PASSWORD}@localhost:5432/${PG_DB}}"

if docker ps --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    echo_i "PostgreSQL container '$PG_CONTAINER' is already running"
elif docker ps -a --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    echo_i "Starting existing PostgreSQL container '$PG_CONTAINER'..."
    docker start "$PG_CONTAINER" >/dev/null 2>&1
else
    echo_i "Creating PostgreSQL container '$PG_CONTAINER'..."
    docker run -d --name "$PG_CONTAINER" \
        -e POSTGRES_USER="$PG_USER" \
        -e POSTGRES_PASSWORD="$PG_PASSWORD" \
        -e POSTGRES_DB="$PG_DB" \
        -p 5432:5432 postgres:16-alpine
fi

echo_i "Waiting for PostgreSQL..."
until docker exec "$PG_CONTAINER" pg_isready -U "$PG_USER" >/dev/null 2>&1; do
    sleep 1
done
# Do not echo the DSN: it embeds the password.
echo_s "PostgreSQL ready (user=$PG_USER, db=$PG_DB, host=localhost:5432)"
export BAL_SERVER_PG_DSN="$PG_DSN"
