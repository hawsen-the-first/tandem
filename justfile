# Tandem developer commands. Run `just` to list them. Inside `nix develop` all tools are available.
set shell := ["bash", "-euo", "pipefail", "-c"]

compose := "docker compose -f dev/compose.yaml --env-file dev/.env"

# List recipes
default:
    @just --list

# One-time: generate dev secrets and certificates
dev-init:
    ./dev/scripts/dev-init.sh

# Start PostgreSQL with TLS (and the one-shot certificate generator on first start)
dev-up:
    @test -f dev/.env || { echo "dev/.env is missing: run 'just dev-init' first" >&2; exit 1; }
    {{ compose }} up --detach --wait db

# Stop the development containers (data is kept)
dev-down:
    {{ compose }} down

# Delete the database and certificate volumes and re-generate everything
dev-reset:
    @test -f dev/.env && {{ compose }} down --volumes || true
    rm -rf dev/.env dev/secrets
    ./dev/scripts/dev-init.sh

# Apply migrations under the migration role
migrate:
    . dev/scripts/dev-env.sh migrate && cargo run --quiet -p tandem-cli -- migrate up

# Revert the most recent migration
migrate-down:
    . dev/scripts/dev-env.sh migrate && cargo run --quiet -p tandem-cli -- migrate down

# Run the api service natively
api:
    . dev/scripts/dev-env.sh app && cargo run -p tandem-api

# Run the worker service natively
worker:
    . dev/scripts/dev-env.sh app && cargo run -p tandem-worker

# Run the web client dev server (proxies to the api)
web:
    cd web && npm run dev

# Rust tests (integration tests start throw-away PostgreSQL containers)
test:
    cargo test --workspace

# Performance smoke test (PF-1 to PF-3): needs Docker
perf:
    cargo test -p tandem-api --test perf --release -- --ignored --nocapture

# API contract: routes match the OpenAPI description, committed copy is current
contract:
    cargo test -p tandem-api --test contract

# Web unit and component tests
test-web:
    cd web && npm test

# End-to-end tests (Chromium and Firefox, with accessibility checks) against an isolated stack; needs Docker
e2e *args:
    ./dev/scripts/e2e.sh {{args}}

# Formatting, lint and type checks
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cd web && npm run lint && npm run format:check && npm run typecheck

# Apply formatting
fmt:
    cargo fmt --all
    cd web && npm run format

# Dependency, licence and unused-dependency checks
audit:
    cargo audit
    cargo deny check
    cargo machete
    cd web && npm audit --audit-level=high

# Regenerate the OpenAPI description and the TypeScript client (added with the api layer)
openapi:
    UPDATE_OPENAPI=1 cargo test -p tandem-api --test contract

# Everything CI runs, in the same order
check: lint test test-web audit

# Regenerate the offline SQL query metadata (needs the dev database, migrated)
sqlx-prepare:
    . dev/scripts/dev-env.sh migrate && cargo sqlx prepare --workspace

# Fail if the committed offline query metadata is out of date (needs the dev database, migrated)
sqlx-check:
    . dev/scripts/dev-env.sh migrate && cargo sqlx prepare --workspace --check
