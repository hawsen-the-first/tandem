# U1 Foundation: NFR Requirements

**Unit**: U1 Foundation and walking skeleton. These requirements apply to everything U1 builds and set the baseline that later units inherit.
**Sources**: `requirements.md` NFR-1 to NFR-7, U1 functional design, the plan answers (Q1 to Q8), and the Security and Resiliency Baseline rules.
IDs: **SC** scalability, **PF** performance, **AV** availability, **RL** reliability, **SE** security, **MT** maintainability and testing, **US** usability, **OB** observability.

## 1. Scalability
| ID | Requirement |
|---|---|
| SC-1 | Target: one team of tens of users on a single host (NFR-3.1). No clustering is required in U1 |
| SC-2 | The `api` process is stateless: sign-ins, tokens and throttling counters live in PostgreSQL (or are safe to lose), so several `api` instances can run behind a proxy later (NFR-3.2) |
| SC-3 | Background work runs through the PostgreSQL job queue with `SKIP LOCKED`, so more workers can be added without code change (skeleton stub in U1, full queue in U4) |
| SC-4 | Connection pools are bounded and separate for `api` and `worker` (bulkheads) |

## 2. Performance (Q5 A)
| ID | Requirement |
|---|---|
| PF-1 | Typical API calls complete in under **200 ms at the 95th percentile** at design load (tens of users) |
| PF-2 | Sign-in completes in under **1 second**, including password hashing |
| PF-3 | The skeleton round trip (post a message, mock agent reply visible in the browser) takes under **500 ms on a LAN** (NFR-3.1) |
| PF-4 | Argon2id is benchmarked at first start to take about 250 ms on the host; the parameters are stored with each hash so they can be raised later without invalidating existing passwords (Q1 C). If the host cannot meet about 250 ms within sensible memory bounds, the benchmark stops at a documented minimum profile and logs a warning |
| PF-5 | Every lookup used on a hot path (email, session token hash, invitation and reset token hash, audit filters) has a supporting index |
| PF-6 | A smoke performance test runs in CI against PF-1 to PF-3 with a small seeded dataset; regressions fail the build |

## 3. Availability (Resiliency: RESILIENCY-06, product scope)
| ID | Requirement |
|---|---|
| AV-1 | `api` and `worker` each expose `/healthz` (process is running) and `/readyz` (database reachable, migrations current, vault key usable, queue reachable) |
| AV-2 | Graceful shutdown: stop accepting work, finish or hand back in-flight requests and jobs within a timeout, then exit; a restart loses no committed state |
| AV-3 | Start-up refuses to serve if the database schema is newer or older than the binary expects, with a clear message |
| AV-4 | Migrations are reversible and versioned; `tandem migrate up|down` runs them under an advisory lock (NFR-5.5) |
| AV-5 | RTO, RPO, topology and disaster recovery are operator decisions (NFR-5.4); backup and restore commands are delivered in U8 |

## 4. Reliability (RESILIENCY-10, SECURITY-15)
| ID | Requirement |
|---|---|
| RL-1 | Every external call (database, DNS, HTTP including the optional online breach check) has an explicit timeout; no unbounded waits |
| RL-2 | The optional online breach check uses a 2-second timeout and a circuit breaker; when it is open or failing, the offline list alone decides and the event is logged (graceful degradation, BR-2.3) |
| RL-3 | A global error handler turns any unhandled error into a generic response and a structured log entry with the request ID; handlers fail closed (SECURITY-15) |
| RL-4 | Transactions roll back on error; connections, locks and files are released on every path |
| RL-5 | Outbox publishing is at-least-once and consumers are idempotent; a job retries up to 3 times with growing delay, then is marked failed (BR-13.3, BR-13.4) |
| RL-6 | A failure of the audit write fails the operation it records (BR-11.1) |

## 5. Security (Security Baseline)
| ID | Requirement | Rule |
|---|---|---|
| SE-1 | The connection from `api`, `worker` and the migration tool to PostgreSQL requires TLS 1.2 or higher with certificate verification. The reference Compose file provisions a certificate; operators may supply their own. The app refuses to start with an unencrypted database URL (Q3 A) | SECURITY-01 |
| SE-2 | Secrets (TOTP secrets, later provider keys and connector tokens) are sealed at application level with AES-256-GCM using an operator-supplied 256-bit master key from a file path (preferred) or environment variable; a key ID is stored beside every sealed value so the key can be rotated (Q8 A). Tokens and passwords are stored only as hashes. Volume or disk encryption for the database and blob volumes is an operator control documented in the self-hosting guide (NFR-5.4) | SECURITY-01 |
| SE-3 | The application writes a structured access log line for every request (method, route template, status, duration, request ID, user ID when known) to standard output. Access logs for the operator's reverse proxy are documented as the operator's responsibility | SECURITY-02 |
| SE-4 | JSON logs on standard output with timestamp, level, request or job ID, component and message, sent to the operator's log collector. A redaction layer drops fields named like passwords, tokens, secrets and codes; emails of unknown accounts are never logged; user IDs, not emails, identify actors in logs | SECURITY-03 |
| SE-5 | All HTML-serving responses carry: strict `Content-Security-Policy` (no `unsafe-inline` or `unsafe-eval`; scripts and styles from the same origin only), `Strict-Transport-Security: max-age=31536000; includeSubDomains`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: strict-origin-when-cross-origin`. The front end therefore uses static stylesheets and no runtime style injection (Q4 A) | SECURITY-04 |
| SE-6 | Every endpoint and WebSocket message is validated by typed request structures with length bounds, allow-listed formats and unknown fields rejected; request bodies are capped at 1 MB and WebSocket messages at 64 KB; every SQL statement is parameterised; no user input reaches a shell | SECURITY-05 |
| SE-7 | Containers run as non-root with a read-only root filesystem where possible. Two database roles: **migration** (schema changes only, used by `tandem migrate`) and **app** (data access for `api` and `worker`). On the audit table the app role has `INSERT` and `SELECT` only. Roles hold specific privileges, no wildcard grants | SECURITY-06 |
| SE-8 | The reference Compose network exposes only the `api` entry point; PostgreSQL is internal. The dev-only compose file binds services to localhost | SECURITY-07 |
| SE-9 | Deny by default: every route requires a valid session except an explicit public list (first-run setup, sign-in, invitation and reset-token endpoints, `/healthz`, `/readyz`, static assets). Object-level checks on every resource ID (skeleton sessions are owner-only); function-level role checks on the server; CORS is same-origin only with no wildcard; the session token is validated against the database on every request (existence, expiry, status, scope); WebSocket upgrade checks the session and the Origin header | SECURITY-08 |
| SE-10 | No default credentials; debug features off in release builds; production errors are generic; the static file server lists no directories; only needed endpoints are exposed; the metrics endpoint is served on a separate internal port or protected by a token | SECURITY-09 |
| SE-11 | Lock files are committed (`Cargo.lock`, the front-end lock file); CI runs `cargo audit`, `cargo deny` (advisories, licences GPL-3.0 compatible, sources limited to crates.io and the npm registry) and `npm audit`; unused dependencies are removed (`cargo machete` or equivalent); toolchain and base-image versions are pinned with no `latest` tags; the SBOM for production images is produced in U8 | SECURITY-10 |
| SE-12 | Authentication and authorisation live in dedicated modules (identity in `app`, enforced by middleware in `api`). Rate limiting: sign-in and token endpoints per BR-3.4; all other endpoints 300 requests per minute per source by default. Misuse cases are listed in section 5.1 | SECURITY-11 |
| SE-13 | Password policy per FR-1.8; Argon2id (PF-4); breached-password check (offline always, optional online); TOTP MFA required for admins; sessions with server-side expiry, invalidated on sign-out, cookies `Secure`, `HttpOnly`, `SameSite`; brute-force protection per FR-1.10; no hardcoded credentials, with configuration from environment variables and files | SECURITY-12 |
| SE-14 | Deserialisation is typed with unknown fields denied; no external CDN scripts (fonts and scripts are bundled), so SRI is not needed; role changes and setting changes record before and after values; the repository's `main` branch is protected with required CI checks and required review so pipeline definitions and code cannot be changed without review (repository setting to enable) | SECURITY-13 |
| SE-15 | The audit log is append-only: the app database role cannot update or delete audit rows; there is no automatic pruning (Q2 B), so retention exceeds the 90-day minimum; operators prune only with a documented command and a privileged credential. Alerting rules for repeated sign-in failures, lockouts, authorisation denials and privilege changes are defined from the OB-1 metrics and shipped as example rules with a dashboard definition in U8. Retention of ordinary application logs is the operator's, with 90 days or more recommended | SECURITY-14 |
| SE-16 | Global error handler, fail-closed behaviour, resource cleanup and generic user-facing messages as in RL-3 and RL-4; no unhandled panics escape a request (panics are caught at the task boundary and logged) | SECURITY-15 |
| SE-17 | First-run setup is available only while no user exists and cannot be re-enabled by configuration | SECURITY-09, FR-1.2 |

### 5.1 Misuse cases considered (SECURITY-11)
| Misuse | Control |
|---|---|
| Guess passwords or probe which emails exist | Progressive delay, lockout, per-source limit, identical responses and timing (BR-3) |
| Replay a captured TOTP code | Time-step reuse refused (BR-4.4) |
| Use an old or leaked invitation or reset link | Single-use, expiring, hashed tokens, one generic error (BR-5.4, BR-6.2) |
| Lock a colleague out on purpose | Delay before lockout, short 15-minute lock, per-source limit, audit trail |
| Cross-site request forgery | SameSite cookie plus CSRF token header on every state-changing request; WebSocket Origin check |
| Escalate by tampering with the role or scope | Role and scope read from the database per request (BR-9.1); admin resources hidden from non-admins |
| Remove the last admin to take over | Last-admin rule enforced in the same transaction (BR-7.2) |
| Tamper with the audit trail | App role has no update or delete permission on audit rows |

## 6. Observability (RESILIENCY-05, RESILIENCY-07, Q6 A)
| ID | Requirement |
|---|---|
| OB-1 | Prometheus metrics at an internal endpoint, including: `http_requests_total` and `http_request_duration_seconds` by route and status; `auth_sign_in_total` by outcome; `auth_lockouts_total`; `authz_denied_total`; `db_pool_connections` (in use and size) and `db_query_duration_seconds`; `outbox_pending`; `job_queue_depth`, `job_oldest_age_seconds`, `job_duration_seconds`; `ws_connections`; process memory and CPU |
| OB-2 | OpenTelemetry tracing across `api` and `worker`: the trace context travels in the job record, and the trace ID appears in every log line. Export over OTLP only when the operator sets an endpoint; off by default |
| OB-3 | A dashboard definition and example alert rules for the OB-1 metrics are delivered as files in U8; U1 guarantees the metrics exist |
| OB-4 | Resiliency alarms to ship as examples: database pool saturation, queue age growth, outbox backlog, readiness failing, backup age (backup arrives in U8) |

## 7. Maintainability and testing (NFR-7)
| ID | Requirement |
|---|---|
| MT-1 | Every business rule BR-1 to BR-13 has at least one automated test; domain rules are unit-tested without I/O |
| MT-2 | Integration tests run against a real PostgreSQL started in a throw-away container by the tests (Q7 A), including migrations up and down |
| MT-3 | API contract: the OpenAPI description is generated from the server and the TypeScript client from it; CI fails if they drift |
| MT-4 | End-to-end tests (Playwright) cover first-run setup, invitation, sign-in with MFA, forced reset, role change, deactivation, and the walking skeleton round trip; the skeleton test stays green through later units |
| MT-5 | Accessibility checks (axe) and a keyboard walk-through run on every U1 screen (US-13.7) |
| MT-6 | Model calls are mockable: the scripted mock provider is deterministic (NFR-7.3, BR-13.5) |
| MT-7 | Static checks fail the build: `rustfmt`, `clippy` with warnings denied, strict TypeScript, ESLint, Prettier |
| MT-8 | Minimal CI in U1 (build, test, lint, dependency scan) runs on every pull request; main is protected (SE-14) |

## 8. Usability and accessibility (NFR-6, US-13.7)
| ID | Requirement |
|---|---|
| US-1 | WCAG 2.2 AA for every U1 screen, both themes (BR-12.3) |
| US-2 | Usable from 360 px wide; light, dark and system themes |
| US-3 | English only in the MVP, but user-facing text is kept in a message catalogue rather than scattered in components, so translation can be added later |
| US-4 | Error messages follow the patterns in `business-rules.md` and never expose internals |

## 9. Compliance summary

### Security Baseline (U1)
| Rule | Status | Evidence |
|---|---|---|
| SECURITY-01 Encryption | Compliant (design) | SE-1 (DB TLS enforced), SE-2 (secrets sealed; disk encryption documented as operator control) |
| SECURITY-02 Access logging | Compliant (design) | SE-3; proxy logs are the operator's, documented |
| SECURITY-03 Application logging | Compliant (design) | SE-4 |
| SECURITY-04 Security headers | Compliant (design) | SE-5 |
| SECURITY-05 Input validation | Compliant (design) | SE-6 |
| SECURITY-06 Least privilege | Compliant (design) | SE-7 |
| SECURITY-07 Network | Compliant (design) | SE-8 |
| SECURITY-08 Access control | Compliant (design) | SE-9 |
| SECURITY-09 Hardening | Compliant (design) | SE-10, SE-17 |
| SECURITY-10 Supply chain | Compliant (design) | SE-11; SBOM in U8 for production images |
| SECURITY-11 Secure design | Compliant (design) | SE-12, section 5.1 |
| SECURITY-12 Authentication | Compliant (design) | SE-13 |
| SECURITY-13 Integrity | Compliant (design) | SE-14 |
| SECURITY-14 Alerting and monitoring | Compliant (design) | SE-15, OB-1, OB-3 (alert and dashboard files delivered in U8) |
| SECURITY-15 Fail-safe | Compliant (design) | RL-3, RL-4, SE-16 |

Open blocking findings: **none** (F1 resolved by no automatic pruning plus an append-only app role; F2 resolved by required database TLS; F3 resolved by OB-2 and OB-3).

### Resiliency Baseline (U1, product scope)
| Rule | Status | Note |
|---|---|---|
| RESILIENCY-01 Criticality | Compliant | Done in Application Design |
| RESILIENCY-02 Recovery targets | Operator-owned | Self-hosting guide (NFR-5.4) |
| RESILIENCY-03 Change management | Compliant (design) | Pull-request review, changelog and release notes (NFR-5.8); required CI checks (SE-14, MT-8) |
| RESILIENCY-04 Deployment and rollback | N/A for U1, delivered in U8 | GitHub Actions (NFR-5.8); rollback by image version (NFR-5.5); reversible migrations (AV-4) |
| RESILIENCY-05 Monitoring | Compliant (design) | OB-1 to OB-3 |
| RESILIENCY-06 Health checks | Compliant (design) | AV-1 |
| RESILIENCY-07 Resiliency monitoring | Compliant (design) | OB-4 |
| RESILIENCY-08, 09, 11, 13, 14, 15 | Operator-owned | Topology, scaling, DR, failover, testing and incident response are operator decisions (NFR-5.4) |
| RESILIENCY-10 Dependency isolation | Compliant (design) | RL-1, RL-2, SC-4 |
| RESILIENCY-12 Backup | N/A for U1, delivered in U8 | Backup and restore commands (NFR-5.6) |
