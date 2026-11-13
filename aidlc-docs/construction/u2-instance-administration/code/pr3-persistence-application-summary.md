# U2 PR 3: persistence, application, worker, CLI

Steps 17 to 27 of the code generation plan. Administration now works end to end below the API:
the services, the storage, the test runner in the worker, and the vault re-key. No endpoint or
screen exists yet (PR 4 and 5).

## What was added

| Area | Change |
|------|--------|
| Migration 0005 (reversible) | `providers`, `provider_models`, `model_defaults`, `search_configs`, `search_active`, `connectors`, `sandbox_policies` (with `version`), `config_test_runs`. Sealed `bytea` columns with check constraints that tie "a key is saved" to "a sealed value exists"; a unique index on lower-cased provider names; a **partial unique index allowing one queued or running run per target**; defaults cleared by `ON DELETE SET NULL`. Grants for the app role are in the migration; the audit table is untouched |
| `ports` | `ProviderRepo`, `DefaultsRepo`, `SearchRepo`, `ConnectorRepo`, `PolicyRepo`, `TestRunRepo` and `Tx` accessors; `Locks` and `LockGuard`. Repositories hold **sealed bytes only**; the application seals and opens, bound to purpose and row |
| `store-postgres` | The six repositories; `PgLocks` (session advisory lock on a dedicated connection, a dropped guard closes the connection so a crashed worker cannot keep a lock); test run results as JSON; cleanup prunes old runs (latest finished run per target always kept, otherwise the newest 10 within 30 days); `vault rekey` covers provider keys, search keys and connector tokens in the same transaction; `.sqlx` regenerated |
| `app` administration service | Providers and models (W16, W18), defaults (W19), search settings and activation (W20), the GitHub token (W21), the sandbox policy (W22), starting and reading tests. Every change audits in its own transaction with before and after values; secrets appear as `set` or `not set` |
| `app` test runner | The `config_test` job: takes the target's advisory lock, marks the run running, runs the probe, stores each result as it lands (and the detected capabilities, or the connector's account and expiry), finishes, then audits `config.test.run` (a failed audit write is logged; the result stands). Lock contention schedules a delayed job instead of burning an attempt. A crashed attempt restarts from nothing; the last failed attempt fails the run. A sweep fails runs still queued after 2 minutes |
| `app` read port | `config_list_models`, `config_defaults`, `config_resolve_model`, `config_active_search`, `config_github_credentials`, `config_effective_policy`. A 5 second per-process snapshot cache cleared by local writes; secrets stay sealed in the snapshot and are opened on demand into connection values that cannot print |
| `worker` | The recovery sweep runs with the existing lapsed-lease recovery; cleanup prunes runs. Real adapters are wired in `bootstrap` |
| `cli` | `vault rekey` reports the new counts |

## Behaviour worth knowing

- A test run is a job, so a long provider test (up to 60 s) occupies that worker's job loop. Run more than one worker if chat replies must not wait behind tests.
- The runner reads the database directly, not the cache, so a test always uses what was just saved.
- A provider's models are replaced by id: renaming keeps the id (so defaults keep pointing at it), detected capabilities survive an edit, declared ones are replaced by what the admin sent.
- Saving the container tier probes the runtime on every save and never trusts a stored result.
- A policy save compares versions in the database; a concurrent save is refused with "Someone else changed the policy".
- `AppError::AlreadyRunning` is mapped to 409 with a fixed message for now; PR 4 adds the run id to the body.

## Verification

fmt, clippy `-D warnings` (online and offline), `cargo test --workspace` (including the Docker
integration suites), deny, machete, audit and the outbound lint check pass. New tests: app 40
(19 administration, 16 test start and runner, 5 read port), Postgres integration 12
(repositories, unique active run, advisory lock between two workers, sealed round trip, grants,
two workers and one run, lock contention, defaults cleared in one transaction), worker 2 (a run
completes through the real loops; the sweep), CLI rekey extended.
