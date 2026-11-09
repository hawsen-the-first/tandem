# Tandem: Components

**Basis**: `requirements.md`, `stories.md`, `application-design-plan.md` (decisions Q1 to Q8, assumptions X1 to X8).
**Style**: Hexagonal. A domain core with no I/O; ports for everything external; adapters and delivery mechanisms around it.
**Deployables**: `api` image (also serves the web client), `worker` image, PostgreSQL, optional SearXNG, and short-lived sandbox containers started by the worker.

## 1. Deployables and code layout

One repository, one Rust workspace, shared crates, two binaries.

| Unit | Kind | Contains |
|---|---|---|
| `web` | Vite + React SPA (PWA) | C1 |
| `api` binary | Deployable (stateless) | C2, C3, plus application services in request mode |
| `worker` binary | Deployable (scalable) | C14 to C17, C18 consumer, application services in job mode |
| `core` crate | Library | Domain model and state machines (C5 to C10 rules); no I/O |
| `app` crate | Library | Application services and use cases (C4 to C13) over ports |
| `ports` crate | Library | Port traits (provider, search, connector, blob, git, runner, queue, event bus, vault) |
| adapter crates | Libraries | Postgres, providers, search, GitHub, git, blob, vault, runners |

## 2. Component catalogue

Criticality classes (RESILIENCY-01): **Critical** = loss stops use or risks data or security; **High** = core loop degrades; **Medium** = a feature degrades with a clear fallback. Business impact is stated for the self-hosting team (there is no revenue or regulatory dependency; impact is blocked work or lost decisions).

### Client and delivery

| ID | Component | Responsibilities | Interfaces | Class | Impact if unavailable |
|---|---|---|---|---|---|
| C1 | **Web Client** | Sessions, threads, chat, plan and task review, artifacts, admin screens; responsive, light/dark, PWA; renders sanitised Markdown and Mermaid | Calls C2 (REST), connects to C3 (WebSocket) | High | Users cannot interact; server state is unaffected |
| C2 | **HTTP API** | REST endpoints, authentication middleware, input validation, rate limiting, security headers, OpenAPI description; serves the static client | REST over TLS (behind a reverse proxy); calls application services | Critical | Nothing can be read or changed |
| C3 | **Realtime Gateway** | One WebSocket per client; authorises subscriptions to sessions; delivers events, presence and agent streaming | WebSocket; subscribes to C19 | High | Live updates stop; REST still works and clients can refresh |

### Application services (shared `app` crate)

| ID | Component | Responsibilities | Class | Impact if unavailable |
|---|---|---|---|---|
| C4 | **Identity and Access** | Users, invitations, sign-in, Argon2id, breached-password check, TOTP MFA, cookies and server-side sessions, lockout, roles; central `authorize(...)` for membership and admin checks | Critical | No sign-in or authorisation; system fails closed |
| C5 | **Session Service** | Sessions, membership, workspace choice, per-session model choice, session lifecycle state machine (`active`, `ended`, `archived`) | High | Cannot start, end or change sessions |
| C6 | **Thread and Messaging** | Question rounds as threads, messages, replies, references, uploads (metadata), session chat, thread status | High | Discussion stops |
| C7 | **Approval Service** | Single-approval rule for threads, plan and task output (data model supports richer rules later), revoke, dependency check before revoke, outcome summary confirmation | Critical | Gates cannot open; nothing proceeds (safe failure) |
| C8 | **Plan Service** | Plan versions, steps, dependencies, step-to-thread traceability, rework requests, plan approval gate | High | Planning blocked |
| C9 | **Task Service** | Task records and status, dependency ordering, output snapshots (diff or artifact version), accept, reject with feedback, discard, retry | High | Work cannot be tracked or reviewed |
| C10 | **Artifact Service** | Versioned artifacts, diff between versions, edit, download, save into workspace | Medium | Artifacts unavailable; discussion and approvals still work |
| C11 | **Notification and Presence** | In-app notifications, unread counters, presence | Medium | Fewer prompts to act; nothing is lost |
| C12 | **Admin and Settings** | Model providers, defaults, search providers, connectors, **sandbox policy** (new), instance settings, token usage view | High | Cannot reconfigure; running sessions continue |
| C13 | **Audit Service** | Append-only security audit log (SECURITY-14) with filters | High | Security events not recorded (writers fail loudly rather than skip) |

### Worker-side components

| ID | Component | Responsibilities | Class | Impact if unavailable |
|---|---|---|---|---|
| C14 | **Orchestration Engine** | Consumes domain events; drives the durable lifecycle state machine; enqueues jobs for question rounds, outcome summaries, plan drafting and task execution; validates every orchestrator proposal before applying it | Critical | Sessions stall at the next transition; state is safe in the database |
| C15 | **Agent Runtime** | Builds context, runs the provider-agnostic agent loop for orchestrator, thread agents and task agents, streams output, records the activity log; holds model credentials; routes tool calls through the policy-checked tool layer | High | `@` agents and tasks stop; discussion and approvals continue |
| C16 | **Sandbox Manager** | Resolves the instance-wide sandbox policy into a per-run policy; selects the runner; enforces path, tool, network and resource rules; runners: **in-process** (tool-layer confinement) and **container** (per-run, mounted paths only, network off by default) | Critical | No tool can run; agents fail closed |
| C17 | **Workspace Manager** | Clones into isolated worktrees via libgit2, session branches, read-only extra repos, snapshots diffs, commit and push, cleanup on session end; only component with Git credentials and workspace access | Critical | Repo sessions cannot start, commit or clean up |

### Infrastructure services

| ID | Component | Responsibilities | Class | Impact if unavailable |
|---|---|---|---|---|
| C18 | **Job Queue** | PostgreSQL queue (`SKIP LOCKED`): enqueue, lease, heartbeat, retry with backoff, dead-letter, resume after restart | High | Background work pauses; resumes on recovery |
| C19 | **Event Bus** | In-process hub behind a port; `LISTEN/NOTIFY` between API and worker processes; fan-out to C3 | High | Live updates and cross-process triggers delayed; polling fallback via the queue |
| C20 | **Model Provider Port and adapters** | OpenAI-compatible, Anthropic API, optional Agent SDK; capability discovery; timeouts, retries, circuit breaking | High | Agents degrade with a clear message (NFR-5.2) |
| C21 | **Search Port and adapters** | SearXNG default, Brave, Tavily | Medium | `@search` shows an unavailable message |
| C22 | **Connector Port and GitHub adapter** | Repo listing, clone credentials, push, pull request creation; credentials encrypted | High | Repo sessions cannot clone or push; managed-folder sessions unaffected |
| C23 | **Blob Store Port and adapters** | Local-disk adapter; content hashes; S3-compatible later | High | Uploads and artifact bodies unavailable |
| C24 | **Secret Vault** | Application-level envelope encryption for provider keys, connector tokens and TOTP secrets; key from env or file | Critical | Secrets unreadable; dependent features fail closed |
| C25 | **Persistence** | PostgreSQL repositories, reversible versioned migrations, transactions | Critical | System down |
| C26 | **Observability** | Structured logs with request IDs, Prometheus metrics, `/healthz` and `/readyz` | High | Operators lose visibility; behaviour unaffected |
| C27 | **Operations CLI** | First-run bootstrap, migrations, backup and restore, admin recovery | High | Operators lose maintenance tooling; runtime unaffected |

## 3. Key interface contracts (summary)

| Port | Purpose | Implementations in MVP |
|---|---|---|
| `ModelProvider` | Chat completion with tools, streaming, capability probe | OpenAI-compatible, Anthropic, Agent SDK |
| `SearchProvider` | Web search with cited results | SearXNG, Brave, Tavily |
| `Connector` | Repo list, clone auth, push, create PR | GitHub |
| `BlobStore` | Put, get and delete content by hash | Local disk |
| `GitWorkspace` | Clone, worktree, diff, commit, push, delete | libgit2 adapter |
| `SandboxRunner` | Execute a policy-bound tool call | In-process, container |
| `JobQueue` | Enqueue, lease, complete, fail | PostgreSQL |
| `EventBus` | Publish and subscribe to domain and UI events | In-process plus `LISTEN/NOTIFY` |
| `SecretVault` | Encrypt and decrypt secrets | Envelope encryption |

## 4. Security properties built into the boundaries
- **The API has no Git credentials, no workspace access and no container runtime access.** Only the worker (C15 to C17) does, which is why `api` and `worker` are separate images.
- **Agents are untrusted.** The agent loop runs in the worker; every tool call is checked against a per-run policy and executed by a runner. Credentials never enter a sandbox (assumption X6).
- **Code owns every gate.** C14, C7 and the state machines in `core` decide transitions; model output is parsed into structured proposals that code validates (decision Q5).
