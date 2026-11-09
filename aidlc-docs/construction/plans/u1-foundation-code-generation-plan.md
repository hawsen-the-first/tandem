# Code Generation Plan: U1 Foundation and walking skeleton

**This plan is the single source of truth for U1 code generation.** Part 2 executes it step by step, ticks each box immediately, and ticks each story when its functionality is implemented. No step is skipped, reordered or extended without updating this plan and asking for approval.

**Workspace root**: `~/dev/repos/tandem` (application code at the repository root, never in `aidlc-docs/`). Markdown summaries per layer go in `aidlc-docs/construction/u1-foundation/code/`.
**Project type**: greenfield, multi-unit; structure by architecture (Application Design Q7 and `unit-of-work.md` section 4), not `src/{unit}`.

## 1. Unit context

### Stories implemented (12) and skeleton
US-1.1 first-run admin setup · US-1.2 sign in · US-1.3 invite users · US-1.4 deactivate and reactivate · US-1.5 MFA · US-1.6 change password · US-1.7 forced reset · US-1.8 change role · US-1.9 sign-in duration and breach-check setting · US-2.7 audit log · US-13.3 sign-in protection · US-13.7 mobile, themes, WCAG baseline. Plus the walking-skeleton stubs (session, message, outbox, queue, event bus, mock provider), which complete no other story.

### Inputs (all approved)
`functional-design/` (entities, workflows W1 to W15, rules BR-1 to BR-13, frontend components), `nfr-requirements/` (SC, PF, AV, RL, SE, OB, MT, US requirements; tech stack T1 to T8), `nfr-design/` (patterns, LC-1 to LC-23), `infrastructure-design/` and `shared-infrastructure.md`.

### Dependencies on other units
None. U1 is the first unit. It leaves interfaces for U2 (admin configuration), U3 (full collaboration model) and U4 (full queue and orchestration).

### Entities owned
User, Invitation, PasswordReset, AuthSession, MfaCredential, RecoveryCode, SignInGuard (and the unknown-account guard), InstanceSetting, AuditEntry, and the skeleton's ChatSession, ChatMessage, OutboxEvent, Job.

### Decisions fixed for generation
- UI primitives: **Radix UI** with CSS Modules and CSS custom-property tokens (no CSS-in-JS runtime).
- Breached-password filter built from the **HIBP most-prevalent passwords (about 1 million)**; see risk R2.
- Verification: compile, lint and unit tests are run per layer inside the Nix dev shell and Docker; integration and end-to-end suites run in the Build and Test stage.
- U1 code is delivered in **five pull requests by layer** (section 4), each merged before the next starts.

### Crate and app layout for U1
`unit-of-work.md` listed the crates for the whole product. U1 adds three small crates the NFR design needs, recorded in step 57:

| Path | Purpose |
|---|---|
| `crates/core` | Domain model, state machines, pure business rules (no I/O) |
| `crates/ports` | Traits for repositories, services and infrastructure |
| `crates/app` | Application services (use cases) |
| `crates/support` | **Added**: clock and random-source traits, circuit breaker, backoff, supervised tasks |
| `crates/config` | **Added**: configuration loader |
| `crates/credentials` | **Added**: Argon2id hasher pool, breached-password checker, TOTP and recovery codes |
| `crates/store-postgres` | Migrations, repositories, queue, outbox, schema guard |
| `crates/vault` | Envelope encryption |
| `crates/providers` | Scripted mock provider (real adapters arrive in U4 and later) |
| `crates/telemetry` | Logging with redaction, tracing, metrics registry, health types |
| `apps/api`, `apps/worker`, `apps/cli` | The two services and the operations CLI |
| `web/` | Vite, React, TypeScript client |
| `tools/breach-filter` | **Added**: builds the offline breached-password filter |
| `tests/` | Cross-crate integration, contract, end-to-end and performance tests |

### Conventions
- **Rust**: `rustfmt`, `clippy` with warnings denied, `#![forbid(unsafe_code)]` in every crate that does not need it, typed errors with `thiserror`, no `unwrap` or `expect` in non-test code, every SQL statement parameterised.
- **Naming**: IDs are UUIDv7 newtypes; times come only from the `Clock` trait; tokens and codes only from the `Rng` trait (so tests control them).
- **Front end**: strict TypeScript, no `any`; all user-facing text from the message catalogue; every interactive element gets a stable `data-testid` named `{component}-{element-role}` (for example `sign-in-form-submit-button`); no dynamic IDs.
- **Tests**: every business rule BR-1 to BR-13 has at least one test named after the rule (for example `br_3_1_delay_doubles_and_caps_at_60s`); the BR-to-test table is produced in step 58.
- **Commits**: small, one logical change each, on the branch for the current PR.

## 2. Risks and how the plan handles them
| ID | Risk | Handling |
|---|---|---|
| R1 | Rust, `just` and OpenSSL are not installed here; the Nix flake must work with this machine's Nix | Step 2 creates and tests the flake first; if flakes are disabled or the network blocks, I stop and ask before changing system settings |
| R2 | The full HIBP most-prevalent dataset is very large (tens of gigabytes) and may not be practical to download here | Step 32 writes and tests the filter builder on a fixture, then tries to obtain the dataset. **If that is not practical, the plan falls back to a smaller permissively licensed common-passwords list for the committed filter, records the gap, and asks you before proceeding**; the checker's interface and online check are unchanged, so the HIBP-built filter can be swapped in later |
| R3 | Playwright browsers on NixOS need the Nix-provided browser bundle | Step 56 uses the Nix-provided browsers locally and the standard installer in CI |
| R4 | SQLx compile-time checking needs a database at build time | Step 27 commits the offline query metadata and CI verifies it is current |
| R5 | U1 is large and may span several sessions | Work stops cleanly at each PR boundary; the plan and `aidlc-state.md` record the next unticked step |

## 3. Steps

### PR 1 of 5: Scaffold, development environment and CI
- [x] **Step 1 - Workspace scaffold.** Create `Cargo.toml` (workspace, shared lints and dependency versions), `rust-toolchain.toml` (pinned stable with rustfmt and clippy), `rustfmt.toml`, `.editorconfig`, `.gitignore`, `deny.toml` (advisories, GPL-3.0-compatible licences, crates.io only), and compiling skeleton crates and apps for every path in the layout table (empty library with a smoke test; apps print their version). *Stories: none (foundation).*
- [x] **Step 2 - Nix dev shell.** Create `flake.nix` and `flake.lock` providing pinned Rust, Node, `just`, `cargo-audit`, `cargo-deny`, `sqlx-cli`, OpenSSL and Playwright dependencies; `.node-version`. Verify `nix develop` works and shows the tool versions. *(R1)*
- [x] **Step 3 - Task runner.** Create `justfile` with every recipe in infrastructure design section 5 (`dev-init`, `dev-up`, `dev-down`, `dev-reset`, `migrate`, `migrate-down`, `api`, `worker`, `web`, `test`, `test-web`, `e2e`, `lint`, `audit`, `openapi`, `perf`, `check`); recipes whose targets do not exist yet fail with a clear message.
- [x] **Step 4 - Development environment.** Create `dev/compose.yaml` (PostgreSQL 18 pinned, TLS, `hostssl` only, health check, loopback port), `dev/certgen/` (one-shot CA and server certificate generator), `dev/postgres/init/` (role creation script using generated passwords), `dev/.env.example`, and the `dev-init` script that generates `dev/.env` and `dev/secrets/master.key` with mode 0600. *Supports SE-1, SE-2, SE-7.*
- [x] **Step 5 - CI workflow.** Create `.github/workflows/ci.yml` with jobs `rust` and `web` (SHA-pinned Actions, read-only permissions, timeouts, concurrency). The `contract`, `e2e` and `perf` jobs are added in steps 46 and 56 when they have something to run.
- [x] **Step 6 - Repository hygiene and security files.** Create `.github/workflows/codeql.yml`, `.github/dependabot.yml`, `CODEOWNERS`, `SECURITY.md`, and the developer section of `README.md` (first-time setup, everyday loop, Safari limitation, repository settings checklist).
- [x] **Step 7 - Web skeleton.** Create `web/` with Vite, React, strict TypeScript, ESLint, Prettier, Vitest and one smoke test, a lock file, and a placeholder page, so the `web` job has real work.
- [x] **Step 8 - PR 1 verification and summary.** Run `just check` in the dev shell, bring up the dev database with TLS and confirm an unencrypted connection is refused and a verified TLS connection works; write `code/pr1-scaffold-summary.md`. Open PR 1.

### PR 2 of 5: Domain and application logic (no I/O)
- [x] **Step 9 - `support` crate.** `Clock` and `Rng` traits with system and fake implementations; circuit breaker (opens after 5 consecutive failures or half of the last 20; open 30 s; one trial call); exponential backoff helper; supervised task runner with restart backoff 1 s to 30 s. Unit tests with the fake clock. *Supports RL-2, RL-5.*
- [x] **Step 10 - `core`: identity types.** `UserId` and other UUIDv7 newtypes; `Email` (normalised, length, shape), `DisplayName`, `Role`, `UserStatus`, `User`, `Invitation`, `PasswordReset`, `AuthSession` with `Scope`, `MfaCredential`, `RecoveryCode`, `SignInGuard` per `domain-entities.md`. *US-1.1 to US-1.9.*
- [x] **Step 11 - `core`: pure business rules.** Password policy (BR-2.1, BR-2.2, BR-2.4), delay and lockout calculator (BR-3.1 to BR-3.3), expiry calculators (invitation 7 days, reset 24 hours, session idle and absolute), last-admin rule (BR-7.2, BR-7.3), permission matrix and scope rules (BR-9.3, BR-4.1). *US-1.1, 1.2, 1.4, 1.8, 13.3.*
- [x] **Step 12 - `core`: state machines.** Transitions and invariants for user status, invitation, auth session and job (domain-entities section 5), rejecting illegal transitions. *US-1.3, 1.4, 1.5.*
- [x] **Step 13 - `core`: audit and settings.** `AuditEvent` with the event catalogue (BR-11.5), builder that strips secrets from details (BR-11.4), outcome type; `InstanceSettingKey` definitions with defaults and ranges (BR-10) and validation (idle at most absolute). *US-1.9, US-2.7.*
- [x] **Step 14 - `core`: skeleton domain.** `ChatSession`, `ChatMessage`, `OutboxEvent`, `Job` with retry policy (5 s, 30 s, 2 min; three attempts), message length rule (BR-13.2). *Skeleton.*
- [x] **Step 15 - `core` tests.** Unit tests for every rule above, named after the BR ids; property-style table tests for the delay calculator and password policy (plain table tests, since property-based testing is disabled).
- [x] **Step 16 - `ports` crate.** Traits: `UnitOfWork` and transaction handle, repositories (`Users`, `Invitations`, `PasswordResets`, `Sessions`, `MfaCredentials`, `SignInGuards`, `Settings`, `Audit` with append and query only, `Chat`, `Outbox`, `Jobs`), `Vault`, `PasswordHasher`, `BreachChecker`, `TotpService`, `ModelProvider`. No concrete types.
- [ ] **Step 17 - `app`: authorisation and context.** `RequestContext`, `Access::authorize` implementing BR-9, typed capability results, and the "not found for non-admins" behaviour. *US-1.8, US-1.4.*
- [ ] **Step 18 - `app`: setup, invitation acceptance, sign-in.** Services for W1, W3, W4: first-run setup, accept invitation, sign-in with guard, hasher, breach check, MFA step and recovery codes, session creation, audit and failure handling. *US-1.1, 1.2, 1.3, 13.3.*
- [ ] **Step 19 - `app`: MFA, password change, forced reset.** Services for W5, W6, W7, W8 including "Also reset MFA" and session ending rules. *US-1.5, 1.6, 1.7.*
- [ ] **Step 20 - `app`: administration.** Services for W2, W9, W10, W12, W13: invite, revoke, deactivate, reactivate, change role with last-admin protection, instance settings, audit query. *US-1.3, 1.4, 1.8, 1.9, 2.7.*
- [ ] **Step 21 - `app`: skeleton.** Chat session and message services (W15), the orchestration decision step (`message.posted` enqueues `MockAgentReply`), and the mock agent job handler with idempotency. *Skeleton.*
- [ ] **Step 22 - `app` tests.** Service tests over in-memory fakes covering W1 to W15, including: audit failure fails the operation, concurrent last-admin demotion, replayed TOTP step, expired and reused links, unknown email timing path, duplicate outbox delivery.
- [ ] **Step 23 - `providers`: scripted mock provider.** Deterministic scripts behind `ModelProvider` (BR-13.5) with tests.
- [ ] **Step 24 - PR 2 verification and summary.** Compile, clippy, unit tests in the dev shell; write `code/pr2-domain-application-summary.md`. Open PR 2.

### PR 3 of 5: Persistence, vault and credentials
- [ ] **Step 25 - Migrations.** In `crates/store-postgres/migrations`: reversible up and down scripts for all U1 tables, indexes for every hot path (PF-5), the `schema_version` record, and a grants migration giving `tandem_app` data access with `INSERT` and `SELECT` only on `audit_entries`. *US-2.7, SE-7.*
- [ ] **Step 26 - Database access layer (LC-5, LC-21).** Pool factory with TLS `verify-full` through rustls, acquire and statement timeouts, separate pool settings for `api` and `worker`, transaction helper with rollback on error, advisory-lock helper, schema guard (refuse mismatch), start-up refusal without TLS. *SE-1, AV-3, RL-1.*
- [ ] **Step 27 - Repositories.** Implementations of every repository port with parameterised, compile-time-checked SQLx queries and committed offline metadata (`.sqlx/`); audit repository exposes insert and select only. *All stories; R4.*
- [ ] **Step 28 - Outbox, queue, notify, cleanup.** Outbox writer with `NOTIFY` on commit, relay primitives (advisory lock, ordered batches, mark published), job queue with `SKIP LOCKED`, lease, heartbeat, retry and failure, replay query by last event ID within 24 hours, cleanup queries (never touching audit). *Skeleton, SC-3, RL-5.*
- [ ] **Step 29 - Account guards.** `FOR UPDATE` guard repository and the bounded unknown-email guard keyed by a hash. *US-13.3, BR-3.5.*
- [ ] **Step 30 - `vault` crate (LC-14).** AES-256-GCM envelope with key ID, nonce and associated data, key file with current and decrypt-only keys, start-up canary, `rekey` function; tests include tamper detection, wrong key, rotation. *SE-2, US-1.5.*
- [ ] **Step 31 - `credentials` crate (LC-11, LC-12, LC-13).** Argon2id hasher pool (first-start benchmark, stored profile, concurrency gate, 2 s load shedding, rehash on sign-in, dummy hash); breached-password checker (offline filter loader, online k-anonymity client with 2 s timeout and the circuit breaker); TOTP (RFC 6238 test vectors, one-step tolerance, replay guard) and recovery codes. *US-1.2, 1.5, 13.3, PF-4.*
- [ ] **Step 32 - Breached-password filter (R2).** `tools/breach-filter` (builder, with a tested fixture run) and the committed filter data with attribution text; attempt to obtain the HIBP most-prevalent set; apply the fallback in R2 and stop to ask if it is not practical.
- [ ] **Step 33 - Integration tests.** In `tests/` with throw-away PostgreSQL 18 containers: migrations up and down, repositories, role privileges (the app role cannot update or delete audit rows), guard concurrency, queue leasing and expiry, outbox ordering with the advisory lock, replay window, schema guard. (Executed in Build and Test; written and compiled here.)
- [ ] **Step 34 - PR 3 verification and summary.** Compile, clippy and unit tests; write `code/pr3-persistence-credentials-summary.md`. Open PR 3.

### PR 4 of 5: API, worker and CLI
- [ ] **Step 35 - `config` crate (LC-1).** Loader for environment, optional TOML, `*_FILE` secrets, precedence, validation with one error per problem, secrets-in-file rejection, public-URL rule (https unless loopback), trusted-proxy parsing. Tests for every rule. *SE-13, assumption D15, D16.*
- [ ] **Step 36 - `telemetry` crate (LC-2, LC-3, LC-4).** JSON logging with request or job and trace IDs, the redaction layer (BR and SE-4 field list), optional OTLP export, the metrics registry with the OB-1 metrics, health checker types. Redaction tests. *SE-3, SE-4, OB-1, OB-2.*
- [ ] **Step 37 - `api` bootstrap.** Wiring of config, pools, vault, hasher pool and services; graceful shutdown; supervised background tasks; `/healthz`, `/readyz`, `/metrics` (bearer token, disabled without one). *AV-1, AV-2, SE-10.*
- [ ] **Step 38 - `api` pipeline (LC-6 to LC-9).** Middleware in the order of NFR design section 1: request ID, access log, panic catcher and error handler, security headers (exact CSP), rate limiter with trusted proxies, body limit, session resolution without cache and with `last_seen_at` throttling, CSRF and Origin checks, typed authorisation extractors, and the start-up check that every route is public or declares an extractor. *SE-5, SE-6, SE-9, SE-12.*
- [ ] **Step 39 - `api` authentication endpoints.** Setup status and setup, sign-in (with MFA and recovery code), sign-out, current user (with CSRF token), MFA enrol, confirm, disable and regenerate, password change, invitation validate and accept, reset validate and complete. *US-1.1, 1.2, 1.3, 1.5, 1.6, 1.7, 13.3.*
- [ ] **Step 40 - `api` administration endpoints.** Users list, invitations create, list and revoke, role change, deactivate, reactivate, reset link, instance settings get and put, audit log query. *US-1.3, 1.4, 1.7, 1.8, 1.9, 2.7.*
- [ ] **Step 41 - `api` skeleton and WebSocket.** Chat session and message endpoints; WebSocket gateway with cookie and Origin checks, per-user limits, ping, bounded queues, LISTEN-driven fan-out, replay from the last event ID and resync. *Skeleton, PF-3.*
- [ ] **Step 42 - `api` assets and OpenAPI.** Static asset server for `web/dist` (hashed assets, no-cache entry page, no directory listing); OpenAPI generation and the committed description. *SE-5, MT-3.*
- [ ] **Step 43 - `worker`.** Bootstrap, outbox relay under the advisory lock, job runner (`MockAgentReply` with retries), cleanup job, health endpoints, graceful shutdown. *Skeleton, AV-2, RL-5.*
- [ ] **Step 44 - `cli`.** `migrate up|down`, `vault rekey`, `admin issue-reset` for the last admin (BR-4.7), and the documented `audit prune` command that needs a privileged credential (Q2 B of NFR Requirements). *AV-4, SE-2, SE-15.*
- [ ] **Step 45 - Service-level tests.** Pipeline tests (CSRF, Origin, rate limit, headers, body limit, panic handling), misuse-case tests from NFR requirements section 5.1, start-up guard tests, metrics token tests, contract tests.
- [ ] **Step 46 - CI additions and performance harness.** Add the `contract` and `perf` jobs to `ci.yml`, the performance smoke harness in `tests/perf` for PF-1 to PF-3, and update the required-checks list in `README.md`. *PF-6, MT-3.*
- [ ] **Step 47 - PR 4 verification and summary.** Compile, clippy and unit tests; start `api` and `worker` against the dev database and check `/readyz`; write `code/pr4-api-worker-cli-summary.md`. Open PR 4.

### PR 5 of 5: Web client and end-to-end tests
- [ ] **Step 48 - Web foundations.** Router, TanStack Query, generated API client from the OpenAPI description, message catalogue, error mapping from `business-rules.md`, ESLint and Prettier rules. *US-13.7.*
- [ ] **Step 49 - Design tokens, themes and base components.** Light, dark and system themes via root attribute, CSS Modules; `FormField`, `PasswordField` (live rules), `CopyOnceSecret`, `ConfirmDialog`, `ResponsiveTable`, `Toast` and announcer region, `ErrorBoundary` on Radix primitives, with `data-testid` conventions. *US-13.7, BR-12.*
- [ ] **Step 50 - Application shell and guards.** Session context from `/auth/me`, CSRF header, `RequireAuth`, `RequireAdmin`, scope redirect to MFA enrolment, `AppShell` with header, navigation (collapses under 720 px), theme toggle and user menu. *US-13.7, BR-9.*
- [ ] **Step 51 - Public pages.** `SetupPage`, `SignInPage` with `MfaChallengeForm` and throttle countdown, `AcceptInvitationPage`, `ResetPasswordPage`, `NotFoundPage`. *US-1.1, 1.2, 1.3, 1.7, 13.3.*
- [ ] **Step 52 - MFA and account pages.** `MfaEnrolmentPage` (client-side QR, recovery codes shown once), `AccountPage` with `ChangePasswordForm` and `MfaSection`. *US-1.5, 1.6.*
- [ ] **Step 53 - Administration pages.** `UsersPage` (table, invite, role, reset link, deactivate, reactivate dialogs with last-admin protection), `SettingsPage`, `AuditLogPage`. *US-1.3, 1.4, 1.7, 1.8, 1.9, 2.7.*
- [ ] **Step 54 - Skeleton pages and WebSocket client.** `ChatSessionListPage`, `ChatSessionPage`, WebSocket client with reconnect backoff, replay from the last event ID and resync. *Skeleton.*
- [ ] **Step 55 - Component tests and accessibility.** Vitest and Testing Library tests for every page and shared component, with axe checks and keyboard interaction tests. *US-13.7, MT-5.*
- [ ] **Step 56 - End-to-end tests and CI job.** Playwright suites for first-run setup to MFA, invitation to sign-in, forced reset, role change, deactivation, throttling UI, skeleton round trip, axe checks and a keyboard walk-through, in Chromium and Firefox; add the `e2e` job to `ci.yml`. *MT-4, MT-5, R3.*
- [ ] **Step 57 - Documentation.** Update `README.md`, add `docs/development.md` and API notes, update `unit-of-work.md` with the three added crates and the breach-filter tool, and refresh `shared-infrastructure.md` for anything added.
- [ ] **Step 58 - Final verification and close-out.** Run `just check`; produce the BR-to-test table; tick every story in the traceability table; write `code/u1-code-generation-summary.md` and update `aidlc-state.md`. Open PR 5.

## 4. Pull request plan
| PR | Branch | Steps | Contents |
|---|---|---|---|
| 1 | `unit-u1/scaffold` | 1 to 8 | Scaffold, Nix shell, `just`, dev environment, CI (`rust`, `web`), hygiene files |
| 2 | `unit-u1/domain-app` | 9 to 24 | `support`, `core`, `ports`, `app`, `providers` with tests |
| 3 | `unit-u1/persistence` | 25 to 34 | Migrations, repositories, queue, vault, credentials, breach filter, integration tests |
| 4 | `unit-u1/services` | 35 to 47 | `config`, `telemetry`, `api`, `worker`, `cli`, service tests, CI `contract` and `perf` |
| 5 | `unit-u1/web` | 48 to 58 | Web client, end-to-end tests, CI `e2e`, documentation, close-out |

Each PR is based on `main` after the previous PR is merged. The aidlc documents for each layer are committed with that PR. The design-stage PRs (#4 and this plan) merge first.

## 5. Story traceability (stories ticked when their steps are done)
| Done | Story | Steps |
|---|---|---|
| [ ] | US-1.1 First-run admin setup | 10, 11, 18, 22, 39, 51, 56 |
| [ ] | US-1.2 Sign in | 11, 18, 27, 31, 38, 39, 51, 56 |
| [ ] | US-1.3 Invite users | 12, 18, 20, 39, 40, 51, 53, 56 |
| [ ] | US-1.4 Deactivate and reactivate | 11, 12, 17, 20, 40, 53, 56 |
| [ ] | US-1.5 Multi-factor authentication | 19, 30, 31, 39, 52, 56 |
| [ ] | US-1.6 Change my password | 19, 39, 52, 56 |
| [ ] | US-1.7 Force a password reset | 19, 39, 40, 44, 51, 53, 56 |
| [ ] | US-1.8 Change a user's role | 11, 17, 20, 40, 53, 56 |
| [ ] | US-1.9 Set sign-in duration and breach check | 13, 20, 40, 53 |
| [ ] | US-2.7 View the security audit log | 13, 20, 25, 27, 40, 53 |
| [ ] | US-13.3 Lockout after repeated failures | 11, 18, 29, 38, 51 |
| [ ] | US-13.7 Mobile, themes, WCAG baseline | 48, 49, 50, 55, 56 |
| n/a | Skeleton (no story completion) | 14, 21, 28, 41, 43, 54, 56 |

## 6. Extension compliance (planning)
- **Security Baseline**: each security-relevant step names its SE or SECURITY rule; step 38 (pipeline), 30 (vault), 31 (credentials), 25 and 26 (database roles and TLS), 35 (configuration), 36 (redaction) carry the main controls. Compliance is re-evaluated at the completion message.
- **Resiliency Baseline**: steps 9, 26, 28, 37 and 43 carry timeouts, breaker, outbox, queue and graceful shutdown; RESILIENCY-04 and -12 remain for U8.
- **Property-Based Testing**: disabled; table-driven tests are used instead.
