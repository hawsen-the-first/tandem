# NFR Design Plan: U3 Collaboration core

**Stage**: CONSTRUCTION - NFR Design, **Full** (unit `u3-collaboration-core`)
**Inputs**:
- U3 NFR requirements (SC-7 to US-8, T16 to T24) and functional design (BR-22 to BR-34).
- U1 and U2 NFR design: patterns and logical components LC-1 to LC-34. These stand: the outbox, the job queue,
  the WebSocket hub with its `NOTIFY` listener, the vault and the rate limiter.

**New logical components** continue from **LC-35**.

## Steps
- [x] 1. Analyse the NFR requirements and the U1 real-time components
- [x] 2. Answer the questions below
- [x] 3. Review the answers; follow up if needed
- [x] 4. Generate `nfr-design-patterns.md` and `logical-components.md`
- [x] 5. Approval (pull request)

## Settled by earlier decisions (not asked)
- **Downloads go through the API.** Uploads are encrypted by Tandem (T17), so a pre-signed S3 address would only
  hand out ciphertext. Downloads stream through the `api`, which decrypts on the fly after the access check.
- **Slow WebSocket clients.** A full send queue (256 events) closes that client's subscription with "reload"
  (tech-stack decisions). A slow client never holds memory or slows others.
- **Vault key rotation.** The U2 `rekey` command gains uploads. It rewraps each file's data key only, and never
  re-encrypts file contents.

## Working assumptions (not questions; confirm or correct at approval)
- **D1, sequence allocation.** The per-session counter is a column on the session row, incremented with
  `UPDATE … RETURNING` in the writing transaction. This serialises writers within one session only; at 30 messages
  per 10 s per user, it is far from a bottleneck.
- **D2, rate limiting.** Per-user write limits (SE-32) use the same in-process limiter as U1, keyed by user id.
  Behind several `api` processes the effective limit is per process, which is acceptable at design load. The
  account guard stays in PostgreSQL.
- **D3, notification delivery.** Notifications are delivered on a per-user channel of the same WebSocket. The event
  carries the notification id only, so the bell refetches its count.
- **D4, quiet-thread reminders.** Each thread has a computed `remind_at` column, set when activity happens or a
  period changes, and indexed for open and reopened threads. A worker loop every 5 minutes reads the due rows with
  `SKIP LOCKED`, so several workers never send the same reminder.
- **D5, retention and sweeps.** These run as daily jobs in the worker: pending-upload discard (hourly), event
  pruning, notification pruning, upload retention and the orphan sweep. Each works in batches of 500 with a time
  budget, so no single run holds locks for long.

---

## Questions

Fill in the letter after each `[Answer]:` tag, and add notes if you like.

## Question 1 (resilience)
What should an `api` process do when its PostgreSQL `NOTIFY` listener drops (a database restart or network blip)?

A) **Reconnect and catch up from the event store.** While reconnecting, each subscribed session's last delivered
sequence is remembered. After reconnecting, the gateway reads `session_events` after it and delivers the missed
events, so clients notice nothing. If the gap passes the replay window, those clients are told to reload.

B) **Reconnect, and tell every connected client to reload.** It is simpler, but every blip causes a visible refresh
for everyone on that process.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 2 (scalability, logical components)
How do the new `session_events` store and the U1 outbox relate? This was left open in the technology decisions.

A) **`session_events` is the outbox for session-scoped events.** It is written in the same transaction as the
change, then `NOTIFY` carries (session, sequence). The U1 outbox stays for jobs and other non-session events. There
is one durable record per event, and replay reads the same table delivery uses.

B) **The U1 outbox feeds `session_events`.** Changes write to the outbox as today, and a relay copies them into
`session_events` with sequence numbers. There is one write path, but a relay step adds latency and another thing that
can lag.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3 (performance)
How should the gateway check access for each event it delivers (BR-23.7, SE-28)?

A) **A per-process cache of (user, session) access**, filled on subscribe. It is cleared at once by the member,
owner, visibility, end and archive events for that session, with a 60 s expiry as a backstop. Delivery stays fast,
and losing access still takes effect at the next event.

B) **A database query for every delivery.** It is always exact, but costs 30 queries per message in a busy session.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4 (performance)
After a `message.created` event, how should the browser fetch the new content (events carry ids only, T20)?

A) **Batch by sequence.** The client waits up to 50 ms to collect events, then makes one request,
`GET …/messages?after={last}`, for each channel it is showing. It makes one request for a burst instead of one per
message, which keeps well within 500 ms.

B) **One request per message by id.** Simplest, but a burst of 20 messages means 20 requests per reader.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5 (resilience)
If the blob store fails (an S3 outage or a full disk), what should happen?

A) **Uploads and downloads fail cleanly; the rest of Tandem keeps working.**
- A circuit breaker opens after 5 failures in 30 s and stays open for 30 s.
- Uploads are refused with "File storage is unavailable. Try again shortly", and downloads answer 503 with the same
  text.
- Messages, threads and approvals continue.
- `/readyz` reports the blob store as **degraded** without failing, so a proxy does not take every `api` process
  out of service.
- An alert fires.

B) **`/readyz` fails while the blob store is unreachable**, as AV-8 reads today, so orchestrators restart or route
around the process. It is stricter, but an S3 outage takes all of Tandem offline.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6 (security)
Where should the sanitiser's allow-list live, so that it can be reviewed and tested once (SE-34)?

A) **One shared schema module in the web client**, `markdown/schema.ts`, used by `MarkdownView` and by the hostile
input tests. Any change to it needs a test that covers the new element or attribute, and lint forbids importing
`rehype-raw` or calling `dangerouslySetInnerHTML` anywhere.

B) **As A, plus a matching check on the server.** The API refuses bodies containing raw HTML tags, rather than
storing and escaping them. This is defence in depth, but some legitimate text (for example, `<div>` in prose about
HTML) would need to be written as code.

X) Other (please describe after [Answer]: tag below)

[Answer]: A
