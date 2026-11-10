#!/usr/bin/env bash
# Runs the Playwright end-to-end suite against a real stack: an isolated PostgreSQL (its own
# Compose project, volume and port), the migrated schema, the api serving the built web client, and
# the worker. Nothing here touches the development database. Needs Docker; run from anywhere.
#
#   ./dev/scripts/e2e.sh [playwright arguments...]
set -euo pipefail
cd "$(dirname "$0")/../.."

PORT_DB="${E2E_DB_PORT:-55432}"
PORT_API="${E2E_API_PORT:-8090}"
PORT_WORKER="${E2E_WORKER_PORT:-8091}"
WORK="$(mktemp -d)"
API_PID=""
WORKER_PID=""

compose() {
  COMPOSE_PROJECT_NAME=tandem-e2e TANDEM_DEV_DB_PORT="$PORT_DB" \
    docker compose -f dev/compose.yaml --env-file dev/.env "$@"
}

cleanup() {
  status=$?
  [ -n "$API_PID" ] && kill -TERM "$API_PID" 2>/dev/null || true
  [ -n "$WORKER_PID" ] && kill -TERM "$WORKER_PID" 2>/dev/null || true
  wait 2>/dev/null || true
  if [ "$status" -ne 0 ]; then
    echo "--- api log (tail) ---" >&2
    tail -n 40 "$WORK/api.log" 2>/dev/null >&2 || true
    echo "--- worker log (tail) ---" >&2
    tail -n 20 "$WORK/worker.log" 2>/dev/null >&2 || true
  fi
  compose down --volumes >/dev/null 2>&1 || true
  rm -rf "$WORK"
  exit "$status"
}
trap cleanup EXIT

./dev/scripts/dev-init.sh >/dev/null
compose down --volumes >/dev/null 2>&1 || true
compose up --detach --wait db

echo "building the web client and the services..."
(cd web && npm run build >/dev/null)
cargo build --release -p tandem-api -p tandem-worker -p tandem-cli

export TANDEM_DB_PORT_OVERRIDE="$PORT_DB"
E2E_PUBLIC_URL="http://localhost:${PORT_API}"
E2E_WEB_DIR="$PWD/web/dist"

( . dev/scripts/dev-env.sh migrate && ./target/release/tandem migrate up )

(
  . dev/scripts/dev-env.sh app
  # dev-env.sh sets development defaults; the e2e values are applied after it.
  export TANDEM_PUBLIC_URL="$E2E_PUBLIC_URL" TANDEM_WEB_DIR="$E2E_WEB_DIR" TANDEM_LOG_LEVEL=warn
  export TANDEM_LISTEN_ADDR="127.0.0.1:${PORT_API}"
  # The browser reaches the api from loopback; tests present distinct client addresses through
  # X-Forwarded-For so one spec file cannot use up another file's per-source allowance.
  export TANDEM_TRUSTED_PROXIES="127.0.0.1/32,::1/128"
  export TANDEM_WORKER_LISTEN_ADDR="127.0.0.1:${PORT_WORKER}"
  ./target/release/tandem-api >"$WORK/api.log" 2>&1 &
  echo $! >"$WORK/api.pid"
  ./target/release/tandem-worker >"$WORK/worker.log" 2>&1 &
  echo $! >"$WORK/worker.pid"
)
API_PID="$(cat "$WORK/api.pid")"
WORKER_PID="$(cat "$WORK/worker.pid")"

for _ in $(seq 1 120); do
  if curl -fs "http://127.0.0.1:${PORT_API}/readyz" >/dev/null 2>&1 \
    && curl -fs "http://127.0.0.1:${PORT_WORKER}/readyz" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
curl -fs "http://127.0.0.1:${PORT_API}/readyz" >/dev/null || { echo "the api did not become ready" >&2; exit 1; }

cd web
E2E_BASE_URL="http://localhost:${PORT_API}" npx playwright test "$@"
