#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

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
export BAL_SERVER_URL="http://127.0.0.1:9133"
export SSL_KEY_PATH="$WORKING_DIR/private_key.pem"

if [ "$BAL_SERVER_DB_BACKEND" = "postgresql" ]; then
    . "$SCRIPT_DIR/pg-server.sh"
fi

cargo run --bin=bal-pusher regtest --features=pusher
