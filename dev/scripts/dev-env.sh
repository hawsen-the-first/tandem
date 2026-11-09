# Source this file from the repository root: `. dev/scripts/dev-env.sh <migrate|app>`
# Exports the TANDEM_* variables for running a Tandem process against the dev database.
# shellcheck shell=bash

_role="${1:?usage: . dev/scripts/dev-env.sh <migrate|app>}"
case "$_role" in migrate | app) ;; *) echo "unknown role: $_role" >&2; return 1 ;; esac
[ -f dev/.env ] || { echo "dev/.env is missing: run 'just dev-init' first" >&2; return 1; }

set -a
# shellcheck disable=SC1091
. dev/.env
set +a

if [ "$_role" = migrate ]; then _pw="$TANDEM_MIGRATE_PASSWORD"; else _pw="$TANDEM_APP_PASSWORD"; fi
export TANDEM_DATABASE_URL="postgres://tandem_${_role}:${_pw}@localhost:${TANDEM_DEV_DB_PORT}/tandem?sslmode=verify-full"
export TANDEM_DATABASE_CA_FILE="$PWD/dev/secrets/certs/ca.crt"
export TANDEM_MASTER_KEY_FILE="$PWD/dev/secrets/master.key"
export TANDEM_PUBLIC_URL="http://localhost:5173"
export TANDEM_LISTEN_ADDR="127.0.0.1:8080"
export TANDEM_LOG_LEVEL="${TANDEM_LOG_LEVEL:-debug}"
unset _role _pw
