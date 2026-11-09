# U1 Code Generation: PR 3 summary (persistence, vault, credentials)

Plan steps 25 to 34 of `construction/plans/u1-foundation-code-generation-plan.md`. Branch `unit-u1/persistence`.

## What was created
| Crate or tool | Contents | Tests |
|---|---|---|
| `store-postgres` | Reversible sqlx migrations 0001 to 0004 (identity, audit, skeleton, grants); `PgStore` and `PgTx` implementing every port; mandatory verify-full TLS pool; schema-version guard; transactional outbox with `pg_notify` on commit and an advisory-lock relay lease; `FOR UPDATE SKIP LOCKED` job queue with lapsed-lease recovery; sign-in guards locked `FOR UPDATE` with a bounded unknown-email table; retention cleanup that never touches the audit log. Compile-time checked queries with committed offline metadata in `.sqlx/` | 9 unit, 19 integration, 1 end-to-end |
| `vault` | AES-256-GCM envelope vault: key file with one current and any number of decrypt-only keys, purpose and row bound AAD, canary check, `rekey` | 14 |
| `credentials` | Argon2id hasher (profile ladder, benchmark, concurrency gate with a busy answer, rehash when parameters are outdated, dummy verify for unknown accounts); Bloom-filter breach checker with the embedded SecLists-derived filter plus the optional online k-anonymity check behind a circuit breaker; TOTP (RFC 6238 vectors) | 31 |
| `tools/breach-filter` | Builder library and CLI for the filter file | 6 |

## Integration tests (step 33)
Run against a throw-away PostgreSQL 18.6 started with the `docker` CLI, TLS required, with generated CA and server certificates, the same two roles as the dev environment, and one database cloned per test from a migrated template.

- Migrations: up, down and up again; the schema guard refuses older and newer schemas and the app role fails closed.
- Privileges: the app role cannot update or delete audit rows, and cannot drop or alter tables.
- Repositories: every aggregate, unique-email conflict, one agent reply per message, insertion-order tie-break.
- Concurrency: sign-in guards, queue leasing, lapsed-lease recovery, outbox ordering and the relay lease.
- Cleanup: retention windows respected, audit log untouched.
- `tests/app_flow.rs`: the real `App` over `PgStore` with the real Argon2id hasher, vault and TOTP: setup, MFA enrolment, sign-in with a second factor, invitation and acceptance, chat message, relay, job, agent reply, plus a check that the audit log cannot be rewritten.

## Verification performed
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (all pass; the integration tests need Docker), `cargo deny check` (advisories, bans, licences and sources all ok), `cargo machete`, and `just sqlx-check` against the migrated dev database.

## Design notes and deviations
| Item | Detail |
|---|---|
| Docker CLI harness instead of testcontainers | `testcontainers` pulls in `astral-tokio-tar`, which RUSTSEC-2026-0440 marks unmaintained (no fix). By your choice it was dropped rather than the advisory ignored. The harness runs `docker run --rm` and a small watchdog removes the container when the test process exits |
| Licence allow-list | `CDLA-Permissive-2.0` added to `deny.toml`: it is the licence of `webpki-roots` (Mozilla CA bundle data), pulled in by sqlx's rustls support. It is a permissive licence |
| Job retention uses `run_at` | `jobs.created_at` is the database clock, not the application's. Cleanup judges finished jobs by `run_at`, which the application controls and tests can fix |
| Message ordering | `chat_messages.seq` identity column breaks ties between equal timestamps, as the PR 2 port note required |
| HIBP corpus | The SecLists fallback approved at step 32 is used; the online check covers full HIBP when enabled |
| Offline query metadata | `.sqlx/` is committed so CI builds without a database; regenerate with `just sqlx-prepare`. CI does not check it is current (that needs a database); `just sqlx-check` does, locally |

## Merge order
Based on `main` after PR #7 merged; no dependencies on other open PRs.
