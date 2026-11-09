# NFR Design Plan: U1 Foundation and walking skeleton

**Stage**: CONSTRUCTION - NFR Design (unit `u1-foundation`)
**Inputs**: `nfr-requirements.md` and `tech-stack-decisions.md` (approved), U1 functional design, `application-design.md`
**Goal**: turn the NFRs into concrete patterns and logical components that Code Generation can build directly.

## Pattern areas and how they will be designed
| Area | Patterns planned | NFR source |
|---|---|---|
| Resilience | Timeouts everywhere, circuit breaker on the online breach check, bounded pools per process, transactional outbox, idempotent consumers, retry with backoff, graceful shutdown, fail-closed error handling | RL-1 to RL-6, AV-2 |
| Scalability | Stateless `api`, `SKIP LOCKED` queue, `LISTEN/NOTIFY` wake-ups with a polling fallback, pool bulkheads | SC-1 to SC-4 |
| Performance | Indexed hot paths, a bounded Argon2 worker pool, no per-request network calls beyond PostgreSQL, a small performance smoke test | PF-1 to PF-6 |
| Security | Layered middleware chain, typed authorisation extractors, session and CSRF design, account guard and per-source limiter, vault envelope, DB role split, log redaction, security headers | SE-1 to SE-17 |
| Observability | Metrics registry, structured logging with request and job IDs, trace context carried in jobs, health and readiness checks | OB-1 to OB-4, AV-1 |

## Working design assumptions (not questions)
- **D1 Middleware order** (outermost first): request ID, access log, panic catcher and global error handler, security headers, per-source rate limit, body size limit, session resolution, CSRF check (state-changing methods), authorisation extractor in the handler. Public routes skip session resolution and CSRF but keep all the others.
- **D2 Authorisation by type**: handlers receive typed extractors (`AnyUser`, `FullScopeUser`, `AdminUser`) so a handler cannot compile without declaring what it needs; admin extractors return "not found" to non-admins (BR-9.2). Deny by default: a route with no extractor and not on the public list fails a start-up check.
- **D3 Session cookie**: name with the `__Host-` prefix (Secure, Path=/, no Domain), HttpOnly, SameSite=Lax; value is a random token whose SHA-256 hash is looked up. CSRF token per session, returned by `/auth/me` and sent in a custom header; compared in constant time.
- **D4 Account guard**: failure count, delay and lock stored in PostgreSQL and updated with row-level locking, so concurrent attempts cannot bypass the delay. Unknown emails use a bounded, expiring in-database guard keyed by a hash of the normalised email.
- **D5 Vault envelope**: each sealed value stores key ID, nonce and ciphertext; the key file may hold several keys (one current, others for decryption only); `tandem vault rekey` re-seals values under the current key.
- **D6 Outbox relay**: a relay task in the `worker` reads unpublished outbox rows in order, publishes them, and marks them published; the `api` also listens to `NOTIFY` to push events to its local WebSockets. `NOTIFY` only wakes listeners; correctness comes from the outbox rows, and a 1-second poll covers missed notifications.
- **D7 Job runner**: leases last 60 seconds and are renewed by heartbeat; a crashed worker's lease expires and the job is retried; retry delay is 5 s, 30 s, 2 min (BR-13.3).
- **D8 Graceful shutdown**: stop accepting new work, wait up to 30 seconds for in-flight requests and jobs, then close pools and exit; readiness turns false immediately.
- **D9 WebSocket**: authenticated by the session cookie plus an Origin check; ping every 25 seconds; at most 10 connections per user; outbound queues are bounded and a client that cannot keep up is disconnected and must resync.
- **D10 Metrics and traces**: a central registry exposes the OB-1 metrics; trace context (trace and span IDs) is stored in each outbox event and job so a trace follows a message from `api` to `worker`.
- **D11 Database access**: separate pools for `api` and `worker`, each with a maximum size, acquire timeout (2 s) and statement timeout (5 s for requests, 30 s for jobs); transactions are short; the audit insert happens inside the same transaction as the change.
- **D12 Log redaction**: a logging layer removes any field whose name matches password, token, secret, code, cookie or authorization before output; request and response bodies are never logged.
- **D13 Migrations**: run only by `tandem migrate` under the migration role (the app role cannot change schema); the Compose file runs it as a one-shot service before `api` and `worker`; services check the schema version at start (AV-3).

## Questions

## Question 1
Where should the per-source sign-in and request rate limits be counted (the per-account delay and lock always live in PostgreSQL)?

A) In memory inside each `api` process: no extra database writes and very fast; with several `api` instances each counts separately, which is acceptable for one team on one host (SC-1)

B) In PostgreSQL: one consistent count across instances, at the cost of a database write per limited request

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 2
Should the server cache session lookups between requests?

A) No cache: every request checks the session and the user's current role and status in PostgreSQL, so demotion, deactivation and sign-out take effect immediately (BR-9.1)

B) A very short in-memory cache (5 seconds): fewer database reads, but a revoked or demoted user keeps access for up to 5 seconds

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3
How should the metrics endpoint be protected (SE-10)?

A) A separate internal port that the reference Compose file does not publish; reachable only from the container network or by the operator's monitoring

B) The main port, protected by a bearer token the operator sets

C) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 4
Argon2id is memory-hungry, so a flood of sign-in attempts could exhaust the host. What should happen when too many hashes are already running?

A) Cap concurrent hashes at the number of CPU cores (with a memory ceiling); extra attempts wait briefly (up to 2 seconds), then get a generic "busy, try again" answer, so the host stays healthy

B) No cap: every attempt is hashed immediately and the host's limits decide

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
When a browser's WebSocket drops and reconnects, how should it catch up on what it missed (skeleton in U1, deepened in U3)?

A) The client sends the ID of the last event it saw and the server replays the missed events from the outbox; if the gap is too old, the client reloads the view

B) The client always reloads the whole view after reconnecting: simpler, more data transferred

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
How long should ended sessions (sign-ins), used invitation and reset tokens, and expired tokens be kept before a cleanup job deletes them? (The audit log is never pruned.)

A) 30 days

B) 90 days

C) Indefinitely: no cleanup job

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
How should operators configure Tandem?

A) Environment variables, plus an optional TOML file named by one variable; secrets only from files or environment variables; the environment wins over the file

B) Environment variables only (twelve-factor style)

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
A browser may retry a message post after a network blip, which could create a duplicate message. How should that be prevented?

A) The client attaches a random message ID; the server treats a repeat of the same ID within the same session as the same message and returns the original

B) No duplicate protection in U1; revisit in U3

C) Other (please describe after [Answer]: tag below)

[Answer]: B

---

## Logical components to be designed
Request pipeline and middleware chain; typed authorisation extractors; session store and CSRF guard; account guard and source limiter; password hasher pool; breached-password checker (offline filter plus online client with circuit breaker); vault; audit writer; outbox relay; event hub and WebSocket gateway; job runner; metrics registry; health checker; configuration loader; telemetry (logs and traces); cleanup job; database access layer (pools, transactions, roles).

## Execution Checklist (generation, after answers are approved)
- [x] 1. Design each pattern area (resilience, scalability, performance, security, observability) with the concrete mechanism, parameters and failure behaviour; write `nfr-design-patterns.md`
- [x] 2. Define each logical component: responsibility, interfaces, dependencies, configuration, metrics; write `logical-components.md`
- [x] 3. Map every NFR requirement (SC, PF, AV, RL, SE, OB, MT) to the pattern and component that satisfies it (traceability check)
- [x] 4. Re-evaluate Security Baseline and applicable Resiliency rules against the design and record compliance
- [x] 5. Update state, log the approval prompt, present the completion message

## Extension compliance (planning)
- Security Baseline: enforced; design evidence recorded in checklist step 4
- Resiliency Baseline: enforced for product scope; operator-owned decisions unchanged
- Property-Based Testing: disabled

---

## Consequences of the answers
| Answer | Consequence |
|---|---|
| Q1 A | Per-source limits are in memory per `api` process; each instance counts separately (documented limit of the single-host target) |
| Q2 A | No session cache; every request reads session, role and status from PostgreSQL. To keep this inside the 200 ms budget, `last_seen_at` is written at most once a minute |
| Q3 B | `/metrics` is on the main port behind a bearer token; if no token is configured the endpoint is disabled (deny by default). Operators should also block `/metrics` at their proxy |
| Q4 A | Concurrent Argon2 hashes are capped by cores and a memory budget; overflow waits up to 2 seconds, then returns a generic "busy" answer |
| Q5 A | Outbox events are kept 24 hours so a reconnecting client can replay from its last event ID; older gaps trigger a reload |
| Q6 A | A daily cleanup deletes ended sign-ins, used or expired tokens and finished jobs older than 30 days. It never touches the audit log |
| Q7 A | Environment variables plus an optional TOML file; environment wins; secrets only from files or environment |
| Q8 B | No duplicate-post protection in U1; revisit in U3 |

## Additional design assumptions made while generating
- **D14**: Published outbox rows are kept 24 hours (for replay) and then removed by the cleanup job; finished jobs are kept 30 days.
- **D15**: `TANDEM_PUBLIC_URL` is required; it builds invitation and reset links and is the allowed Origin for state-changing requests and WebSockets.
- **D16**: The client IP is the direct peer unless it falls inside `TANDEM_TRUSTED_PROXIES`, in which case the `X-Forwarded-For` value is used (needed for rate limiting and audit behind a reverse proxy).
