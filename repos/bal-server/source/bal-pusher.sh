#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

dev_only_guard "bal-pusher.sh" \
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

export BAL_SERVER_DB_FILE="$WORKING_DIR/bal.db"
export BAL_SERVER_DB_BACKEND="postgresql"
#export BAL_PUSHER_BITCOIN_COOKIE_FILE=/~/.bitcoin/.cookie
#export BAL_PUSHER_REGTEST_COOKIE_FILE=/~/.bitcoin/regtest/.cookie
#export BAL_PUSHER_TESTNET_COOKIE_FILE=/~/.bitcoin/testnet3/.cookie
#export BAL_PUSHER_SIGNET_COOKIE_FILE=/~/.bitcoin/signet/.cookie

export BAL_PUSHER_REGTEST_ZMQ_HASHBLOCK=tcp://127.0.0.1:21332
export BAL_PUSHER_SEND_STATS=true
export WELIST_SERVER_URL=http://localhost:8086
export WELIST_SKIP_URL_VALIDATION=true
# Local development defaults to the legacy v1 signature, verified by the
# service against GET /.pub_key.pem: a localhost URL cannot use the canonical
# URL policy required by v2/v3. v2 uses canonical JSON with /.pub_key.pem key
# discovery; v3 uses canonical JSON with DNSSEC or onion key discovery. Both
# require BAL_SERVER_URL to be a public canonical origin, so override the
# version only after changing the URL below. The WeTime service accepts legacy
# pings by default; for loopback addresses it also needs
# WELIST_ALLOW_LOCALHOST_PING=true.
export WELIST_PING_SIG_VERSION="${WELIST_PING_SIG_VERSION:-1}"
export BAL_SERVER_URL="http://127.0.0.1:9133"
export SSL_KEY_PATH="$WORKING_DIR/private_key.pem"

if [ "$BAL_SERVER_DB_BACKEND" = "postgresql" ]; then
    . "$SCRIPT_DIR/pg-server.sh"
fi

cargo run --bin=bal-pusher regtest --features=pusher
