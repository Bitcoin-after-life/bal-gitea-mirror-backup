#!/usr/bin/env bash
# Second bal-server instance: own port, own database, same key pair.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib.sh"

dev_only_guard "bal-server-2.sh" \
    "This launcher uses trace logging, the local PostgreSQL database, loopback binding, and the development key pair." "$@"

# Load local, gitignored environment overrides (e.g. BAL_PG_PASSWORD).
if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$SCRIPT_DIR/.env"
    set +a
fi

WORKING_DIR=$(pwd)
if [ ! -f "$WORKING_DIR/public_key.pem" ]; then
  echo_i "creating keypairs"
  openssl genpkey -algorithm ED25519 -out private_key.pem
  openssl pkey -in private_key.pem -pubout -out public_key.pem
fi

export RUST_LOG="trace"
export BAL_SERVER_DB_FILE="$WORKING_DIR/bal2.db"
export BAL_SERVER_INFO="BAL devel willexecutor server 2"
export BAL_SERVER_BIND_ADDRESS="127.0.0.1"
export BAL_SERVER_BIND_PORT=9134
export BAL_SERVER_PUB_KEY_PATH="$WORKING_DIR/public_key.pem"
export BAL_SERVER_EXPOSE_STATS=true

# Dedicated xpub from the regtest descriptor wallet "server2" (BIP84 account),
# so that the two instances never derive the same deposit addresses.
export BAL_SERVER_REGTEST_ADDRESS="tpubDCPHz2qeRgsqYMKkrEidz5NMFmCv36NX1C3FFYjg2hnvgrBTiw37uXxVnrvF6y6dswhJrQF9FTFW3jP334mMrXD5yndoHrNmjgToAaBP7iJ"
export BAL_SERVER_REGTEST_FIXED_FEE=1000
export BAL_SERVER_DB_BACKEND="postgresql"

# Instance-specific settings must win over .env, which defines BAL_PG_DB=baldb.
export BAL_PG_DB="baldb2"
unset BAL_SERVER_PG_DSN

if [ "$BAL_SERVER_DB_BACKEND" = "postgresql" ]; then
    . "$SCRIPT_DIR/pg-server.sh"
fi

cargo run --bin=bal-server
