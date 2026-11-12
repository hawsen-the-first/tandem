# Code Generation Plan: U2 Instance administration

**This plan is the single source of truth for U2 code generation.** Part 2 executes it step by step, ticks each box immediately, and ticks each story when its functionality is implemented. No step is skipped, reordered or extended without updating this plan and saying so in the pull request.

**Workspace root**: `~/dev/repos/tandem` (application code at the repository root, never in `aidlc-docs/`). Markdown summaries per pull request go in `aidlc-docs/construction/u2-instance-administration/code/`.
**Project type**: greenfield, multi-unit; structure by architecture (`unit-of-work.md` section 4), not `src/{unit}`.

## 1. Unit context

### Stories implemented (6)
US-2.1 provider configuration · US-2.3 test a provider · US-2.4 default models · US-2.5 web search · US-2.6 GitHub connector (fine-grained token) · US-2.9 sandbox policy.
Stories created by this unit's design and **not** built here: US-2.12 (allow and block lists, U9) and US-2.13 (GitHub App, U5).

### Inputs (all approved)
`functional-design/` (entities, workflows W16 to W24, rules BR-14 to BR-21, frontend components), `nfr-requirements/` (SC-5 to US-5, T5 to T15), `nfr-design/` (ten patterns, LC-24 to LC-34, D1 to D7). U1's artifacts and code stand.

### Dependencies on other units
U1 only: identity and admin authorisation, the vault, the audit writer, the job queue and worker, the API pipeline, the web shell and shared components. U2 leaves one interface for later units: the **config read port** (LC-31) used by U4 (models, defaults, search, policy) and U5 (GitHub credentials).

### Entities owned
Provider, ProviderModel, ModelDefaults, SearchConfig and the active search kind, Connector (GitHub), SandboxPolicy (scope `instance`), ConfigTestRun.

### Decisions fixed for generation
- Five pull requests by layer (section 4), each merged before the next starts.
- New crates: `outbound` (LC-25), `search` (LC-27), `github` (LC-28), `sandbox` (the runtime probe, LC-29; the runners arrive in U7). The OpenAI-compatible adapter goes in the existing `providers` crate. Pure rules live in `core` (LC-24, LC-33).
- All server-side calls on an admin's behalf go through `outbound`. The existing breach checker in `credentials` calls a fixed public host (api.pwnedpasswords.com) with no admin input, so it is **exempt** from the "no other HTTP client" lint, with the reason written next to the exemption.
- Tests that call out run against **in-process loopback mock servers**, never the internet; CI has no secrets for third-party services.
- End-to-end tests cover providers, search and the policy against mock servers started by the e2e stack; the **GitHub test action is covered by adapter tests only** because its API base is fixed to `https://api.github.com` and must not be reached from CI. The GitHub screen's save, saved-state and remove flows are covered end to end.
- Verification per pull request: format, clippy `-D warnings`, tests, `cargo deny`, `cargo audit`, `cargo machete`, `just sqlx-check`; web lint, format, types, tests, build and `check:api`; the e2e suite from pull request 5 (and a smoke run of the existing suite after pull request 4).

## 2. Risks
| ID | Risk | Mitigation |
|---|---|---|
| R1 | Literal-IP hosts never reach the resolver, so a name-only check would miss `http://169.254.169.254/` | The URL rules parse every spelling of an address (decimal, octal, hex, IPv4-mapped) at save time and the client checks the literal again before connecting (steps 3, 6) |
| R2 | Holding an advisory lock needs one connection for acquire and release; a pooled connection can be returned between calls | The runner holds a dedicated connection for the whole run and releases the lock on that connection (step 22) |
| R3 | A custom resolver must still allow TLS server-name verification | The resolver returns only addresses; the host name stays in the request so SNI and certificate checks are unchanged (step 6) |
| R4 | Unix-socket mock for the Docker probe is not portable to Windows | The probe and its tests are `cfg(unix)`; the container tier is simply unavailable elsewhere (step 15) |
| R5 | The audit event catalogue is pinned by a test at 20 types | The test is updated to the new count in the same step that adds the types (step 5) |
| R6 | The U1 redaction rule change could redact harmless fields (`key_id`) | An allow-list and tests for both sides (step 8) |
| R7 | Large web surface in pull request 5 | Pages are built in parallel by separate agents under review, as in U1, over shared components written first (steps 33 to 37) |
| R8 | e2e mock servers add moving parts | One small Node mock module, started and stopped by `dev/scripts/e2e.sh` (step 39) |

## 3. Code layout added in U2
```text
crates/core/src/      config.rs (entities, suitability), outbound_url.rs, sandbox_policy.rs; audit, job, ids, permissions extended
crates/outbound/      LC-25 client, resolver, redirect policy, errors
crates/providers/     openai_compatible.rs (alongside the mock)
crates/search/        searxng.rs, brave.rs, tavily.rs
crates/github/        connector.rs (token test, repositories, expiry)
crates/sandbox/       docker_probe.rs (Unix socket, read-only version call)
crates/ports/src/     probe ports, connection value types, config repository traits
crates/store-postgres/ migrations/0005_instance_config.*, config repositories, test runs, advisory lock
crates/app/src/       integrations.rs (LC-34), config_reader.rs (LC-31), config_test.rs (LC-32); testing.rs extended
apps/api/src/routes/  integrations.rs (+ DTOs), test_runs.rs
apps/worker, apps/cli extended
web/src/pages/admin/integrations/  layout, ProvidersPage, SearchPage, GitHubPage, SandboxPolicyPage
web/src/components/   SecretField, StatusBadge, TestResultView, UrlField, HostListEditor, CapabilityWarning
```

## 4. Pull requests
| PR | Steps | Contents |
|---|---|---|
| 1 | 1 to 10 | Domain types and rules, the outbound client, the container-socket setting, the redaction fix, lint enforcement |
| 2 | 11 to 16 | Probe ports and the provider, search, GitHub and runtime adapters, with mock-server tests |
| 3 | 17 to 27 | Migration, repositories, the application services, the read port, the test runner, worker and CLI changes |
| 4 | 28 to 32 | API routes, rate-limit bucket, OpenAPI and web types, API and lifecycle tests |
| 5 | 33 to 41 | Web screens, components, accessibility and end-to-end tests, documentation, close-out |

## 5. Steps

### Pull request 1: domain, outbound client, settings, lint
- [ ] **Step 1 - Workspace.** Add crates `outbound`, `search`, `github`, `sandbox` (empty libraries with the shared lints and manifests) to the workspace; register in `Cargo.toml`, README layout and `docs/development.md` crate list.
- [ ] **Step 2 - `core`: configuration entities.** `ProviderId`, `ProviderModelId`, `TestRunId`; `ProviderKind`, `Provider`, `ProviderModel`, `Capability` (tool calling and context tokens, each with source `detected` or `declared`), `ModelDefaults`, `SearchKind`, `ConnectorKind`, `TestStatus`, `ProbeResult`, `TestRun` and its state machine; the suitability rule (tool calling supported and at least 8,000 tokens); validation of names and model names (BR-14.2, BR-14.3, BR-15). Table-driven tests. *US-2.1, 2.3, 2.4.*
- [ ] **Step 3 - `core`: outbound URL rules (LC-24).** `OutboundUrl::parse`, address classification, always-blocked ranges including IPv4-mapped IPv6 and odd spellings, host classes, refusals with reasons (BR-19.1 to BR-19.3, BR-19.7). Exhaustive tables. *All U2 stories; SE-19, MT-10.*
- [ ] **Step 4 - `core`: sandbox policy rules (LC-33).** Tool registry (A1, `command_run` never allowed), factory defaults and ranges (A2) in one constants module, `SandboxPolicy` with `scope` and `version`, validation (BR-18.3 to BR-18.6), the effective-policy clamp (BR-18.8). Tests. *US-2.9.*
- [ ] **Step 5 - `core`: audit, jobs, permissions.** Eight new audit event types (A8) and the catalogue test updated (20 to 28); `JobKind::ConfigTest { run_id }` with its name; a permission action for managing integrations (admin, full scope). *All U2 stories.*
- [ ] **Step 6 - `outbound` crate (LC-25).** Own async resolver with all-address checks, connect to a checked address only, literal-IP check, manual redirect policy (same host, at most 3), 5 s connect and 15 s total timeouts, 1 MiB cap, TLS verification, no proxies, no cookies; `OutboundError`; metrics hooks. Tests with loopback mock servers, a rebinding resolver double, redirects, oversize and slow answers. *SE-18, SE-19, RL-7.*
- [ ] **Step 7 - `config` crate.** Optional `TANDEM_CONTAINER_SOCKET` (absolute path to a Unix socket, validated; absent means not configured); tests. *US-2.9; SE-23.*
- [ ] **Step 8 - `telemetry`: redaction and metrics.** The word `key` joins the sensitive names with an allow-list (`key_id`, `key_saved_at`, and similar); metric names and helpers `outbound_requests_total`, `outbound_request_duration_seconds`, `config_test_total`; redaction tests both ways. *SE-21, OB-5.*
- [ ] **Step 9 - Lint enforcement.** `clippy.toml` `disallowed-methods` for HTTP client construction outside `outbound`, with the documented `credentials` exemption; a CI check that proves a violation fails. *SE-18.*
- [ ] **Step 10 - Pull request 1 verification and summary.** Run all gates; write `code/pr1-foundations-summary.md`. Open pull request 1.

### Pull request 2: adapters
- [ ] **Step 11 - `ports`: probe ports and connection types.** `ProviderProbe`, `SearchProbe`, `ConnectorProbe`, `RuntimeProbe`; connection value types whose secrets use the redacting `Secret` (no `Debug`, `Display` or serialisation); result sinks for per-probe results. *D6.*
- [ ] **Step 12 - `providers`: OpenAI-compatible adapter (LC-26).** List models, chat probe, forced-`echo` tool probe, context-length extraction (A5, A6), fixed statuses and messages, no remote text relayed; concurrency of 4 within the 60 s budget. Tests against mock servers: with and without tool support, with and without a listing, unauthorised, unknown model, malformed, slow, oversized. *US-2.1, 2.3; SE-22, PF-8.*
- [ ] **Step 13 - `search` crate (LC-27).** SearXNG, Brave and Tavily test calls; fixed endpoints for Brave and Tavily (overridable only in tests); statuses per BR-16.4; mock-server tests. *US-2.5.*
- [ ] **Step 14 - `github` crate (LC-28).** The connector port and the token test over the outbound client: account login, token expiry header, up to 100 repositories with total; statuses per BR-17.3; mock-server tests (the API base is injectable in tests only). *US-2.6.*
- [ ] **Step 15 - `sandbox` crate (LC-29).** Docker runtime probe: one `GET /version` over the Unix socket, 3 s, 64 KiB cap; `not_configured`, `reachable`, `unreachable`; `cfg(unix)` tests with a Unix listener. *US-2.9; SE-23, N6.*
- [ ] **Step 16 - Pull request 2 verification and summary.** Gates; `code/pr2-adapters-summary.md`. Open pull request 2.

### Pull request 3: persistence, application, worker, CLI
- [ ] **Step 17 - Migration 0005 (reversible).** `providers`, `provider_models`, `model_defaults`, `search_configs`, `search_active`, `connectors`, `sandbox_policies` (key: scope, with `version`), `config_test_runs` (partial unique index on target while `queued` or `running`); sealed `bytea` columns; foreign keys (defaults set null on delete; models removed with their provider); grants for the app role (SE-26); down migration; the schema-version expectation moves to 5. *T12.*
- [ ] **Step 18 - `store-postgres`: repositories.** Providers and models, defaults, search, connector, policy, test runs; secrets sealed and opened through the vault with the three purposes bound to the row; a dedicated-connection advisory lock helper (R2); cleanup additions (D3); regenerate `.sqlx`. *US-2.1 to 2.9; SE-20.*
- [ ] **Step 19 - `ports` repository traits and in-memory fakes.** Traits for the repositories and `Tx` accessors; matching fakes in `app::testing` (including the unique active-run rule and a lock double).
- [ ] **Step 20 - `app`: admin configuration service (LC-34), part 1.** Providers (W16), removal with impact (W18), defaults (W19), search settings and activation (W20), connector (W21), sandbox policy (W22) and the runtime check start; audit in the same transaction; secret semantics (BR-20.3). Unit tests with the fakes. *US-2.1, 2.4, 2.5, 2.6, 2.9.*
- [ ] **Step 21 - `app`: test start and read.** Start a run (insert plus enqueue in one transaction, one active per target, `already running` with the active id), read a run, latest result per target. *US-2.3, 2.5, 2.6, 2.9; RL-8.*
- [ ] **Step 22 - `app`: test runner (LC-32).** The `config_test` job handler: advisory lock on a held connection, `running`, probes through the adapters with results stored as they land, 60 s budget, finish, audit `config.test.run`; failure paths (D2); the recovery sweep for stuck `queued` runs. Tests include crash and retry, lock contention, partial results, audit-write failure. *PF-8, RL-8, RL-9.*
- [ ] **Step 23 - `app`: config read port and cache (LC-31).** `list_models`, `defaults`, `resolve_model`, `active_search`, `github_credentials`, `effective_policy`; 5 s snapshot cache cleared on local writes; non-loggable connection objects; policy `version` capture. Tests including the cache limit and clamping. *SC-6, PF-9; D5, D6.*
- [ ] **Step 24 - `worker`: wiring.** Register the `config_test` handler; run the recovery sweep in a supervised loop; the cleanup job prunes test runs; health and metrics unchanged. Integration test: a started run completes through the real loops.
- [ ] **Step 25 - `cli`: `vault rekey`.** Cover the new sealed columns in the same transaction; test that provider keys, search keys and the connector token open after a rekey with only the new key. *RL-10.*
- [ ] **Step 26 - Postgres integration tests.** Two workers and one run; defaults cleared when a provider is removed; unique active run; grants (the app role cannot touch the audit table, can change config tables); vault round trip; migration up and down.
- [ ] **Step 27 - Pull request 3 verification and summary.** Gates; `code/pr3-persistence-application-summary.md`. Open pull request 3.

### Pull request 4: API
- [ ] **Step 28 - API: providers and defaults.** DTOs and routes for providers (list, create, update, delete, impact) and model defaults; admin-only through the route table; secrets never returned (`saved`, `saved_at`); validation errors with field names. *US-2.1, 2.4.*
- [ ] **Step 29 - API: search, connector, policy, tests.** Routes for search kinds and activation, the GitHub token (save, remove, status), the sandbox policy (read, save, check runtime), test starts (202 with the run, 409 when active) and `GET /api/admin/test-runs/{id}`. *US-2.3, 2.5, 2.6, 2.9.*
- [ ] **Step 30 - Pipeline and OpenAPI.** New rate-limit bucket `Test` (10 per minute per source); register all routes with `AdminUser`; annotate for OpenAPI; update the pinned route lists; regenerate `apps/api/openapi.json` and the web types. *SE-24, MT-12.*
- [ ] **Step 31 - API tests.** Non-admin and anonymous access, CSRF, secrets never in any response or log line, URL refusals with reasons, test-run lifecycle against the real worker loops with mock servers (Docker Postgres), audit entries with before and after values, rate limit on test starts, concurrency (second start answers 409).
- [ ] **Step 32 - Pull request 4 verification and summary.** Gates plus a smoke run of the existing e2e suite; `code/pr4-api-summary.md`. Open pull request 4.

### Pull request 5: web, end-to-end, documentation
- [ ] **Step 33 - Web foundations.** `Integrations` nav entry and `AdminIntegrationsLayout` (sub-navigation as links), the integrations message catalogue, shared components `SecretField`, `StatusBadge`, `TestResultView`, `UrlField`, `HostListEditor`, `CapabilityWarning`, a test-run polling hook (1 s, 90 s limit); component tests with axe. *US-5, BR-20.*
- [ ] **Step 34 - ProvidersPage.** Table, `ProviderDialog` with `ModelListEditor`, test panel with per-model results and capability badges, `DefaultModelsCard` with warnings, removal dialog with the impact list; tests. *US-2.1, 2.3, 2.4.*
- [ ] **Step 35 - SearchPage.** Three kind cards, settings, test, make active; tests. *US-2.5.*
- [ ] **Step 36 - GitHubPage.** Token form, permissions notice, test result with repositories, removal; tests. *US-2.6.*
- [ ] **Step 37 - SandboxPolicyPage.** Runner tier with the runtime check, tool list with `command_run` disabled, network rule, limits with factory defaults and "Reset to default"; tests. *US-2.9.*
- [ ] **Step 38 - Accessibility pass.** Every page and dialog has an axe check, keyboard walk-through tests and 360 px checks; headings and live regions reviewed.
- [ ] **Step 39 - End-to-end.** A small Node mock module (OpenAI-compatible, SearXNG) started and stopped by `dev/scripts/e2e.sh`; scenarios: add a provider with a mock, test it and read capabilities, set defaults and see a warning for a weak model, remove a provider and see defaults cleared; switch search provider and test; save and remove the GitHub token; edit the sandbox policy and reset a limit; container tier unavailable without a socket; non-admins get "not found"; axe on every U2 page in both themes. *All U2 stories.*
- [ ] **Step 40 - Documentation.** README, `docs/development.md` (settings, container-socket warning, outbound rules), `docs/api.md`, and a short admin guide `docs/integrations.md` (providers, search, GitHub token permissions, sandbox policy).
- [ ] **Step 41 - Final verification and close-out.** Run every gate and the e2e suite; extend the BR-to-test table for BR-14 to BR-21; tick every story below; write `code/u2-code-generation-summary.md`; update `aidlc-state.md`. Open pull request 5.

## 6. Story traceability (stories ticked when their steps are done)
| Done | Story | Steps |
|---|---|---|
| [ ] | US-2.1 Configure an OpenAI-compatible provider | 2, 3, 6, 11, 12, 17, 18, 20, 28, 34, 39 |
| [ ] | US-2.3 Test a provider connection | 2, 6, 12, 21, 22, 29, 31, 34, 39 |
| [ ] | US-2.4 Set default models | 2, 17, 18, 20, 23, 28, 34, 39 |
| [ ] | US-2.5 Configure web search | 3, 13, 18, 20, 21, 29, 35, 39 |
| [ ] | US-2.6 Configure the GitHub connector | 6, 14, 18, 20, 21, 29, 36 |
| [ ] | US-2.9 Configure the sandbox policy | 4, 7, 15, 17, 18, 20, 23, 29, 37, 39 |

## 7. Extension compliance (planning)
- **Security Baseline**: steps 3, 6 and 9 carry the outbound controls (SE-18, SE-19); 8, 18 and 23 the secret handling (SE-20, SE-21); 12 to 14 SE-22; 15 SE-23; 30 and 31 SE-24; 17 SE-26. Compliance is re-evaluated at the completion message.
- **Resiliency Baseline**: steps 6 and 22 carry timeouts, single-flight, partial results and recovery (RL-7 to RL-11, AV-6); step 24 the supervised sweep.
