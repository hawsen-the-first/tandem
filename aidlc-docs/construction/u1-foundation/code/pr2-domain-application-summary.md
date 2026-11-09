# U1 Code Generation: PR 2 summary (domain and application logic, no I/O)

Plan steps 9 to 24 of `construction/plans/u1-foundation-code-generation-plan.md` (step 23, the mock provider, was written before the application tests that use it). Branch `unit-u1/domain-app`.

## What was created
| Crate | Contents | Tests |
|---|---|---|
| `support` | `Clock` and `Rng` traits with system and fake implementations; circuit breaker (opens after 5 consecutive failures or half of 20; 30 s open; one trial call); exponential backoff; supervised task runner (restart with backoff, panics included) | 17 |
| `core` | Typed UUIDv7 IDs; `Email`, `DisplayName`, `Role`, `User` and its state transitions; password policy (BR-2); sign-in guard with progressive delay and lockout (BR-3); expiry rules; permission matrix and scopes (BR-9, BR-4.1); invitation, reset, session, MFA and recovery-code state machines; token and recovery-code generation and hashing; audit event catalogue with secret-stripping (BR-11); security settings and ranges (BR-10); skeleton chat, outbox and job domain with the retry policy (BR-13.3) | 75 |
| `ports` | `Store` and `Tx` (transaction boundary), repository traits for every aggregate, `PasswordHasher`, `BreachChecker`, `TotpService`, `Vault`, `ModelProvider`. The audit repository has no update or delete method | 1 |
| `app` | `App` with every workflow W1 to W15: first-run setup, sign-in (guard, hasher, breach, MFA, recovery codes, rehash), session resolution, sign-out, MFA, password change, forced reset, invitations, deactivate/reactivate, role changes, settings, audit query, skeleton chat, outbox relay decision step, job runner, event feed. Plus in-memory fakes (`testing` feature) | 86 |
| `providers` | Scripted, deterministic `MockProvider` | 6 |

Every business rule BR-1 to BR-13 that this layer can express has at least one test named after its rule ID (the full BR-to-test table is produced in step 58).

## Verification performed
- `just check` passes (exit 0): rustfmt, clippy `-D warnings` over the workspace, all tests (184 across the four crates plus smoke tests), `cargo audit`, `cargo deny`, `cargo machete`, and the web checks.
- Failure paths are tested: audit write failure rolls back the change (BR-11.1); two simultaneous setups create one admin; two admins demoting each other at once leave one; a busy hasher answers "busy" without counting a failure; a crashed worker's lapsed lease is recovered; duplicate event delivery never produces a second reply.

## Design notes and deviations
| Item | Detail |
|---|---|
| Stateless two-step MFA sign-in | A correct password without a second factor returns `MfaRequired` and changes nothing; the client resubmits email, password and code together. A wrong code counts as a failure (BR-4.8) |
| Ports added during generation | `SessionRepo::get`, `OutboxRepo::latest_id`, `Tx::lock_setup` and `Tx::lock_admin_changes` (serialise first-run setup and admin-count-sensitive changes) |
| `ChatMessage.in_reply_to` | Added so the "one reply per message" rule is a uniqueness constraint (BR-13.4). `domain-entities.md` updated |
| Message ordering | Messages with equal timestamps must keep insertion order so a reply never sorts before its parent. The in-memory store does; the PostgreSQL adapter (PR 3) must add a sequence column for the tie-break. Documented on the port |
| CSRF token | Derived from the session token (not stored), to be implemented in the api layer |
| `core` depends on `support` | For the clock and random-source traits used to generate IDs; nothing in `core` performs I/O |
| `cargo deny` caught missing versions | Path dependencies need a version to satisfy the "no wildcards" rule; fixed |
| Error mapping | `AppError` carries user-safe validation and generic variants; `Internal` messages are for logs only. The api layer maps them to status codes |

## Merge order
Based on `main` after PRs #4, #5 and #6 merged; no dependencies on other open PRs.

## CodeQL finding on this PR and how it was handled
CodeQL reported 33 `rust/hard-coded-cryptographic-value` alerts, all password literals in test code and the test fakes (none in production code). Rather than dismissing alerts on GitHub, the PR adds `.github/codeql/codeql-config.yml`, which ignores test-only paths (`**/tests/**`, `**/tests.rs`, `**/*_tests.rs`, `**/testing.rs`), so production code stays fully scanned. The inline password-policy tests were moved to `crates/core/src/password_policy/tests.rs` to fit the convention, now recorded in the README and the plan. Review point: this narrows what CodeQL analyses to non-test code.
