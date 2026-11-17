# NFR Requirements Plan: U3 Collaboration core

**Stage**: CONSTRUCTION - NFR Requirements, **Full** (unit `u3-collaboration-core`). C7, the component for safe
rendering and uploads, is security-critical, and the unit-of-work table lists real-time latency and input limits.
**Inputs**: U3 functional design (BR-22 to BR-34, A14 to A16), and the U1 NFR requirements and technology decisions
(the baseline U3 inherits), with U2's additions.
**Scope**: only what U3 adds or changes. Everything in U1's and U2's `nfr-requirements.md` still applies: security
headers and CSP, rate limits, audit, redaction, TLS, health checks, observability and accessibility. This file does
not repeat it.

## Steps
- [x] 1. Analyse the functional design and the U1 real-time path (outbox, PostgreSQL `NOTIFY`, WebSocket hub)
- [x] 2. Answer the questions below
- [x] 3. Review the answers; follow up if needed
- [x] 4. Generate `nfr-requirements.md` and `tech-stack-decisions.md`
- [x] 5. Approval (approved; pull request)

## Decisions from the answers
| # | Decision |
|---|---|
| Q1 | A `BlobStore` port with two adapters in U3: a **local directory** (`TANDEM_DATA_DIR`, the default) and **S3-compatible** object storage, selected by configuration from day one |
| Q2 | Tandem encrypts every upload itself: envelope encryption, a data key per file sealed by the vault under an `upload` purpose. This applies to both stores |
| Q3 | No malware scanning in the MVP. An optional scanner hook (ClamAV) is backlog |
| Q4 | Uploads are kept for a configurable time after a session ends, then deleted. Messages and approvals are kept |
| Q5 | Events carry ids and metadata only; clients fetch content over HTTP |
| Q6 | `react-markdown`, `remark-gfm` and `rehype-sanitize` with our schema; `highlight.js` with languages loaded on demand |
| Q7 | Search covers titles only (trigram index); full-text search is backlog |
| Q8 | Presence is held in memory per process and shared through `NOTIFY` |
| Q9 | Playwright with two browser contexts in CI, plus the delivery histogram in the smoke performance test; a p95 over 500 ms fails the build |

**Assumptions added from the answers (confirm at approval)**:
- **N9 (Q4).** Upload retention is an instance setting, **180 days** after `ended_at` by default, from 1 day to "keep forever". Archiving does not pause the clock. A daily job deletes the files and marks the uploads `expired`, and messages show "File removed after the retention period". The owner and members are not warned in advance in the MVP.
- **N10 (Q1).** The S3 adapter is set with an endpoint, bucket, region, a path-style option and credentials read from a file or environment. Its endpoint follows the outbound URL rules (BR-19), with private networks allowed so that a local MinIO works. CI tests it against a MinIO container. Moving files between stores is a documented `tandem` command in U8, not part of U3.

## Already settled (not asked)
- **Fan-out across processes.** U1's transactional outbox already publishes through PostgreSQL `NOTIFY`, and each
  `api` process relays to its WebSocket clients. Several `api` processes therefore work behind a proxy without a
  message broker (NFR-3.2). U3 keeps that path and moves it behind an `EventBus` port, so that a broker can replace
  it later.
- **Ordering.** The per-session sequence number is taken inside the writing transaction (BR-29).

## Working assumptions (not questions; confirm or correct at approval)
- **N1, scale.** Design load is 50 active users, 200 sessions, 30 members in the largest session and 200 concurrent
  WebSocket connections per `api` process. A session's history can reach 50,000 messages, so every list is
  paginated by sequence and nothing loads a whole history.
- **N2, performance.**
  - **PF-3 is extended.** A message from one member appears for other readers in under **500 ms at the 95th
    percentile on a LAN**, at design load.
  - **Reads.** Opening a channel shows its newest 50 messages in under **300 ms at the 95th percentile**.
  - **Uploads.** These are bounded by bandwidth, with no extra server cost beyond streaming and hashing.
- **N3, rate limits for writes.** These are per user, on top of U1's per-source limits:
  - 30 messages per 10 seconds;
  - 20 uploads per minute;
  - 10 thread opens per minute;
  - 30 membership changes per minute.

  Each refusal is a 429 with `Retry-After`, as in U1.
- **N4, WebSocket limits.**
  - Each user has at most 10 open connections, the oldest closed first.
  - Client frames are at most 16 KiB. Clients only subscribe, acknowledge and send presence pings; every write goes
    over HTTP.
  - The server pings every 30 s and drops a connection silent for 75 s.
- **N5, replay window.** Events are kept for 24 hours or 10,000 per session, whichever is larger, then pruned. A
  client further behind reloads (BR-29.2).
- **N6, reliability.**
  - An upload is written to temporary storage and moved into place only after its database row commits. A sweep
    removes orphaned files, whether a file without a row or a row without a file, and logs each one.
  - Workspace removal and reminder jobs are idempotent and use the U1 job retry policy.
- **N7, observability.** New metrics:
  - `ws_connections` (gauge);
  - `ws_events_sent_total`;
  - `realtime_delivery_seconds` (histogram, commit to send);
  - `messages_posted_total`;
  - `uploads_bytes_total`;
  - `upload_rejected_total` by reason;
  - `reminders_sent_total`;
  - `workspace_removal_failures_total`.

  An alert rule template fires on repeated workspace removal failures. No message text or file name appears in
  logs, metrics or traces.
- **N8, testing.**
  - **Hostile input.** A corpus is run against `MarkdownView` and against the file-serving headers.
  - **Two browsers.** A Playwright test checks the 500 ms delivery target.
  - **Concurrency.** Tests cover double approve, approve against revoke, and simultaneous posts that must keep their
    order.
  - **Access.** A matrix test covers each endpoint and event as an open reader, private non-member, member, owner and
    admin (BR-23).

---

## Questions

Fill in the letter after each `[Answer]:` tag, and add notes if you like.

## Question 1
Where should uploaded files be stored?

A) **A local directory on a volume.** Its path is set by `TANDEM_DATA_DIR`, and files are named by content hash and
session. It is simple and fast, and backed up with the volume. It works with several `api` processes only if they
share the volume. An S3-compatible adapter can come later behind the same port.

B) **In PostgreSQL**, as large objects or `bytea`. One store to back up and no shared volume, but the database grows
by the full size of the uploads (up to 2 GiB per session).

C) **S3-compatible object storage from the start**, such as MinIO in Compose. It scales out, but every self-hoster
gets another service to run.

X) Other (please describe after [Answer]: tag below)

[Answer]: A by default, C supported and configurable from day one.

## Question 2
Should uploads be encrypted by Tandem itself (SECURITY-01)?

A) **Yes, envelope encryption with the vault.** Each file has its own data key, sealed under an `upload` purpose. A
stolen volume or backup reveals nothing without the vault key. The cost is a small amount of CPU and no
deduplication across sessions.

B) **No.** Rely on disk or volume encryption, which is documented as an operator duty in the self-hosting guide (U8).
Files are still only served to readers.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3
Should uploads be scanned for malware?

A) **Not in the MVP.** Uploads are always served as downloads with `nosniff` (BR-28.4), and agents only read text.
An optional scanner hook is post-MVP.

B) **An optional ClamAV hook now.** It is off by default; when it is configured, files stay `pending` until scanned
clean, and infected files are refused.

X) Other (please describe after [Answer]: tag below)

[Answer]: A with B as a later feature

## Question 4
What happens to uploads and message content after a session ends?

A) **Kept for as long as the session exists.** Ended and archived sessions stay fully readable (US-11.6). Deleting
a session (post-MVP) will delete them.

B) **Uploads are deleted when the session ends**, and the messages show "file removed". This saves space, but the
decision record loses its attachments.

C) **Uploads are kept for a configurable time after ending** (for example 180 days), then deleted.

X) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 5
What should a real-time event carry (A16)?

A) **Ids and metadata only.** The client fetches the content over HTTP, so access is checked in one place. Each new
message costs one extra request, which is still well within 500 ms on a LAN.

B) **The full message for authorised receivers.** The gateway re-checks access (BR-23.7) and includes the rendered
data, such as the body and author, in `message.created` and `message.edited`. It is faster and has fewer requests,
but there are two places where content leaves the server.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
Which Markdown renderer and code highlighter should the web client use (BR-28)?

A) **`react-markdown` with `remark-gfm` and `rehype-sanitize`** using our allow-list schema, which renders to React
elements with no HTML strings. Code is highlighted by **`highlight.js`** with a small set of common languages, each
loaded on demand.

B) **`markdown-it`** to HTML, then **DOMPurify**, inserted as HTML. It is fast and mature, but relies on HTML string
insertion, which is harder to keep within the strict CSP and Trusted Types.

C) **Server-side rendering.** The Rust side renders with `pulldown-cmark` and `ammonia`, and stores the sanitised
HTML alongside the source. The client inserts it as HTML.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
What should the reference picker (`#`) and the session list search across?

A) **Titles only in the MVP:** thread titles, upload file names and, for messages, their first line. A PostgreSQL
trigram index is enough. Full-text search of message bodies is post-MVP.

B) **Full-text search of message bodies as well**, with PostgreSQL full-text search in English and a "Search this
session" box.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
How is "presence" kept when there are several `api` processes?

A) **In memory per process, shared through the same `NOTIFY` path.** Each process announces join, leave and
heartbeat changes for its connections. Presence is lost on restart and rebuilds within one heartbeat, which is fine
for a status dot.

B) **Stored in PostgreSQL** with a heartbeat row per connection. It survives restarts, but means a write every
heartbeat for every connection.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 9
How should the 500 ms delivery target (N2) be verified?

A) **A Playwright test in CI with two browser contexts** on the same machine, plus a measured `realtime_delivery_seconds`
histogram during the smoke performance test. It fails the build if p95 is over 500 ms.

B) **As A, plus a manual check on a real LAN** with two devices before U3 closes, recorded in the U3 summary.

X) Other (please describe after [Answer]: tag below)

[Answer]: A
