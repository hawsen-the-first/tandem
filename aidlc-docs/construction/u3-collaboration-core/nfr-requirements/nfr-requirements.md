# U3 Collaboration Core: NFR Requirements

**Unit**: U3 Collaboration core.
**Sources**: the U3 functional design (BR-22 to BR-34), the plan answers (Q1 to Q9) and assumptions (N1 to N10) in
`construction/plans/u3-collaboration-core-nfr-requirements-plan.md`, and the Security and Resiliency Baseline rules.
**Scope**: additions only. Everything in U1's and U2's NFR requirements still applies.
IDs continue from U1 and U2: **SC** scalability, **PF** performance, **AV** availability, **RL** reliability,
**SE** security, **OB** observability, **MT** maintainability and testing, **US** usability.

## 1. Scalability
| ID | Requirement |
|---|---|
| SC-7 | **Design load (N1).** 50 active users, 200 sessions, 30 members in the largest session, 200 WebSocket connections per `api` process, and up to 50,000 messages in one session's history |
| SC-8 | Every list (messages, replies, threads, sessions, notifications, members) is paginated by sequence or by keyset. No request loads a whole history |
| SC-9 | Live events fan out through the U1 outbox and PostgreSQL `NOTIFY`, behind an `EventBus` port. Several `api` processes work behind a proxy without a broker, and a broker adapter can replace the port later (NFR-3.2) |
| SC-10 | Presence is kept in memory per process and shared through the same `NOTIFY` path (Q8). Nothing is written per heartbeat |
| SC-11 | **Uploads go through a `BlobStore` port with two adapters:** a local directory (default) and S3-compatible storage (Q1). The local adapter supports several `api` processes only when they share the volume, and the self-hosting guide says so |

## 2. Performance
| ID | Requirement |
|---|---|
| PF-10 | A message posted by one member appears for every other connected reader in under **500 ms at the 95th percentile on a LAN** at design load, measured from the post request to rendering (extends PF-3) |
| PF-11 | Opening a channel shows its newest 50 messages in under **300 ms at the 95th percentile**. The overview and the thread list meet PF-1 (200 ms) |
| PF-12 | The server streams uploads: memory per upload is bounded (no whole-file buffering) whatever the file size, and encryption and hashing run on the stream |
| PF-13 | `NOTIFY` payloads carry only the session id and sequence (under 8,000 bytes, a PostgreSQL limit). Gateways read events by sequence |
| PF-14 | Search for the `#` picker and the session list returns in under 200 ms at the 95th percentile, using trigram indexes on titles and file names (Q7) |

## 3. Availability
| ID | Requirement |
|---|---|
| AV-8 | `/readyz` also checks the blob store: the local directory is writable, or the S3 bucket is reachable with a head request under 2 s. An unavailable store is reported as `degraded` and does **not** fail readiness: uploads and downloads answer 503 while the rest of Tandem keeps working (revised in NFR Design, Q5) |
| AV-9 | Browsers reconnect after an `api` restart with backoff (1 s up to 30 s, with jitter). They resume from their last sequence, or reload if it falls outside the replay window (N5). No message is lost, because writes go over HTTP |
| AV-10 | Graceful shutdown closes WebSockets with a "going away" code, so that clients reconnect to another process at once |

## 4. Reliability
| ID | Requirement |
|---|---|
| RL-12 | **Uploads are written in a fixed order:** encrypt and stream to a temporary object, insert the row, commit, then move into place. Any failure removes the temporary object, and nothing partial is visible (BR-26.3) |
| RL-13 | A daily sweep removes orphans in both directions: a stored object without a row, and a committed row without an object. Each is logged with ids only, and the second kind also raises `blob_orphans_total` |
| RL-14 | Workspace removal, pending-upload discard, retention deletion, reminder checks, event pruning and notification pruning are idempotent jobs. They use the U1 retry policy (RL-5) |
| RL-15 | **Event delivery** is at least once. Clients ignore duplicates by sequence. Missed events are replayed from the store for 24 hours or 10,000 events per session, whichever is larger (N5) |
| RL-16 | Approve, revoke and status changes lock the thread row, so concurrent actions produce exactly one outcome (BR-27). Posts take their sequence number inside the transaction, so the order seen equals commit order |
| RL-17 | Every blob store call has a timeout: 10 s per S3 request, plus a streaming deadline proportional to size. The S3 adapter retries idempotent calls twice with backoff, and never retries a partial upload |
| RL-18 | **Upload retention (Q4, N9).** By default, uploads are deleted 180 days after the session ends; the instance setting ranges from 1 day to keep forever. The deletion job marks rows `expired` and deletes the objects, and a failure is retried, never ignored |

## 5. Security (Security Baseline)
| ID | Requirement | Rule |
|---|---|---|
| SE-27 | **Access control.** Every request, subscription and event delivery goes through the one access check (BR-23, logic model §1). Private sessions answer "not found" to everyone outside them, and ids are checked within the session | SECURITY-08 |
| SE-28 | **Events carry ids and metadata only (Q5).** Content leaves the server only through authorised HTTP reads. The gateway re-checks access on membership and visibility changes, and closes subscriptions that have lost it | SECURITY-08, SECURITY-11 |
| SE-29 | **Uploads are encrypted by Tandem (Q2).** Each file gets its own AES-256-GCM data key, sealed by the vault under the `upload` purpose and bound to the upload id. Both stores hold only ciphertext. The content hash is of the plaintext and is stored only in the database | SECURITY-01 |
| SE-30 | **Serving uploads.** Responses are `Content-Disposition: attachment` with `X-Content-Type-Options: nosniff` and `Content-Security-Policy: sandbox`. Matching PNG, JPEG, GIF and WebP are the only inline types. File names are encoded as RFC 6266 `filename*` (BR-28.4) | SECURITY-04 |
| SE-31 | **Input limits are enforced before parsing.** Bodies carry 20,000 characters. Uploads are limited to 25 MiB (the request is aborted at the limit), 10 attachments and 2 GiB per session. WebSocket frames are at most 16 KiB. Unknown fields are refused | SECURITY-05 |
| SE-32 | **Rate limits per user (N3):** 30 messages per 10 s, 20 uploads per minute, 10 thread opens per minute, 30 membership changes per minute. A refusal is a 429 with `Retry-After` | SECURITY-11 |
| SE-33 | **WebSocket limits (N4).** At most 10 connections per user. The `Origin` must match the instance, the sign-in cookie is required, and CSRF does not apply because clients send no writes on the socket. The server pings every 30 s and drops after 75 s silent | SECURITY-08, SECURITY-11 |
| SE-34 | **Safe rendering.** Markdown is rendered to React elements through an allow-list sanitiser (Q6). Nothing is inserted as an HTML string, the CSP stays unchanged (no `unsafe-inline`), and Trusted Types are enforced where the browser supports them | SECURITY-04, SECURITY-05 |
| SE-35 | **No content in telemetry.** Message bodies, summaries, file names and file contents never appear in logs, metrics labels, traces or audit entries. Redaction tests extend to the new types | SECURITY-03 |
| SE-36 | **S3 adapter.** Its credentials come from a file or the environment, never the database or the browser. Its endpoint follows the BR-19 outbound URL rules, with private networks allowed. TLS is verified, and `http://` is accepted only for private hosts | SECURITY-01, SECURITY-12 |
| SE-37 | **No malware scanning in the MVP (Q3).** The risk is accepted and documented. Uploads are never run, rendered as HTML or given to agents other than as text. A scanner hook is backlog | SECURITY-11 |

### 5.1 Misuse cases considered (SECURITY-11)
| Misuse | Mitigation |
|---|---|
| Guessing ids to read a private session | "Not found" for everything, opaque ids, the one access check (SE-27) |
| Staying subscribed after removal | Subscriptions are re-checked on member and visibility events, and closed (SE-28) |
| Stored XSS through messages, titles, file names or summaries | An allow-list renderer, text-only titles, a strict CSP and Trusted Types (SE-34) |
| Uploading HTML or SVG to run script on the origin | Always served as an attachment, with `nosniff` and a sandbox CSP (SE-30) |
| Filling the disk | Per-file, per-message and per-session limits; rate limits; pending upload discard; retention (SE-31, SE-32, RL-18) |
| Flooding a session or the socket | Per-user write limits, frame limits and connection caps (SE-32, SE-33) |
| Reading uploads from a stolen volume or bucket | Envelope encryption with the vault (SE-29) |
| Mention spam to people outside the session | Write rate limits. Only members can mention, and only in open sessions can a non-member be mentioned (BR-25.7) |
| Prompt injection through approved summaries (U4) | Agents are bound by policy, not by text. The summary's precedence covers its content, never its permissions (US-13.2) |

## 6. Observability
| ID | Requirement |
|---|---|
| OB-7 | **New metrics:** `ws_connections`, `ws_events_sent_total`, `realtime_delivery_seconds` (commit to send), `messages_posted_total`, `uploads_bytes_total`, `upload_rejected_total{reason}`, `reminders_sent_total`, `workspace_removal_failures_total`, `blob_orphans_total`, `uploads_expired_total`. Labels never carry ids or content |
| OB-8 | The alert rule templates add repeated workspace removal failures, any `blob_orphans_total` increase, and `realtime_delivery_seconds` p95 above 500 ms for 10 minutes |

## 7. Maintainability and testing
| ID | Requirement |
|---|---|
| MT-13 | **Access matrix tests.** Every endpoint and event kind is tested as an open reader, private non-member, member, owner, admin who is not a member, and removed member (BR-23) |
| MT-14 | **Hostile input tests.** A corpus runs against `MarkdownView`: scripts, event attributes, `javascript:` and `data:` links, raw HTML, SVG, deeply nested lists, long lines, Unicode direction tricks. The serving headers are checked for each sniffed type |
| MT-15 | **Concurrency tests.** They cover double approve, approve against revoke, simultaneous posts keeping their order, and an upload racing a session ending |
| MT-16 | **Delivery tests.** A Playwright test with two browser contexts checks PF-10 in CI. The smoke performance test records the delivery histogram and fails over a 500 ms p95 (Q9) |
| MT-17 | **Blob store tests.** Both adapters pass one contract test suite: the local one against a temporary directory, the S3 one against a MinIO container (N10) |
| MT-18 | Every BR-22 to BR-34 rule maps to at least one test, in a U3 table like U2's (MT-11) |

## 8. Usability and accessibility
| ID | Requirement |
|---|---|
| US-6 | **The message list is a `log` region.** New messages are announced politely as a count and throttled, never read out in full. Focus never moves when messages arrive |
| US-7 | Every action is reachable by keyboard, including reply, menus, pickers and dialogs. Pickers follow the ARIA combobox pattern. Status is shown in text, not colour alone (thread status, presence) |
| US-8 | The layout works at 360 px. The sidebar becomes a drawer, and the composer stays visible with the on-screen keyboard |

## 9. Compliance summary

### Security Baseline (U3 additions)
| Rule | Status | Where |
|---|---|---|
| SECURITY-01 Encryption | Compliant | SE-29, SE-36; TLS to PostgreSQL from U1 |
| SECURITY-03 Logging | Compliant | SE-35 |
| SECURITY-04 Headers and CSP | Compliant | SE-30, SE-34 |
| SECURITY-05 Input validation | Compliant | SE-31 |
| SECURITY-08 Access control | Compliant | SE-27, SE-28, SE-33, MT-13 |
| SECURITY-11 Secure design | Compliant | SE-32, SE-33, SE-37, misuse table |
| SECURITY-12 Credentials | Compliant | SE-36 |
| SECURITY-15 Fail-safe | Compliant | RL-12, RL-16; "not found" by default |
| SECURITY-02, 06, 07, 09, 10, 13, 14 | N/A for U3 additions | Covered by U1 and U8; SECURITY-13 upload integrity is covered by the hash and GCM tag |

### Resiliency Baseline (U3 additions, product scope)
| Rule | Status | Where |
|---|---|---|
| RESILIENCY-05, 07 Monitoring | Compliant | OB-7, OB-8 |
| RESILIENCY-06 Health checks | Compliant | AV-8 |
| RESILIENCY-10 Dependency isolation | Compliant | RL-17 (timeouts and retries for the blob store); `NOTIFY` failure degrades to reload (AV-9) |
| RESILIENCY-09 Capacity | Compliant | SC-7 to SC-11 |
| RESILIENCY-01 to 04, 08 | N/A for U3 | Operator decisions (NFR-5.4) and U8 delivery |
