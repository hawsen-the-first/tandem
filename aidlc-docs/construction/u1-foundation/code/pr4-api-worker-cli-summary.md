# U1 Code Generation: PR 4 summary (config, telemetry, runtime, api, worker, cli)

Plan steps 35 to 47 of `construction/plans/u1-foundation-code-generation-plan.md`. Branch `unit-u1/api-worker-cli`.

## What was created
| Crate or app | Contents | Tests |
|---|---|---|
| `config` | Environment, optional TOML file (`TANDEM_CONFIG`), `*_FILE` secrets, environment-wins precedence, one message per problem, secrets in the file rejected, `https` required unless loopback, `sslmode=verify-*` required on the database URL, trusted proxy CIDRs, optional web directory and worker listen address | 26 |
| `telemetry` | JSON log layer with correlation IDs and a redaction layer (SE-4), W3C trace context, metrics registry with every OB-1 metric (plus `hash_*`, `outbox_*`, `job_*`, `ws_connections`, process memory and CPU), readiness aggregation with per-probe timeout and shutdown flag | 22 |
| `runtime` (new, see below) | Shared start-up for api, worker and cli: pool, vault, schema guard, vault canary guard, hashing profile benchmark and storage, hasher and breach checker wiring, readiness probes, shutdown | 3 |
| `api` | Request pipeline in the order of NFR design section 1, a route table that makes "public or guarded" structural, authentication, MFA, invitation and reset endpoints, administration endpoints, chat endpoints, the event WebSocket, the asset server, `/healthz`, `/readyz`, `/metrics`, generated OpenAPI with a committed copy | 42 (+1 perf) |
| `worker` | Outbox relay under the advisory lock (`NOTIFY` wake-up plus 1 s poll), job runner with retries, lapsed-lease recovery, daily cleanup, queue gauges, health and metrics server, graceful shutdown | 8 |
| `cli` (`tandem`) | `migrate up` and `down --yes`, `vault rekey`, `admin issue-reset` (last-admin recovery, BR-4.7), `audit prune` with a privileged credential and a 90-day floor (SE-15) | 9 |

Application and store additions: `App::issue_admin_reset_for_cli`, `EventPage::scanned_up_to`, `PgStore` internal settings, queue statistics, `rekey_sealed_values` and `prune_audit_before` (2 new application tests).

## API surface
`/api/setup`, `/api/auth/{sign-in,sign-out,me,mfa/{enrol,confirm,recovery-codes,disable},password}`, `/api/invitations/{validate,accept}`, `/api/password-resets/{validate,complete}`, `/api/admin/{users,invitations,settings,audit}`, `/api/chat/sessions[/{id}/messages]`, `/api/events` (WebSocket), `/api/openapi.json`. Tokens travel in request bodies, never in URLs. The generated description is `apps/api/openapi.json`.

The links the API returns point at client routes the web client (PR 5) must provide: `/accept-invitation?token=` and `/reset-password?token=`.

## Verification performed
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (all pass; the Postgres-backed tests need Docker), `cargo deny check`, `cargo audit`, `cargo machete`, `just sqlx-check`.
- Performance smoke test, release mode, real PostgreSQL and WebSocket (PF-1 to PF-3): sign-in p95 183 ms (budget 1 s), typical reads p95 3 ms (200 ms), message-to-reply round trip p95 46 ms (500 ms).
- The api and worker were started against the dev database: `/readyz` is 200 with the database, schema and vault probes (plus relay lag for the worker); SIGTERM exits 0.
- Misuse cases of NFR requirements 5.1 each have an HTTP-level test: probing for emails, lockout, TOTP replay, reused invitation and reset links, CSRF, role tampering and stale sessions after demotion, last-admin removal, cross-user chat access. The audit-trail case is in the PR 3 integration tests.

## Design notes and deviations
| Item | Detail |
|---|---|
| New `runtime` crate | Not in the plan. The api, worker and cli all need the same start-up guards; a shared crate avoids three copies |
| OTLP export not implemented | `TANDEM_OTLP_ENDPOINT` is accepted and validated, trace IDs appear in every log line and travel in outbox rows and jobs, but no exporter is compiled in: the OpenTelemetry stack adds a large dependency tree. Start-up logs a warning when the endpoint is set. **Needs a decision**: add the exporter in a follow-up PR |
| HTTP header read timeout (10 s) not enforced | `axum::serve` does not expose it. The 30 s per-request timeout is enforced. Slow-header attacks are best handled by the reverse proxy; a manual accept loop can add it later |
| Statement timeout | The pool uses `TANDEM_DB_STATEMENT_TIMEOUT_SECS` (default 30) for api and worker alike; the design said 5 s for requests. Operators can set it lower |
| WebSocket outbound queue | A client that cannot take a frame within 5 s is disconnected and must resync, instead of a separate bounded queue per connection |
| Performance test location | `apps/api/tests/perf.rs` (reusing the Postgres harness) rather than a root `tests/perf` crate; run by the new `perf` CI job in release mode |
| Test harness copies | The Docker harness is copied into `api`, `worker` and `cli` test directories. Extracting a shared test-support crate is a possible follow-up |
| Internal settings | The vault canary and hashing profile live in `instance_settings` under `internal.*` keys with no `updated_by`; they are not part of the settings API |
| Audit prune is not itself audited | The audit catalogue has no prune event (a PR 2 test pins it at 20 types). The CLI prints what it deleted |
| CLI reset issuer | The reset row names the admin as its own issuer because no actor is signed in; the audit entry is labelled "operations CLI" |
| CI | `contract` and `perf` jobs added; `e2e` arrives with the web client. The README's required-checks list is updated |

## Merge order
Based on `main` after PR #8 merged; no dependencies on other open PRs. If you protect `main` with required checks, add `contract` and `perf` once this PR has run them.
