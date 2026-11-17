# U3 Collaboration Core: Technology Stack Decisions

Additions to the U1 and U2 decisions. Items marked **proposed** were not asked as questions; they follow from the
NFRs and can be changed at approval.

## 1. Decisions from your answers
| # | Decision | Rationale | Alternatives considered |
|---|---|---|---|
| T16 | **`BlobStore` port with a local directory adapter (default) and an S3-compatible adapter**, chosen by configuration (Q1) | Simple for one host; scales out on day one for those who need it; one contract test suite for both | PostgreSQL storage (database growth); S3 only (an extra service for everyone) |
| T17 | **Envelope encryption of every upload** with the vault, under the `upload` purpose (Q2) | A stolen volume, bucket or backup reveals nothing; same vault and key rotation as U1 | Rely on operator disk encryption |
| T18 | **No malware scanner in the MVP**, with a hook in the backlog (Q3) | Uploads are never run or rendered as HTML; avoids running ClamAV for everyone | Optional ClamAV hook now |
| T19 | **Upload retention after a session ends**, 180 days by default, as an instance setting (Q4, N9) | Bounds storage while keeping the decision record (messages and approvals stay) | Keep forever; delete at end |
| T20 | **Events carry ids and metadata only** (Q5) | One place where content leaves the server, behind the authorised HTTP reads | Full payloads in events |
| T21 | **`react-markdown`, `remark-gfm` and `rehype-sanitize`** with a Tandem allow-list schema; **`highlight.js`** core with languages loaded on demand (Q6) | Renders to React elements, so no HTML strings, a strict CSP and Trusted Types are possible | markdown-it with DOMPurify; server-side rendering |
| T22 | **Title search with PostgreSQL `pg_trgm`** (Q7) | Enough for pickers and lists; no search service | Full-text search now |
| T23 | **Presence in memory per process**, shared through `NOTIFY` (Q8) | No write per heartbeat; rebuilds within one heartbeat after a restart | Heartbeat rows in PostgreSQL |
| T24 | **Delivery checked in CI** by Playwright with two contexts, plus the histogram in the smoke performance test (Q9) | Repeatable; fails the build on regression | An added manual LAN check |

## 2. Proposed decisions
| Area | Choice | Notes |
|---|---|---|
| S3 client | `aws-sdk-s3` with a custom HTTP connector over our `outbound` client, or `object_store` if the connector cannot be wired | Every call must stay under the outbound URL rules (BR-19, SE-36). The final pick is in Code Generation, by whichever passes the outbound lint |
| Local blob layout | `TANDEM_DATA_DIR/uploads/{session}/{upload id}`, written to `tmp/` then renamed on the same file system | Atomic move (RL-12); ids, not hashes, in paths, because each file is encrypted with its own key |
| Upload encryption format | AES-256-GCM in 64 KiB chunks (STREAM construction: nonce prefix and counter, final-chunk flag) | Streaming with bounded memory (PF-12); truncation and reordering are detected |
| Type sniffing | `infer` crate on the first bytes | Decides the inline image types only (SE-30) |
| Multipart upload | Axum streaming body (`multipart` with field limits) | Aborts at 25 MiB plus 1 byte (SE-31) |
| WebSocket | Axum WebSocket, as U1, with a per-connection send queue (bounded at 256 events, dropped and told to reload when full) | A slow client cannot hold memory |
| Event store | A `session_events` table keyed by (session, sequence), pruned by a job | Replay (RL-15); `NOTIFY` carries only (session, sequence) (PF-13) |
| Business-day calculation | `chrono` with `chrono-tz` for the instance time zone | BR-31.3 |
| Front-end virtual list | `@tanstack/react-virtual` | 50,000-message histories (SC-8) |
| Front-end pickers | The U1 headless primitive library's combobox | ARIA combobox (US-7) |
| Contract tests for S3 | `testcontainers` MinIO module | MT-17 |

## 3. Configuration added
| Setting | Default | Notes |
|---|---|---|
| `TANDEM_DATA_DIR` | `./data` (in development) | Holds uploads with the local adapter |
| `TANDEM_BLOB_STORE` | `local` | `local` or `s3` |
| `TANDEM_S3_ENDPOINT`, `TANDEM_S3_BUCKET`, `TANDEM_S3_REGION`, `TANDEM_S3_PATH_STYLE` | none | Required when the store is `s3` |
| `TANDEM_S3_ACCESS_KEY_FILE`, `TANDEM_S3_SECRET_KEY_FILE` (or the `_KEY` environment forms) | none | Read from files by preference (SE-36) |
| Instance setting: upload retention | 180 days | 1 day to "keep forever" (RL-18) |
| Instance settings: reminder period, time zone | One business day, UTC | From the functional design (BR-31) |

## 4. Open points for NFR Design and Code Generation
- The exact S3 client crate (see above).
- The chunk size and the format header for encrypted uploads (a version byte for future changes).
- Whether the event store and the U1 outbox share one table, or the outbox feeds `session_events`.
