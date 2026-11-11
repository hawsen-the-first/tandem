# Tandem: Units of Work

**Basis**: `unit-of-work-plan.md` (answers Q1 to Q8 and Clarification 1), `application-design.md`, `stories.md`.
**Terminology**: Tandem ships as two deployable **services** (`api`, `worker`) plus a web client, built from one repository. A **unit of work** is a delivery slice: a set of stories and the components needed to deliver them, designed and built completely before the next unit starts. Units are not deployables and not directories (decision Q7).
**Build order** (one developer with AI assistance, strictly sequential, decision Q6): U1, U2, U3, U4, U5, U6, U7, U8, U9. U9 is optional for the MVP.

## 1. Unit overview

| Unit | Name | Stories | Delivers (demo at the end of the unit) |
|---|---|---|---|
| U1 | Foundation and walking skeleton | 12 | Admin sets up an instance, invites a user, both sign in (MFA for admin); a user creates a managed-folder session and posts a message that travels web, API, database, worker and back over the WebSocket; minimal CI is green |
| U2 | Instance administration | 6 | Admin configures an OpenAI-compatible provider, defaults, search, the GitHub connector and the sandbox policy, and tests each connection |
| U3 | Collaboration core | 21 | Members work in a session: threads, messages, replies, uploads, chat, approvals, live updates, notifications, presence; sessions can be ended and archived |
| U4 | Agent runtime and question rounds | 15 | The orchestrator proposes question rounds from a prompt; members call `@ask`, `@search`, `@draft`; outcome summaries; all through the policy-checked tool layer and the in-process runner |
| U5 | Workspaces and GitHub | 7 | Repo sessions: clone, `@code`, extra read-only repos, commit and push to a session branch, pull request, cleanup |
| U6 | Plan, tasks and artifacts | 20 | Plan draft, rework and approval; tasks run in dependency order; diff and artifact review; artifacts with versions, diff, edit and download. **The full core loop works end to end** |
| U7 | Container sandbox runner | 1 | The container runner enforces the sandbox policy at OS level; fail-closed behaviour proven |
| U8 | Hardening and delivery | 4 | Backup and restore, degraded-state UX, end-to-end core-loop test, signed images, SBOM, reference Compose, self-hosting guide, Anthropic API provider |
| U9 | Backlog (Could-have) | 3 | Agent SDK provider, token usage view, save artifact to workspace |

## 2. Unit definitions

### U1 Foundation and walking skeleton
- **Purpose**: Establish the repo, the architecture skeleton and the security base, and prove the whole path end to end with the thinnest possible slice.
- **Responsibilities**: Cargo workspace and `web/` scaffold; `core`, `ports` and `app` crates; PostgreSQL schema base and reversible migrations; secret vault; audit base; identity (bootstrap admin, sign-in, invite by copyable link, MFA, password change and reset by link, deactivate and reactivate, role changes, progressive-delay lockout); instance security settings (sign-in duration, online breach check); append-only audit log viewer; API and web shell with security headers, responsive layout and light/dark themes; health, readiness, metrics and structured logs; operations CLI (migrate, bootstrap); **thin stubs** of session, message, job queue, outbox and event bus for the skeleton; scripted mock model provider (NFR-7.3); minimal CI (build, test, lint, dependency scan) and a dev-only Postgres compose file.
- **Components introduced**: C1, C2, C4, C13, C24, C25, C26, C27; stubs of C3, C5, C6, C18, C19.
- **Not in U1**: image building, reference Compose, signing, SBOM (U8).
- **Exit criteria**: demo above; all U1 Must stories pass; CI green; no secret appears in logs.
- **Why first**: security-critical base (C4, C24, C25) and every later unit depends on it.

### U2 Instance administration
- **Purpose**: Let the admin configure everything agents and workspaces will need, without any agent running yet.
- **Responsibilities**: OpenAI-compatible provider config and test with capability report; default models; search provider config (SearXNG default, API-key providers) and test; GitHub connector config and test; sandbox policy storage and validation (instance-wide, scope kept for later per-session overrides); all changes audited.
- **Components introduced**: C12; config sides of C20, C21, C22.
- **Exit criteria**: admin can save, test and edit each configuration; secrets are never shown again; the policy is stored and audited.

### U3 Collaboration core
- **Purpose**: The human collaboration experience with no AI yet.
- **Responsibilities**: Sessions with managed-folder workspaces; membership and members-only access; overview; threads, messages (Markdown), replies, references, uploads and size limits; session chat; thread status; thread approval, who and when, revoke; ended and archived sessions; real-time delivery; notifications and unread; presence; safe rendering of untrusted Markdown.
- **Components introduced**: C3, C5, C6, C7, C11, C23; C19 deepened from stub.
- **Exit criteria**: two browsers collaborate in a session in near real time (about 500 ms on a LAN); non-members see nothing; approvals gate thread status; safe rendering verified with hostile input.

### U4 Agent runtime and question rounds
- **Purpose**: Bring in the orchestrator and thread agents under a code-owned state machine and a policy-enforced tool layer.
- **Responsibilities**: Full PostgreSQL job queue (leases, heartbeat, retry, dead-letter); outbox and orchestration engine with the lifecycle state machine; agent loop, context builder, activity log, streaming and cancel; tool layer with policy resolution; **in-process runner**; **OpenAI-compatible adapter**; `@ask`, `@search`, `@draft`; question rounds, editing of proposed questions, requesting rounds; outcome summaries and editing; per-session model choice.
- **Components introduced**: C14, C15, C16 (in-process runner), C18 (full); OpenAI-compatible adapter of C20; search adapters of C21.
- **Exit criteria**: from a prompt, the orchestrator proposes questions; agents answer with citations; agents cannot approve or write; failure of a provider leaves state unchanged; jobs resume after a worker restart.

### U5 Workspaces and GitHub
- **Purpose**: Repo sessions and everything that touches Git.
- **Responsibilities**: Workspace manager on libgit2 (clone, session worktree, branch, read-only extra repos, diff snapshots, commit, push, cleanup); GitHub connector runtime (list, clone credential, push, pull request); `@code`; extra-repo request and approval; commit and push by explicit member action; pull request; cleanup retried until it succeeds; uncommitted-change tracking.
- **Components introduced**: C17; runtime side of C22.
- **Exit criteria**: a repo session clones, `@code` cites files, an explicit commit pushes to the session branch, cleanup leaves no clone; the API process holds no Git credentials.

### U6 Plan, tasks and artifacts
- **Purpose**: Complete the core loop.
- **Responsibilities**: Plan drafting, validation (acyclic, linked to threads), versions, rework, step threads, approval and lock; no task before approval; task spawning in dependency order; task agents writing only through the tool layer; no commands; diff and artifact output review, approve, reject with feedback, discard, retry; artifacts with rendering (Markdown, code, Mermaid), history, diff, edit, download.
- **Components introduced**: C8, C9, C10.
- **Exit criteria**: the reference scenario (steps 1 to 12 in `stories.md`) runs end to end on the in-process runner.

### U7 Container sandbox runner
- **Purpose**: OS-level enforcement of the sandbox policy.
- **Responsibilities**: Container runner (per-run container, policy-bound mounts, no network by default, non-root, read-only root, resource limits, no credentials inside); fail-closed on start failure; teardown; documentation of the host requirement; enforcement tests for US-6.8 and US-9.3.
- **Components extended**: C16.
- **Exit criteria**: under the container runner, a write outside the bound path, a network call and a credential read all fail even when the model is instructed to attempt them.

### U8 Hardening and delivery
- **Purpose**: Make it installable, recoverable and trustworthy for operators.
- **Responsibilities**: Image building for `api` and `worker`, reference Compose (api, worker, PostgreSQL, optional SearXNG), signing, SBOM and vulnerability scanning in CI, release process; backup and restore commands; degraded-service messages; data-sent-to-provider documentation; Anthropic API provider; end-to-end core-loop test; self-hosting guide (install, upgrade, TLS and reverse proxy, backup, operator decisions per NFR-5.4).
- **Components extended**: C20 (Anthropic), C26, C27.
- **Exit criteria**: a clean machine can install from the guide and complete the reference scenario; a backup restores to a working instance.

### U9 Backlog (Could-have)
- **Responsibilities**: Agent SDK provider adapter and admin story, token usage view, save artifact to workspace.
- **Exit criteria**: each story passes its checklist; skippable without affecting the MVP.

## 3. Component ownership

Each component is **introduced** in exactly one unit; later units **extend** it.

| Component | Introduced | Extended in |
|---|---|---|
| C1 Web Client | U1 | U2 to U9 (each unit adds its screens) |
| C2 HTTP API | U1 | U2 to U9 |
| C3 Realtime Gateway | U3 (stub in U1) | U4, U6 |
| C4 Identity and Access | U1 | |
| C5 Session Service | U3 (stub in U1) | U5 |
| C6 Thread and Messaging | U3 (stub in U1) | U4 |
| C7 Approval Service | U3 | U6 (plan and task output) |
| C8 Plan Service | U6 | |
| C9 Task Service | U6 | |
| C10 Artifact Service | U6 | U9 |
| C11 Notification and Presence | U3 | U6 |
| C12 Admin and Settings | U2 | U8, U9 |
| C13 Audit Service | U1 | every unit records its events |
| C14 Orchestration Engine | U4 | U6 |
| C15 Agent Runtime | U4 | U5, U6 |
| C16 Sandbox Manager | U4 (in-process runner) | U7 (container runner) |
| C17 Workspace Manager | U5 | U6, U9 |
| C18 Job Queue | U1 (stub) / U4 (full) | |
| C19 Event Bus | U1 (stub) | U3 |
| C20 Model Provider Port | U2 (config) | U4 (OpenAI-compatible), U8 (Anthropic), U9 (Agent SDK) |
| C21 Search Port | U2 (config) | U4 (adapters) |
| C22 Connector Port and GitHub | U2 (config) | U5 (runtime) |
| C23 Blob Store | U3 | U6 |
| C24 Secret Vault | U1 | |
| C25 Persistence | U1 | every unit adds migrations |
| C26 Observability | U1 | U8 |
| C27 Operations CLI | U1 | U8 |

Where a component has a stub and a full version (C18, C19, and the C3, C5, C6 stubs), the unit that completes it is the one named in the story map; the stub exists only to prove the skeleton and is replaced, not duplicated.

## 4. Code organization strategy (greenfield, multi-unit)

Decision Q7: organise **by architecture**, not by unit. The default AI-DLC multi-unit monolith pattern (`src/{unit}/`) is not used because it would cut across the hexagonal layering. All application code lives at the workspace root and never in `aidlc-docs/`.

```text
tandem/
  Cargo.toml                workspace manifest
  crates/
    core/                   domain model and state machines (no I/O)
    ports/                  port traits
    app/                    application services (C4 to C13) and orchestration use cases
    store-postgres/         repositories, queue, outbox, LISTEN/NOTIFY, migrations
    config/                 configuration loader (environment, TOML, secret files)
    credentials/            Argon2id hasher, breached-password checker, TOTP
    support/                clock, random source, circuit breaker, supervised tasks
    bootstrap/              shared start-up for api, worker and cli (pools, vault, guards, probes)
    vault/                  envelope encryption
    providers/              model provider adapters (openai-compatible, anthropic, agent-sdk, mock)
    search/                 SearXNG, Brave, Tavily adapters
    github/                 GitHub connector
    gitws/                  libgit2 workspace adapter
    blob-local/             local-disk blob store
    sandbox/                policy, in-process runner, container runner
    runtime/                agent loop, context builder, tool layer (U4 onwards; not the U1 `bootstrap` crate)
    telemetry/              logging, metrics, health
  apps/
    api/                    HTTP and WebSocket server, serves web/dist
    worker/                 job runner, orchestration, agents, workspaces
    cli/                    operations CLI (C27)
  web/                      Vite + React SPA (PWA), generated API types
  tools/
    breach-filter/         builds the offline breached-password filter (U1)
  tests/                    cross-crate integration and end-to-end tests (U1: end-to-end suite is in web/e2e, performance smoke test in apps/api/tests)
  dev/                      dev-only compose file (U1)
  deploy/                   Dockerfiles, reference Compose, release files (U8)
  docs/                     self-hosting guide, operator documentation (U8)
  .github/workflows/        CI (minimal in U1, extended in U8)
  aidlc-docs/               process documentation only
```

Units are tracked through the story map and by branch and PR naming (`unit-u3/<topic>`), not by directory. Each unit's per-unit design lives under `aidlc-docs/construction/<unit-name>/`.

## 5. Per-unit design stages (recommendation, finalised inside each unit)

Functional Design (FD), NFR Requirements (NFR-R), NFR Design (NFR-D) and Infrastructure Design (ID). **Full** = run in full; **Light** = run briefly with the unit's specifics; **Skip** = nothing new, justification recorded in the stage.

| Unit | FD | NFR-R | NFR-D | ID |
|---|---|---|---|---|
| U1 | Full (identity, vault, audit data model) | Full (sets the security, logging and testing baselines; confirms framework choices per NFR-1.5) | Full | Light (dev compose, CI) |
| U2 | Light (config entities, policy model) | Light | Light | Skip |
| U3 | Full (approval rules, membership, versioning) | Full (real-time latency, input limits) | Full | Skip |
| U4 | Full (state machine, proposals, tool layer, policy resolution) | Full (agent containment, provider resiliency) | Full | Skip |
| U5 | Full (workspace lifecycle, commit rules) | Full (Git credentials, isolation) | Full | Light (workspace volume) |
| U6 | Full (plan, task and artifact rules) | Light | Light | Skip |
| U7 | Light | Full (container hardening, host runtime exposure) | Full | Full (runtime access, networking) |
| U8 | Light | Full (supply chain, backup integrity) | Light | Full (images, Compose, release) |
| U9 | Light | Skip | Skip | Skip |

**Security-critical components** (C4, C7, C16, C17, C24) sit in U1, U3, U4, U5 and U7. Their units run the NFR stages in full so the Security Baseline rules are applied where they matter.
