#!/usr/bin/env bash
# Second bal-pusher instance: talks to the second bal-server and database.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

dev_only_guard "bal-pusher-2.sh" \
    "This launcher disables WELIST URL validation and defaults to the legacy v1 ping signature." "$@"

# Load local, gitignored environment overrides (e.g. BAL_PG_PASSWORD).
if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$SCRIPT_DIR/.env"
    set +a
fi

WORKING_DIR=$(pwd)

export RUST_LOG=trace

export BAL_SERVER_DB_FILE="$WORKING_DIR/bal2.db"
export BAL_SERVER_DB_BACKEND="postgresql"
#export BAL_PUSHER_BITCOIN_COOKIE_FILE=/~/.bitcoin/.cookie
#export BAL_PUSHER_REGTEST_COOKIE_FILE=/~/.bitcoin/regtest/.cookie
#export BAL_PUSHER_TESTNET_COOKIE_FILE=/~/.bitcoin/testnet3/.cookie
#export BAL_PUSHER_SIGNET_COOKIE_FILE=/~/.bitcoin/signet/.cookie

export BAL_PUSHER_REGTEST_ZMQ_HASHBLOCK=tcp://127.0.0.1:21332
# Only the first pusher reports executor stats, to avoid double reporting for
# the same executor key on the welist service.
export BAL_PUSHER_SEND_STATS=false
export WELIST_SERVER_URL=http://localhost:8086
export WELIST_SKIP_URL_VALIDATION=true
export BAL_SERVER_URL="http://127.0.0.1:9134"
export SSL_KEY_PATH="$WORKING_DIR/private_key.pem"

# Instance-specific settings must win over .env, which defines BAL_PG_DB=baldb.
export BAL_PG_DB="baldb2"
unset BAL_SERVER_PG_DSN

if [ "$BAL_SERVER_DB_BACKEND" = "postgresql" ]; then
    . "$SCRIPT_DIR/pg-server.sh"
fi

cargo run --bin=bal-pusher regtest --features=pusher
