# U1 Foundation and walking skeleton: code generation summary

All 58 steps of `construction/plans/u1-foundation-code-generation-plan.md` are done across five pull requests.
Per-PR detail is in `pr1-scaffold-summary.md`, `pr2-domain-application-summary.md`,
`pr3-persistence-credentials-summary.md` and `pr4-api-worker-cli-summary.md`; this file covers PR 5 and the
whole unit. The business-rule to test table is `br-test-traceability.md`.

| PR | Steps | Contents |
|---|---|---|
| 1 | 1 to 8 | Workspace, tooling, CI, CodeQL, Dependabot, dev environment |
| 2 | 9 to 24 | `support`, `core`, `ports`, `app` (workflows W1 to W15), mock provider, in-memory fakes |
| 3 | 25 to 34 | PostgreSQL schema and adapter, outbox, queue, vault, credentials, breach filter |
| 4 | 35 to 47 | `config`, `telemetry`, `bootstrap`, `api`, `worker`, `cli`, contract and perf CI jobs |
| 5 | 48 to 58 | Web client, end-to-end suite and CI job, documentation, close-out |

## PR 5: what was created
| Area | Contents |
|---|---|
| Web foundations | Typed API client generated from the OpenAPI description (committed, checked in CI), CSRF header handling, error mapping to the business-rule wording, message catalogue, design tokens with light, dark and system themes, router and TanStack Query, session provider with route guards |
| Base components | `FormField`, `PasswordField` (live rules), `CopyOnceSecret`, `ConfirmDialog` (native `<dialog>`), `ResponsiveTable`, `Toast` with polite and assertive regions, `ErrorBoundary` |
| Pages | Setup, sign-in with MFA challenge and throttle countdown, invitation, reset, MFA enrolment (client-side QR), account, users (invite, role, reset link, deactivate, reactivate, revoke), settings, audit log, home, chat list and chat session with a WebSocket client (backoff, replay, resync) |
| Tests | 164 Vitest and Testing Library tests, each page and shared component with an axe check and keyboard cases; 61 Playwright runs (30 scenarios, Chromium and Firefox) against a real stack |
| Tooling | `dev/scripts/e2e.sh` (isolated PostgreSQL project, migrations, api serving `web/dist`, worker), `just e2e`, CI job `e2e` |
| Docs | `README.md`, `docs/development.md`, `docs/api.md`; unit-of-work and shared-infrastructure updated |

## Verification performed
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (382 passed, 1 performance test run separately), `cargo deny check`, `cargo audit`, `cargo machete`, `just sqlx-check`.
- Web: `eslint`, `prettier --check`, `tsc --noEmit`, 164 tests, `npm run check:api` (generated types current), `npm audit --audit-level=high`, production build.
- End-to-end: 61 of 61 pass in Chromium and Firefox: first-run setup to forced MFA, invitation to sign-in, forced reset, role change, deactivation and reactivation, throttling countdown, the message round trip over the WebSocket, private chats, axe (WCAG 2.2 AA tags) on every page in both themes, 360 px layouts, keyboard walk-throughs.
- Every business rule BR-1 to BR-13 maps to at least one test (`br-test-traceability.md`). Six rules that had no test of their own at the HTTP level got one (`apps/api/tests/business_rules.rs`).

## Defects the end-to-end suite found (and that are fixed)
These passed every component test and were found only against the real stack:
- **Sign-out left the person on a signed-in page.** Clearing the whole query cache kept the cached user, so the guards never redirected. Fixed by marking the session as ended; the shell test now asserts the redirect.
- **The navigation never collapsed on narrow screens.** A placeholder class name left behind by a scripted edit meant the stylesheet's rules never matched. Fixed, with a regression assertion on the open state.
- **The user menu's accessible name did not contain its visible text** (axe `label-content-name-mismatch`).
- **The audit filters were documented as path parameters** in the OpenAPI description (found while building the audit page). Fixed at the source; the web client now uses typed query parameters.

## Design notes and deviations (PR 5)
| Item | Detail |
|---|---|
| No Radix UI or React Aria | The tech-stack decision left the pick to this stage. Both inject styles at run time (scroll locking, positioning), which the strict CSP (`style-src 'self'`) forbids. Dialogs use the native `<dialog>` element, menus use `<details>`, toasts are plain live regions |
| Chat title limit | 120 characters (`TITLE_MAX_CHARS` in `core`), not the 80 the page directive said |
| Browser versions | The Nix shell provides Playwright browsers 1.64.1, so `@playwright/test` is pinned to exactly that; bump the flake lock and the npm pin together. npm `overrides` align `openapi-typescript` (TypeScript peer) and `@axe-core/playwright` (its own `playwright-core`) |
| e2e client addresses | The e2e stack trusts loopback as a proxy, and each spec file presents its own `X-Forwarded-For`, so one file cannot use up another's per-source allowance. This also exercises the proxy path |
| Safari | Not in the e2e suite: it does not accept `Secure` cookies on `http://localhost` (documented in the README) |
| `npm audit` | Ten moderate findings in development tooling; CI gates at high and passes |
| Crate name | The shared start-up crate is `bootstrap`, not `runtime`, because the unit plan reserves `runtime` for the later agent loop |

## Open items needing your decision
1. **OTLP export is not implemented** (PR 4): trace IDs are logged and propagated; nothing is exported. Follow-up PR?
2. **HTTP header-read timeout (10 s)** is not enforced (`axum::serve`); the 30 s request timeout is.
3. **CodeQL alert on the CLI output** (`apps/cli/src/lib.rs`, a count of re-sealed MFA credentials) is a false positive awaiting dismissal; the two SHA-1 alerts in `breach.rs` are required by the breach-check protocol.
4. **Required checks on `main`**: add `contract`, `perf` and `e2e` once this PR has run them.
5. **Test harness duplication**: the Docker-based Postgres harness is copied into the `api`, `worker` and `cli` test directories; a shared test-support crate would remove it.
6. **Audit prune is not itself audited** (the catalogue has no such event).

## Security Baseline and Resiliency Baseline
No blocking findings. Controls added in this unit are listed against their rule IDs in the NFR design documents and
exercised by the tests above (headers and CSP, CSRF and Origin, sessions and scopes, rate limits, redaction, vault,
append-only audit, TLS to the database, graceful shutdown, readiness, supervised loops).
