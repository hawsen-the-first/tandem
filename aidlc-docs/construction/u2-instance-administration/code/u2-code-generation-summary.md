# U2 Instance administration: code generation summary

All 41 steps of `construction/plans/u2-instance-administration-code-generation-plan.md` are done across five pull
requests. Each PR has its own summary file, and the business-rule to test table is `br-test-traceability.md`.

| PR | Steps | Contents | Summary |
|---|---|---|---|
| 1 (#16) | 1 to 10 | Domain (`core`: config entities, suitability, outbound URL rules, sandbox policy and limits), ports, audit events, redaction, the outbound HTTP client and its lint, settings | `pr1-foundations-summary.md` |
| 2 (#17) | 11 to 16 | Adapters: OpenAI-compatible provider probe, SearXNG, Brave and Tavily, GitHub, container runtime probe | `pr2-adapters-summary.md` |
| 3 (#18) | 17 to 27 | Migration 0005, configuration repositories, `PgLocks`, administration service, test runner, config read port, worker recovery and pruning, vault rekey | `pr3-persistence-application-summary.md` |
| 4 (#19) | 28 to 32 | Admin API for providers, defaults, search, GitHub, policy and test runs; the `Test` rate-limit bucket; OpenAPI and web types | `pr4-api-summary.md` |
| 5 | 33 to 41 | Web pages, shared components, accessibility pass, end-to-end scenarios with mocks, documentation, close-out | `pr5-web-e2e-docs-summary.md` |

## Stories

All six are delivered and ticked in the plan:
- US-2.1 provider configuration
- US-2.3 test a provider
- US-2.4 default models
- US-2.5 web search
- US-2.6 GitHub connector (fine-grained token)
- US-2.9 sandbox policy

US-2.12 (allow and block lists, U9) and US-2.13 (GitHub App, U5) were created by this unit's design for later units.

## Interfaces left for later units

- **Config read port (LC-31).** U4 uses it for models, defaults, active search and the effective policy; U5 uses it
  for GitHub credentials. It is a server-side read with a 5 s cache, and it never sends a secret to the browser.
- **`core::sandbox_policy`.** It holds the resolution and clamping rules of BR-18.8, so per-session policies (FR-16.6)
  can be added without changing the rule.
- **Runtime refusal.** Refusing to run a job on a model that lacks a needed capability (BR-14.8, BR-15.2) is enforced
  in U4. U2 stores the capabilities and shows the warnings.

## Verification performed

- `just check` passes (exit 0): rustfmt, clippy `-D warnings`, every Rust test including the Docker PostgreSQL
  suites, the web checks (types, lint, format, unit tests, build, `check:api`), `cargo audit`, `cargo deny` and
  `cargo machete`.
- The end-to-end suite passes with all 93 tests in Chromium and Firefox. That includes the U2 scenarios against the
  loopback mocks and the four U2 pages in the axe run, in both themes and at 360 px.
- Every rule from BR-14 to BR-21 maps to at least one test (`br-test-traceability.md`, MT-11). The rules not
  testable in U2 are named there: BR-16.5 is U4, and BR-19.8 is a Could-have that was not built.

## Open items needing your decision

1. **Playwright browsers on macOS.** The flake provides browsers on Linux only. A local `just e2e` on macOS needs a
   one-time `cd web && npx playwright install chromium firefox`. Add this to the README, or provide the browsers
   through the flake for Darwin?
2. **Static assets share the per-source rate limit** (300 per minute). Normal use is well within it because the
   hashed assets are cached as immutable. A deployment behind a shared NAT may want static files exempt. Change it,
   or leave it as is?
3. **A test run occupies one worker's job loop for up to 60 s** (noted in PR 3). Running two workers avoids chat
   replies waiting behind a test. Note it in the self-hosting guide (U8)?

## Security Baseline and Resiliency Baseline

No blocking findings.

| Rule | Status | Where |
|---|---|---|
| SE-18, SE-19 outbound controls | Compliant | `outbound` client and lint, BR-19 tests; the client mirror only adds earlier messages |
| SE-20, SE-21 secret handling | Compliant | Sealed per purpose, write-only API, `SecretField` clears on submit; tests check the DOM, storage, responses, audit and logs |
| SE-22 no remote text in results | Compliant | api `se_22_test_results_never_contain_remote_text`; the web shows the fixed messages only |
| SE-23 container socket | Compliant | Worker-only setting with a root-equivalence warning in `docs/development.md`; the e2e suite proves it is unavailable when unset |
| SE-24 test rate limit | Compliant | `Test` bucket, api `se_24_…` |
| SE-26 least privilege | Compliant | postgres `se_26_…` |
| SE-9 CSRF and Origin | Compliant | api `se_9_…`; the web client sends the token (asserted in the web tests) |
| BR-12 / US-5 accessibility | Compliant | axe in unit and e2e runs, keyboard tests, both themes, 360 px |
| RL-7 to RL-11, AV-6 | Compliant | Timeouts, single-flight with 409 follow, partial results, recovery sweep, polling limit in the client |
| Property-Based Testing | N/A | Extension disabled at Requirements Analysis |
