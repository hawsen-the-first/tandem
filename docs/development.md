# Development guide

How to build, run, test and extend Tandem. The README has the short version; this is the reference.

## The stack at a glance

| Piece | What it is | Where |
|---|---|---|
| `api` | HTTP and WebSocket server (Axum). Serves the built web client when `TANDEM_WEB_DIR` is set | `apps/api` |
| `worker` | Outbox relay, job runner, cleanup, health and metrics endpoint | `apps/worker` |
| `tandem` CLI | `migrate`, `vault rekey`, `admin issue-reset`, `audit prune` | `apps/cli` |
| `web` | Vite, React 19, React Router, TanStack Query, CSS Modules. No UI kit | `web/` |
| PostgreSQL 18 | Required TLS, two roles (`tandem_migrate`, `tandem_app`) | `dev/compose.yaml` |

Crates (`crates/`): `core` (domain, no I/O), `ports` (traits), `app` (services), `store-postgres`, `vault`,
`credentials` (Argon2id, breach filter, TOTP), `providers` (model adapters), `config`, `telemetry`, `bootstrap`
(shared start-up), `outbound` (the one HTTP path for admin-directed calls), `search`, `github`, `sandbox` (runtime probe), `support` (clock, rng, breaker, supervisor). Tool: `tools/breach-filter`.

Dependencies point inwards: `core` knows nothing; `app` talks to `ports`; adapters implement `ports`; the apps
wire everything. Application tests use in-memory fakes (`tandem-app` feature `testing`), so most logic is
tested without a database.

## Everyday commands

```sh
just dev-init && just dev-up && just migrate   # once
just api            # https is not needed locally; the web client proxies /api to it
just worker
just web            # http://localhost:5173
just check          # everything CI runs
just e2e            # Playwright, Chromium and Firefox, isolated stack (needs Docker)
just perf           # latency budgets (PF-1 to PF-3), needs Docker
just contract       # routes match the OpenAPI description
just openapi        # regenerate apps/api/openapi.json (then: cd web && npm run generate:api)
just sqlx-prepare   # after changing a SQL query! commits .sqlx/ offline metadata
```

## Configuration

Environment variables with the prefix `TANDEM_`, then an optional TOML file named by `TANDEM_CONFIG`; the
environment wins. Secrets come from the environment or `*_FILE` variables and are rejected in the TOML file.
The loader reports every problem at once and never prints a secret.

| Setting | Notes |
|---|---|
| `TANDEM_PUBLIC_URL` | Required. Origin only. `http://` is accepted only for localhost and loopback addresses |
| `TANDEM_DATABASE_URL` (or `_FILE`) | Required. Must carry `sslmode=verify-full` (or `verify-ca`) |
| `TANDEM_DATABASE_CA_FILE` | Required. The CA that signed the database certificate |
| `TANDEM_MASTER_KEY_FILE` (or `TANDEM_MASTER_KEY`) | Required. Vault key file: `<id> <current\|decrypt-only> <base64 of 32 bytes>` per line |
| `TANDEM_LISTEN_ADDR`, `TANDEM_WORKER_LISTEN_ADDR` | Defaults `127.0.0.1:8080` and `127.0.0.1:8081` |
| `TANDEM_TRUSTED_PROXIES` | Comma-separated IPs and CIDRs whose `X-Forwarded-For` is believed. Empty: forwarded headers are ignored |
| `TANDEM_METRICS_TOKEN` | Bearer token for `/metrics`. Without it `/metrics` answers 404 |
| `TANDEM_WEB_DIR` | The built client (`web/dist`). Optional |
| `TANDEM_SOURCE_URL` | Where the Corresponding Source of this running version can be obtained (AGPL section 13). Defaults to the upstream project. **If you run a modified version for users over a network, set it to where your modified source is published.** The footer links to it |
| `TANDEM_OTLP_ENDPOINT` | Validated, but this build has no OTLP exporter yet (trace IDs are logged only) |
| `TANDEM_LOG_LEVEL`, `TANDEM_HASH_MEMORY_BUDGET_MB`, `TANDEM_DB_*` | Optional tuning, see `crates/config` |

## The request pipeline

Outermost first: request identity and span, access log and metrics, panic catcher, security headers,
rate limit, body limit, session resolution (from the database on every request, nothing cached), CSRF and
Origin checks, then the route guard and the handler. Routes are registered only through `RouteTable`, which
records who may call each one and wraps it in the matching guard, so a route that is neither public nor guarded
cannot exist. A test pins the list of public routes: adding one is a deliberate edit.

To add an endpoint: write the handler with `#[utoipa::path]`, register it in `RouteTable` with its access class
(`AnyUser`, `FullScopeUser`, `AdminUser` or `public`) and rate-limit bucket, add it to `ApiDoc` in
`routes/openapi.rs`, run `just openapi`, then `cd web && npm run generate:api`. The contract test fails until the
description matches the routes.

## Testing conventions

- Rust unit tests live in `tests.rs` files or `tests/` directories; so do tests with password- or key-like
  literals. CodeQL ignores those paths (`.github/codeql/codeql-config.yml`) because it reports test passwords as
  hard-coded credentials, while production code stays fully scanned.
- Database tests start a throw-away PostgreSQL 18 with TLS through the `docker` CLI, migrate a template once and
  clone one database per test. The container is removed when the test process ends.
- Web component tests: Vitest, Testing Library and `vitest-axe`. `src/test/mockApi.ts` replaces `fetch`;
  `src/test/render.tsx` renders the real route table. Every page has an accessibility check and a keyboard test.
- `data-testid` names are kebab-case: `<page>-<part>`, for example `sign-in-submit`.
- End-to-end tests (`web/e2e`) run in Chromium and Firefox against a real stack. The `setup` project performs
  first-run setup once and saves an admin session for the others.

## Web client notes

- **Strict CSP** (`style-src 'self'`, no inline styles or scripts). Components use CSS Modules and design tokens
  in `src/styles/tokens.css`; never add a `<style>` tag or a `style` attribute. For that reason there is no Radix
  or React Aria: their scroll locking and positioning inject styles at run time. Dialogs use the native `<dialog>`
  element (focus trap, Escape, inert background, focus return); menus use `<details>`.
- Themes: light, dark and system through the `data-theme` root attribute; both meet the contrast targets.
- The generated client types are committed (`src/api/schema.d.ts`); CI fails if they are stale (`npm run check:api`).
- Secrets shown once (invitation and reset links, recovery codes) live only in component state and are cleared
  on dismissal. Link tokens are stripped from the address bar as soon as the page has read them.
- Browsers for the end-to-end tests come from the Nix shell (`playwright-driver.browsers`). The npm
  `@playwright/test` version is pinned to the same version as that package in the flake lock (currently 1.64.1); bump both together.

## Releases and operations

Not part of unit U1. Image building, the reference Compose file, signing and the self-hosting guide arrive in U8.
