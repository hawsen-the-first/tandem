# Application Design Plan: Tandem

**Stage**: INCEPTION - Application Design
**Inputs**: `requirements.md`, `stories.md`, `personas.md`, `execution-plan.md` (all approved)
**Scope**: Components, interfaces, services and dependencies. Detailed business rules are left to Functional Design (per unit).

## Working assumptions (not questions)
These follow directly from approved requirements; say so if any is wrong.
- **X1**: REST (JSON) for commands and queries, plus one WebSocket per client for events, presence and agent streaming (NFR-1.4). Types for the TypeScript client are generated from an OpenAPI description.
- **X2**: PostgreSQL is the single source of truth. Blob storage (uploads, artifact bodies) sits behind an interface, local disk by default (NFR-1.3).
- **X3**: Model providers, web search, connectors and blob storage are each defined as an interface (port) with replaceable adapters (FR-9.1, FR-10.1, FR-12.1, NFR-1.3).
- **X4**: The server, not the model, enforces every permission and gate (NFR-4.1, SECURITY-11, SECURITY-15).

---

## Questions

## Question 1
How should the back end be deployed as components?

A) Modular monolith: one Rust codebase and image with clear internal modules, started in two modes (`api` and `worker`) so agent work can be scaled out later

B) Separate API and worker services from day one, with their own codebases and images

C) Microservices per domain (auth, collaboration, agents, workspaces)

D) Other (please describe after [Answer]: tag below)

[Answer]: B (see Clarification 1)

## Question 2
How should background agent and task work be queued (NFR-3.2, NFR-3.3, NFR-5.3)?

A) A job queue in PostgreSQL (row locking with `SKIP LOCKED`): no extra service to run, jobs survive restarts

B) Redis-backed queue: faster, but adds a required service

C) In-process task spawning only, no durable queue

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3
How should real-time events reach connected clients?

A) An in-process broadcast hub behind an interface, using PostgreSQL `LISTEN/NOTIFY` so events cross between API and worker processes and multiple API instances

B) In-process hub only for the MVP (single API process), with the interface ready for a message bus later

C) Redis pub/sub from the start

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4
How should the agent runtime execute thread agents and task agents?

A) In the worker process: a provider-agnostic agent loop that calls the model through an adapter and reaches the outside world only through a server-enforced tool layer (read file, write file, search, and so on), with no command execution

B) Each agent run in its own isolated sandbox (separate process or container) from the start

C) Use the Claude Agent SDK as the single runtime for all agents

D) Other (please describe after [Answer]: tag below)

[Answer]: D (see Clarification 2)

## Question 5
Who controls the session lifecycle: orchestrator or code?

A) Deterministic code owns a durable state machine (session phase, thread, plan and task status in the database). The orchestrator model proposes content (questions, plan, task output); code decides what transitions are allowed

B) The orchestrator model drives a free-form loop; code only adds guard checks at the gates

C) Other (please describe after [Answer]: tag below)

[Answer]: A (after worked examples were provided)

## Question 6
How should the server work with Git for workspaces (FR-11)?

A) Run the `git` command line from the back end (installed in the image), one worktree per session, with credentials passed per command and never written to disk

B) Use a Git library (`git2`/libgit2) inside the Rust process

C) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 7
How should the web client be built and served (NFR-1.1)?

A) A Vite + React single-page app (PWA), built to static files and served by the Rust server: one image to self-host

B) Next.js with its own Node server alongside the Rust back end

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
How should the Rust code be structured internally?

A) Hexagonal: a domain core (sessions, threads, approvals, plan, tasks) with no I/O, ports for providers, search, connectors, blob storage and Git, and adapters plus HTTP/WebSocket handlers around it

B) Simple layers: handlers, services, repositories, without formal ports

C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Execution Checklist (generation, after answers are approved)
- [x] 1. Identify components and responsibilities; write `components.md` (including RESILIENCY-01 criticality class and business impact per component)
- [x] 2. Define interfaces and method signatures; write `component-methods.md` (business rules deferred to Functional Design)
- [x] 3. Define services and orchestration; write `services.md`
- [x] 4. Map dependencies, communication patterns and data flows; write `component-dependency.md` (with validated diagrams)
- [x] 5. Map every epic and Must story to at least one component (traceability check)
- [x] 6. Check the design against Security Baseline (SECURITY-05, 08, 11, 15) and Resiliency Baseline (RESILIENCY-01, 06, 10) rules that apply at this stage
- [x] 7. Consolidate into `application-design.md`
- [x] 8. Update state, log the approval prompt, and present the completion message

## Extension compliance (planning)
- Security Baseline: applied at generation (checklist step 6)
- Resiliency Baseline: RESILIENCY-01 applied at generation (checklist step 1)
- Property-Based Testing: disabled

---

## Clarification 1 (Q1): separate API and worker
Answer B chosen. Follow-up: **one repository with shared crates**, a Rust workspace producing two binaries (`api`, `worker`) built into two images. Separate deployment and scaling without duplicated code.

## Clarification 2 (Q4): configurable sandboxing
User's words: "architect an almost dev container/sandbox approach for each sandbox but make this configurable ... configurable access controls (i.e. read some files but no write access outside the repo) ... enforced in a way that is more robust than just instructions."
Follow-ups answered:
- **Enforcement**: **Policy + tiered runners.** Every agent run gets a policy (readable paths, writable paths, network, allowed tools, resource limits). The server enforces it in the tool layer, and a configurable runner adds OS-level isolation: `in-process` (tool-layer only) or `container` (per-run, mounted paths only, no network by default).
- **Who configures**: **Admin sets one instance-wide policy.**

Resolved interpretation (recorded as assumptions, to confirm at approval):
- **X5**: The instance-wide policy fixes the rules (runner tier, network, resource limits, tool allow-list, whether an agent type may write). The concrete paths are bound per run from the session's grants (task agents: session workspace read-write; thread agents: read-only; approved extra repos: read-only). Per-session overrides are post-MVP; the data model keeps a policy scope so they can be added without rework.
- **X6**: Model calls and all provider credentials stay in the trusted worker. The sandbox only executes tool calls, so credentials never enter it (NFR-4.2).
- **X7**: Both runner tiers are in the MVP design. The container runner needs a container runtime on the host; it is optional and the admin chooses the default. This enlarges the MVP; confirm at approval whether the container runner is built in the MVP or immediately after it.
- **X8**: FR-8.3 (no command execution in the MVP) is unchanged. The tool layer defines a command tool that policy disables, so the first post-MVP milestone is switching it on under the container runner.

## Decisions summary
| # | Decision |
|---|---|
| Q1 | Separate `api` and `worker` services; one repo, shared crates |
| Q2 | PostgreSQL job queue (`SKIP LOCKED`) |
| Q3 | In-process hub behind an interface, `LISTEN/NOTIFY` between processes |
| Q4 | Policy-enforced sandbox with tiered runners (in-process, container); agent loop and credentials stay in the worker |
| Q5 | Code-owned durable state machine; the orchestrator model proposes content only |
| Q6 | Git library (libgit2 via `git2`) |
| Q7 | Vite + React SPA (PWA), served by the API image |
| Q8 | Hexagonal structure: domain core, ports, adapters |
