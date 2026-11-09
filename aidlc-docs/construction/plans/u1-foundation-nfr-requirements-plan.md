# NFR Requirements Plan: U1 Foundation and walking skeleton

**Stage**: CONSTRUCTION - NFR Requirements (unit `u1-foundation`)
**Inputs**: U1 functional design (approved), `requirements.md` (NFR-1 to NFR-7), `application-design.md`, `unit-of-work.md`
**Extensions enforced**: Security Baseline (SECURITY-01 to 15) and Resiliency Baseline (product scope). Property-Based Testing is disabled.
**This stage also settles** the framework choices that NFR-1.5 deferred (front-end stack, test stack, observability stack).

## Findings to resolve (from the extension rules)

| ID | Finding | Why it matters |
|---|---|---|
| **F1 (SECURITY-14)** | The approved design lets the application prune its own audit log (BR-11.2, Q8 B), and allows retention as low as 1 month (BR-10.1). SECURITY-14 requires audit logs to be tamper-evident with application code **unable to delete or modify them**, and a **minimum retention of 90 days** | Blocking unless resolved. See Question 2. The retention floor becomes 3 months in any case (FR-1.9 and BR-10.1 are amended) |
| **F2 (SECURITY-01)** | Encryption in transit to PostgreSQL must be enforced, and storage encryption at rest must be defined | See Question 3. At-rest protection for secrets is application-level (decided); disk encryption for the database volume is an operator decision documented in the self-hosting guide (NFR-5.4) |
| **F3 (SECURITY-14, RESILIENCY-05)** | Alerting and dashboard definitions are required, plus tracing for a multi-service system | The product ships example alert rules and a dashboard definition (U8) and defines the metrics, logs and traces in U1. See Question 6 for tracing |

## Working assumptions (not questions)
- **N1**: Targets from NFR-3.1 stand: one team of tens of users on one host.
- **N2**: Logs are structured JSON on standard output with timestamp, level, request ID, component and message; no secrets, tokens, passwords or full emails of unknown accounts (SECURITY-03).
- **N3**: Metrics are exposed in Prometheus format; `/healthz` (shallow) and `/readyz` (deep: database, queue, vault) exist on both `api` and `worker` (RESILIENCY-06).
- **N4**: Every outbound call (database, HTTP, DNS) has an explicit timeout; connection pools are bounded; the API and the worker use separate pools (bulkheads, RESILIENCY-10).
- **N5**: A global error handler returns generic messages and logs details with the request ID (SECURITY-15); all handlers fail closed.
- **N6**: Dependencies are pinned by lock files; CI (minimal in U1) runs build, tests, lint, `cargo audit`/`cargo deny` and `npm audit`; production images use pinned base images (SECURITY-10).
- **N7**: Responses on HTML-serving routes carry the SECURITY-04 headers (strict CSP without `unsafe-inline` or `unsafe-eval`, HSTS, nosniff, frame denial, referrer policy). No external CDN resources: fonts and scripts are bundled, so SRI is not needed (SECURITY-13).
- **N8**: CORS is closed by default (same-origin only); no wildcard origins on authenticated endpoints (SECURITY-08).
- **N9**: State-changing requests use a SameSite session cookie plus a per-session CSRF token sent in a header and checked on every request; WebSocket connections check the Origin header.
- **N10**: Request body limits are set at the framework level (default 1 MB; uploads arrive in U3 with their own limit).

## Proposed technology decisions (confirm or change at approval)
| Area | Proposal | Reason |
|---|---|---|
| HTTP and runtime | Axum on Tokio | Already in NFR-1.2 |
| Database access | SQLx with reversible migrations, parameterised queries only | NFR-1.2, NFR-5.5, SECURITY-05 |
| Password hashing | `argon2` crate, Argon2id | SECURITY-12 |
| TOTP | A maintained TOTP crate (RFC 6238) with constant-time comparison | |
| TLS | `rustls` for outbound and database connections | Memory-safe, no system OpenSSL dependency |
| Logging and tracing | `tracing` with JSON output; OpenTelemetry export optional (Question 6) | SECURITY-03, RESILIENCY-05 |
| Metrics | Prometheus exposition | NFR-5.7 |
| Validation | Typed request structs with a validation crate; strict `serde` (deny unknown fields) | SECURITY-05, SECURITY-13 |
| Front end | React with TypeScript, Vite, React Router, TanStack Query | NFR-1.1, decision Q7 of Application Design |
| API types | OpenAPI generated from the server, TypeScript client generated from it | Decision D9 |
| Unit tests | `cargo test` (Rust), Vitest and Testing Library (front end) | NFR-7.1 |
| End-to-end and accessibility | Playwright with axe-core checks | NFR-6.2, NFR-7.1 |
| Supply chain | `cargo audit`, `cargo deny` (licences must be GPL-3.0 compatible, NFR-7.4), `npm audit`, lock files committed | SECURITY-10 |

## Questions

## Question 1
How should Argon2id be tuned (SECURITY-12)?

A) Fixed at the OWASP minimum profile (19 MiB memory, 2 iterations, 1 lane): light on small hosts

B) Fixed stronger profile (64 MiB memory, 3 iterations, 1 lane): better against offline attack, heavier per sign-in

C) Benchmarked at first start to take about 250 ms on the host, parameters stored with each hash so they can be raised later without locking anyone out

D) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 2
How should the audit log meet SECURITY-14 (application code must not be able to delete or modify its own audit logs; minimum retention 90 days)? This changes the pruning design from Functional Design.

A) Separate database roles: the running application can only add and read audit entries. Automatic pruning is done by a small dedicated job under its own database role that can only delete entries past the retention period (never edit). Retention floor 3 months

B) No automatic pruning: entries are kept indefinitely and operators prune with a documented command using a privileged credential they control. The retention setting is removed from US-1.9

C) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 3
How should the connection between Tandem and PostgreSQL be protected (SECURITY-01)?

A) TLS required by default: the reference Compose file provisions a certificate for PostgreSQL and the app refuses unencrypted connections; operators can supply their own certificate

B) TLS required, but the operator provides the certificate; the reference Compose file does not provision one

C) TLS optional on the private container network, documented as an operator decision (does not meet SECURITY-01 and would need a documented exception)

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4
Which front-end component approach should be used? It must work with a strict CSP (no inline scripts, no `unsafe-inline`) and meet WCAG 2.2 AA.

A) Accessible headless primitives (for example Radix UI or React Aria) with our own CSS and design tokens; full control over light and dark themes, no runtime style injection

B) A full component kit (for example Mantine or MUI): faster to start, heavier, and some kits need CSP nonces for runtime-injected styles

C) Hand-built components only: no dependency, but more accessibility work

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
What performance targets should U1 be tested against (the main design target is NFR-3.1: tens of users, one host)?

A) Typical API calls under 200 ms at the 95th percentile; sign-in under 1 second (includes hashing); skeleton message round trip with the mock agent under 500 ms on a LAN

B) Relaxed: typical API calls under 500 ms; sign-in under 2 seconds; round trip under 1.5 seconds

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
How should distributed tracing be handled across `api` and `worker` (RESILIENCY-05)?

A) OpenTelemetry tracing across both services, with the trace ID in every log line; exported over OTLP only when the operator sets an endpoint (off by default)

B) Request and job correlation IDs in logs only, with no trace export (documented as a deliberate simplification for a single-team product)

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
How should integration tests get a PostgreSQL database?

A) Throw-away containers started by the tests themselves (for example testcontainers): isolated and repeatable, needs Docker in CI and on developer machines

B) A shared development database from the dev compose file, reset between runs: simpler setup, risk of state leaking between tests

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
Where should the secret vault's master key come from (SECURITY-01, SECURITY-12)? It seals provider keys, connector tokens and TOTP secrets.

A) A random 256-bit key supplied by the operator through an environment variable or a file path (file preferred), with a key ID stored beside each sealed value so the key can be rotated

B) Derived from a passphrase the operator types at every start-up: no key at rest on the server, but the service cannot restart unattended

C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Execution Checklist (generation, after answers are approved)
- [x] 1. Convert the working assumptions and answers into numbered NFR requirements by category (scalability, performance, availability, security, reliability, maintainability, usability); write `nfr-requirements.md`
- [x] 2. Record every technology decision with rationale and alternatives considered; write `tech-stack-decisions.md`
- [x] 3. Evaluate SECURITY-01 to SECURITY-15 and the applicable RESILIENCY rules for U1 and record compliance, N/A and findings
- [x] 4. Amend the approved requirements and Functional Design where the answers require it (retention floor, audit pruning, settings story)
- [x] 5. Update state, log the approval prompt, present the completion message (with Security and Resiliency compliance summaries; no "Continue" option while any finding is blocking)

## Extension compliance (planning)
- Security Baseline: enforced; findings F1 and F2 are open until Questions 2 and 3 are answered
- Resiliency Baseline: enforced (RESILIENCY-04 and -12 are delivered in U8; operator-owned decisions stay in the self-hosting guide)
- Property-Based Testing: disabled
