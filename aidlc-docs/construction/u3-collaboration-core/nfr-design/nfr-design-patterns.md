# U3 Collaboration Core: NFR Design Patterns

How the U3 NFR requirements are met. Decisions referenced: Q1 to Q6 and D1 to D5 in
`construction/plans/u3-collaboration-core-nfr-design-plan.md`. U1 and U2 patterns still apply: the request pipeline,
outbox, job queue, vault, audit, circuit breaker and outbound client.

## 1. One access decision (SE-27, SE-28, BR-23; Q3)
- **One function.** A single `SessionAccess` component (LC-36) answers `allowed`, `not_found`, `must_join` or
  `ended` for a (user, session, need). It is used by:
  - every HTTP handler, through an Axum extractor that takes the session from the path;
  - the gateway on subscribe and before each delivery;
  - the notification writer (BR-33.2).
- **Lookups within the session.** Child ids (thread, message, upload, reference target) are looked up with the
  session id in the `WHERE` clause, never by id alone. A child from another session is therefore indistinguishable
  from a missing one.
- **Gateway cache (Q3 A).** Each `api` process caches (user, session) results for its subscribers.
  - Entries are filled on subscribe, and expire after 60 s as a backstop.
  - Events that can change access clear every entry for their session at once: `member.added`, `member.removed`,
    `session.updated` (visibility or owner), `session.ended` and `session.archived`.
  - After clearing, each subscriber is re-checked against the database before the event is delivered. A subscriber
    who has lost access gets a "subscription ended" frame and is removed.
- **Unknown and forbidden look alike.** "Not found" responses have the same body, status and timing class whether
  the session is missing or private (BR-9.2).

## 2. Session events as the outbox (SC-9, RL-15, PF-13; Q1, Q2, D1)
- **Written in the same transaction (Q2 A).** Every change a reader can see writes one `session_events` row
  (session id, sequence, kind, ids, safe metadata) in the transaction that makes the change. That row is both the
  durable outbox record and the replay log. The U1 outbox remains for jobs and events that are not session-scoped,
  such as user notifications and audit fan-out.
- **Sequence allocation (D1).** `UPDATE sessions SET next_sequence = next_sequence + 1 … RETURNING` inside the
  transaction. The session row lock serialises writers of one session only. Messages and events share the counter,
  so message order equals event order.
- **Notify.** After commit, `pg_notify('tandem_session_events', '{session}:{sequence}')` is sent, a few dozen bytes
  that are always under the 8,000-byte limit. A lost notification is harmless, because the next one, or a catch-up,
  reads by sequence.
- **Delivery.** Each `api` process's gateway (LC-37) listens on the channel. For each session with subscribers, it
  reads `session_events` after its last delivered sequence, in order, then fans out to subscribers.
- **Listener loss (Q1 A).**
  - On a dropped listener connection, the gateway keeps each session's last delivered sequence and reconnects with
    backoff (1 s up to 30 s).
  - It then reads the gap from `session_events` for every subscribed session, and delivers it.
  - Sessions whose gap is older than the replay window send "reload" to their subscribers.
  - While disconnected, a poll every 5 s per subscribed session keeps delivery within seconds. It is cheap at design
    load and stops once the listener is back.
- **Replay for clients.** A browser subscribes with its last sequence and gets the stored events after it, or
  "reload" if they have been pruned. The window is 24 hours or 10,000 events per session, whichever is larger (N5),
  pruned by a worker job (D5).

## 3. Thin events, batched reads (SE-28, PF-10; T20, Q4)
- Events carry ids, kinds and safe metadata only: author id, channel id, thread status, counts and sequence. They
  never carry bodies, titles or file names.
- **Batched fetch (Q4 A).** The client collects `message.*` events for up to 50 ms, then requests
  `GET /api/sessions/{id}/channels/{cid}/messages?after={seq}` once for each channel it is showing. A burst of
  messages costs one request per reader.
- **Channels not on screen.** Events for channels not on screen update counters only (unread and reply counts),
  with no fetch.
- **Latency budget for PF-10, on a LAN at the 95th percentile:**

  | Step | Budget |
  |---|---|
  | Post commit | 60 ms |
  | `NOTIFY` and read events | 20 ms |
  | Fan-out | 10 ms |
  | Client batch wait | 50 ms |
  | Fetch | 80 ms |
  | Render | 30 ms |
  | **Total** | **About 250 ms**, within 500 ms |

## 4. WebSocket gateway hygiene (SE-33, N4, AV-10)
- **Connecting.** The cookie is required, and `Origin` must match the instance. Subscriptions are per session, and a
  connection may hold several. The per-user channel carries notifications (D3).
- **Limits.**
  - At most 10 connections per user, with the oldest closed first.
  - Client frames are at most 16 KiB. Clients may send only `subscribe`, `unsubscribe`, `ack` and `presence`;
    anything else closes the connection.
- **Liveness.** The server pings every 30 s and closes a connection silent for 75 s.
- **Send queues.** Each connection has a bounded send queue of 256. When it is full, the subscription gets "reload"
  and its queue is emptied.
- **Shutdown.** Connections close with "going away" (1001), and clients reconnect with jittered backoff and resume
  by sequence (AV-9).

## 5. Presence without writes (SC-10, BR-32.3; Q8, T23)
- **Local state.** Each process keeps (session, user) to connection count and last input time.
- **Announcing.** On a transition between absent, online and away, the process sends a `presence` notification on
  a separate channel, carrying (session, user, state, process id). Every process merges these into a per-session
  view: a user is online if any process says online.
- **Process restarts.** Every process repeats its local states every 60 s. A process that has been silent for 150 s
  has its entries dropped, so a crashed process's users fade to absent.
- **Delivery and privacy.** Presence events go to members only. They are never stored or audited, and never logged
  with user names.

## 6. Uploads: stream, encrypt, commit, move (PF-12, RL-12, RL-13, SE-29, SE-30; T16 to T19)
- **Upload pipeline.** The request body passes, in order, through:
  1. a **size guard**, which aborts at 25 MiB plus 1 byte;
  2. a **SHA-256 hasher** over the plaintext;
  3. a **type sniffer**, on the first 4 KiB;
  4. **chunked AES-256-GCM** in 64 KiB chunks, as a STREAM construction with a version byte and a final-chunk flag;
  5. a **temporary object** in the blob store.
- **Data keys.** A fresh data key is generated per upload and sealed by the vault (LC-14) under the `upload` purpose,
  with the upload id as associated data.
- **Order of writes (RL-12).** The upload is written in this order:
  1. stream to `tmp/{upload id}`;
  2. insert the `uploads` row (`pending`, hash, size, types, sealed key);
  3. commit;
  4. move `tmp/{upload id}` to `uploads/{session}/{upload id}`. The local adapter renames; S3 copies, then deletes.

  Any failure before the commit deletes the temporary object. A failure after the commit leaves a row whose object
  is still in `tmp/`. The orphan sweep finishes the move or flags it.
- **Downloads.** Access check, then stream, decrypt and verify each chunk tag, then the response. A tag failure
  aborts the response and logs `blob_integrity_failure` with ids only. The SE-30 headers are set before the first
  byte.
- **Deduplication.** None, because every file has its own key. The hash serves integrity and display, not storage.

## 7. Blob store failure isolation (RL-17, AV-8 revised; Q5)
- **Circuit breaker (Q5 A).** All blob store calls go through a circuit breaker (U1 LC-23).
  - It opens after 5 failures in 30 s and probes again after 30 s.
  - While it is open, uploads answer 503 "File storage is unavailable. Try again shortly", and downloads answer 503
    with the same text.
  - Everything else continues.
- **Degraded, not unready.** `/readyz` reports `blob_store: degraded` in its body but keeps a 200 status, so a proxy
  keeps routing to the process. NFR AV-8 is revised to match.
- **Alerting.** `blob_store_available` (a gauge) and the circuit breaker state are exported. The alert template
  fires when the store is unavailable for more than 2 minutes.
- **Timeouts and retries.** 10 s per S3 request, and a streaming deadline of 30 s plus 1 s per MiB. Idempotent calls
  (head, get, delete) retry twice with backoff. Uploads never retry a partial body; the user retries.
- **Full disk.** The local adapter treats "no space" as a failure that counts toward the breaker. It also refuses
  uploads early when free space falls under 1 GiB plus the upload size, with the same message.

## 8. Rate limits and quotas (SE-31, SE-32; D2)
- **Rate limits.** Per-user limits sit beside the U1 per-source limiter, in process and keyed by user id (D2). Each
  route declares its bucket: `message`, `upload`, `thread` or `membership`.
- **Session quota.** The 2 GiB quota is checked in the upload transaction with a `SELECT … FOR UPDATE` on the
  session's running total, `uploaded_bytes`. Two simultaneous uploads therefore cannot both slip under it. Discarding
  or expiring an upload subtracts its size.
- **Limits before reading.** Body and field limits are enforced by the request pipeline before handlers read
  anything (U1 pattern), and the JSON limit for message bodies is 128 KiB.

## 9. Safe rendering (SE-34, BR-28; Q6, T21)
- **One schema (Q6 A).** `web/src/markdown/schema.ts` holds the `rehype-sanitize` schema: elements, attributes,
  protocols and the upload-image rule. `MarkdownView` and the hostile input tests both import it.
- **Upload images.** Image sources are rewritten to API download URLs only when they parse as an upload id of the
  current session. Anything else becomes a link.
- **Lint guards.** ESLint rules forbid:
  - importing `rehype-raw`;
  - using `dangerouslySetInnerHTML`;
  - setting `innerHTML`, `outerHTML` or `insertAdjacentHTML`;
  - importing the sanitiser schema anywhere except `MarkdownView` and its tests.
- **Trusted Types.** The CSP adds `require-trusted-types-for 'script'`, with no policies defined. Any HTML sink
  therefore throws in supporting browsers, which also catches mistakes in dependencies.
- **Highlighting.** `highlight.js` runs on the text of code blocks and returns tokens. They are rendered as spans
  with classes, so no HTML string from the highlighter is ever inserted. Languages load on demand from same-origin
  chunks.
- **Plain text elsewhere.** Titles, file names and author names are always rendered as text nodes.

## 10. Reminders and sweeps (BR-31, RL-14, RL-18; D4, D5)
- **Reminders (D4).** Each thread has a `remind_at` column, recomputed on activity, status change, or a change to
  the session or instance period. It is null when reminders are off, or the thread is approved or the session not
  active.
  - It is indexed as `WHERE remind_at IS NOT NULL`.
  - The worker loop runs every 5 minutes and takes due rows with `FOR UPDATE SKIP LOCKED`. It writes notifications,
    sets `reminded_at`, and clears `remind_at` in one transaction.
- **Business day.** The calculation is a pure function in `core`, taking the start time, the instance time zone and
  the period. It has table tests around weekends and daylight-saving changes.
- **Sweeps (D5).** Each sweep is a job of its own:
  - `discard_pending_uploads` (hourly);
  - `prune_session_events` (daily);
  - `prune_notifications` (daily);
  - `expire_uploads` (daily);
  - `sweep_blob_orphans` (daily);
  - `remove_workspace` (on demand).

  Each processes batches of 500, commits per batch, stops after a 60 s budget, and reschedules itself if there is
  more to do.

## 11. Concurrency rules (RL-16, MT-15)
| Operation | Guard |
|---|---|
| Post, edit, delete | Session row for the sequence; message row for edit and delete |
| Approve, revoke | Thread row `FOR UPDATE`, then a status check |
| Add, remove, transfer | Session row `FOR UPDATE`. A unique (session, user) and a partial unique "one owner per session" index |
| End, archive | Session row `FOR UPDATE`, then a state check. Writes check `state = active` inside their transaction, so a post racing an end either commits before the end or is refused |
| Upload quota | Session row `FOR UPDATE` on `uploaded_bytes` |

## 12. Observability (OB-7, OB-8)
- **Metrics.** As listed in OB-7, plus `blob_store_available`, `blob_integrity_failure_total`, `presence_peers`
  and `notify_listener_reconnects_total`.
- **Timestamps.** Each event row carries its commit time, so `realtime_delivery_seconds` is measured from commit to
  send.
- **Traces.** Traces cover post, event read and fan-out, with session and message ids as attributes. They never
  include content.

## 13. Testability (MT-13 to MT-18)
- **Access matrix.** A generated table of (endpoint or event, actor role, session visibility and state) to the
  expected outcome, run against the API with Docker PostgreSQL.
- **Blob store contract.** One contract suite run against the local adapter (temporary directory) and S3 (MinIO
  container).
- **Gateway.** Tested with a fake listener that can drop, reorder and duplicate notifications, to prove catch-up and
  deduplication.
- **Business days.** The calculation is pure and table-tested.
- **Delivery.** Playwright with two contexts for PF-10.

## 14. Traceability: requirement to pattern and component
| Requirement | Pattern | Components |
|---|---|---|
| SE-27, SE-28 | §1 | LC-36, LC-37 |
| SC-9, RL-15, PF-13 | §2 | LC-35, LC-37 |
| PF-10, PF-11 | §2, §3 | LC-37, LC-46 |
| SE-33, AV-9, AV-10 | §4 | LC-37 |
| SC-10 | §5 | LC-38 |
| PF-12, RL-12, RL-13, SE-29, SE-30 | §6 | LC-39, LC-40 |
| RL-17, AV-8 | §7 | LC-39, LC-23 |
| SE-31, SE-32 | §8 | LC-7, LC-41 |
| SE-34 | §9 | LC-45 |
| BR-31, RL-14, RL-18 | §10 | LC-43, LC-44 |
| RL-16 | §11 | LC-42 |
| OB-7, OB-8 | §12 | LC-3 |
| MT-13 to MT-18 | §13 | All |
