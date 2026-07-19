#!/bin/sh
set -e

mkdir -p /var/bal/.bitcoin
chown bal:bal /var/bal /var/bal/.bitcoin 2>/dev/null || true

PUSHER_NETWORK="${BAL_PUSHER_NETWORK:-bitcoin}"

echo "[entrypoint] Starting bal-server on ${BAL_SERVER_BIND_ADDRESS:-127.0.0.1}:${BAL_SERVER_BIND_PORT:-9137}"
echo "[entrypoint] Starting bal-pusher (network: ${PUSHER_NETWORK})"

su -s /bin/sh bal -c '/usr/local/bin/bal-server' &
SERVER_PID=$!

su -s /bin/sh bal -c "/usr/local/bin/bal-pusher ${PUSHER_NETWORK}" &
PUSHER_PID=$!

cleanup() {
    echo "[entrypoint] Shutting down..."
    kill $SERVER_PID $PUSHER_PID 2>/dev/null
    wait
}
trap cleanup TERM INT

wait
