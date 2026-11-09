# Unit of Work Plan: Tandem

**Stage**: INCEPTION - Units Generation, Part 1 (Planning)
**Inputs**: `requirements.md` (80 FRs, amended), `stories.md` (85 stories), `application-design.md` (27 components)
**Terminology**: Tandem is built as **two deployable services** (`api`, `worker`) plus a web client, in one repo. A **unit of work** here is a *delivery slice*: a group of stories and the components needed to deliver them, designed and built completely before the next unit starts (per-unit loop). Units are not deployables and not necessarily directories.

## Proposed decomposition (a starting hypothesis for the questions below)

| Unit | Delivers | Main components | Main stories |
|---|---|---|---|
| **U1 Foundation and walking skeleton** | Repo scaffold, core/ports/app crates, Postgres and migrations, vault, audit base, identity (bootstrap, sign-in, invite, MFA), API and web shell, health and metrics, CI and images, scripted mock model provider | C1, C2, C4, C13, C24, C25, C26, C27 | E1, US-2.7, US-13.3 |
| **U2 Instance administration** | Provider, defaults, search and connector configuration, sandbox policy, connection tests | C12, C20 (config side), C21, C22 (config side) | US-2.1 to 2.6, 2.9 |
| **U3 Collaboration core** | Sessions (managed folder), membership, threads, messages, chat, uploads, approvals, real-time, notifications, presence | C3, C5, C6, C7, C11, C19, C23 | E3 (less repo), E5, E7, E12, US-13.1, 13.2 |
| **U4 Agent runtime and question rounds** | Job queue, orchestration state machine, agent loop, tool layer, in-process sandbox runner, provider adapters, `@ask` `@search` `@draft`, question rounds | C14, C15, C16, C18, C20, C21 | E4, most of E6 |
| **U5 Workspaces and GitHub** | libgit2 workspace manager, repo sessions, `@code`, extra repos, commit, push, pull request, cleanup | C17, C22 | US-3.2, US-6.3, 6.9, E11 |
| **U6 Plan, tasks and artifacts** | Plan versions and approval, task lifecycle, output review (diff and artifact), artifacts | C8, C9, C10 | E8, E9, E10 |
| **U7 Container sandbox runner** | Per-run container runner under the instance policy | C16 (runner) | US-2.9 (enforcement), US-9.3 |
| **U8 Hardening and delivery** | Backup and restore, degraded-state UX, end-to-end core-loop test, SBOM and signing, self-hosting guide, token usage | C26, C27 | US-13.4 to 13.6, 2.8 |

Question 1 decides whether this vertical-slice shape is used; the other answers adjust it.

## Questions

## Question 1
How should the system be sliced into units?

A) Vertical slices: each unit delivers an end-to-end capability (back end plus the UI for it) so the app is runnable and demoable after every unit, as in the proposed table

B) Layers: back-end units first (all components), then one large web client unit

C) Component groups: units follow the component catalogue closely (identity, collaboration, agents, workspaces, and so on), with the client built alongside each

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 2
Should the first unit be a walking skeleton (the thinnest end-to-end path through every layer)?

A) Yes: U1 ends with sign-in, creating a managed-folder session and posting a message working through web, API, worker and database, plus CI and images, before any feature depth

B) No: U1 is foundation and identity only; the first end-to-end path appears in a later unit

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3
When should CI/CD, Docker images and Compose packaging be built?

A) In U1: CI and images exist from the first commit and each unit extends them; final hardening and the self-hosting guide stay in the last unit

B) Only in the last unit (U8)

C) As a separate early unit of its own

D) Other (please describe after [Answer]: tag below)

[Answer]: B (see Clarification 1)

## Question 4
Where should the container sandbox runner (assumption X7) sit?

A) Its own unit (U7) after the agent and task units, so the MVP can be cut or shipped with only the in-process runner if time runs short

B) Inside U4 with the in-process runner, so both tiers exist from the first agent

C) Post-MVP: design only for now, build after the MVP

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
How should model providers be sequenced (Stories: OpenAI-compatible is Must, Anthropic API is Should, Agent SDK is Could)?

A) OpenAI-compatible adapter in U4; Anthropic API and Agent SDK adapters in U8 or a later slice

B) OpenAI-compatible and Anthropic API in U4; Agent SDK later

C) All three adapters in U4

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
Who builds the units, and can any run in parallel?

A) One developer working with AI assistance: units run strictly one after another in dependency order

B) Several people or agent sessions: independent units (for example U2 and U3 after U1) can be designed and built in parallel

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
How should the repository be organised? (The default AI-DLC pattern for a multi-unit monolith is `src/{unit}/`, which would fight the hexagonal crate layout.)

A) By architecture: `crates/` for shared libraries, `apps/api` and `apps/worker` for binaries, `web/` for the client; units are tracked through the story map and PR branches rather than directories

B) One directory per unit (`units/<unit>/...`), each holding its own crates and UI

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
How should story priority (MoSCoW) shape unit scope?

A) Each unit includes its Must and Should stories; Could stories move to a final backlog unit

B) Each unit includes only its Must stories; Should and Could stories are collected into follow-up units after the core loop works

C) Each unit includes all of its stories regardless of priority

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Categories evaluated without a question (with justification)
- **Business domain**: bounded contexts are already settled by the component catalogue (identity, collaboration, orchestration, agents, workspaces); no open choice.
- **Dependencies and inter-unit communication**: settled by Application Design (outbox events, job queue, ports); the unit dependency matrix is derived from the component matrix.
- **Technical scalability and deployment**: settled (separate `api` and `worker` images, optional SearXNG, sandbox containers); no per-unit differences remain.

---

## Execution Checklist (Part 2, after answers and plan are approved)
- [x] 1. Finalise unit definitions and responsibilities from the answers; write `unit-of-work.md` (including the greenfield code organization strategy)
- [x] 2. Write `unit-of-work-dependency.md` (dependency matrix and build order, derived from the component dependency matrix)
- [x] 3. Write `unit-of-work-story-map.md` assigning every one of the 85 stories to exactly one unit, with a coverage check
- [x] 4. Validate boundaries: no dependency cycles between units, every component built by exactly one unit, every FR reachable through a unit
- [x] 5. Record the per-unit design stages that apply (Functional Design, NFR Requirements, NFR Design, Infrastructure Design) for each unit
- [x] 6. Update state, log the approval prompt, present the completion message

## Extension compliance (planning)
- Security Baseline and Resiliency Baseline: N/A at this stage (decomposition only); security-critical components (C4, C7, C16, C17, C24) are flagged so their units get NFR stages first.
- Property-Based Testing: disabled

---

## Clarification 1 (Q2 vs Q3): CI timing
Q3 B ("only in the last unit") conflicted with Q2 A, whose skeleton included "CI and images". Resolved: **minimal CI now, packaging last.** U1 adds a minimal CI (build, test, lint, dependency scan) and a dev-only Postgres compose file; the skeleton runs locally. Image building, the reference Compose file, signing and SBOM wait for U8. This keeps SECURITY-10 (dependency scanning in CI) satisfied from the start while honouring the choice to package last.

## Resolved decomposition (applies the answers to the proposal)

| Unit | Scope after answers |
|---|---|
| **U1 Foundation and walking skeleton** | As proposed, plus **thin stubs** of the job queue, outbox and event bus so a posted message travels web, API, database, worker and back over the WebSocket; minimal CI and a dev-only compose file. Deepened later (U3, U4) |
| **U2 Instance administration** | Providers (OpenAI-compatible), defaults, tests, search, connector config, sandbox policy (Must and Should stories) |
| **U3 Collaboration core** | As proposed (managed-folder sessions) |
| **U4 Agent runtime and question rounds** | Full job queue, orchestration, agent loop, tool layer, **in-process runner**, **OpenAI-compatible adapter only**, `@ask` `@search` `@draft` |
| **U5 Workspaces and GitHub** | As proposed |
| **U6 Plan, tasks and artifacts** | As proposed, minus Could stories (see U9) |
| **U7 Container sandbox runner** | After U6, as its own unit (Q4 A) |
| **U8 Hardening and delivery** | Backup/restore, degraded-state UX, end-to-end core-loop test, **image building, reference Compose, signing, SBOM**, self-hosting guide, **Anthropic API adapter and its admin story (Should)** |
| **U9 Backlog (Could)** | Agent SDK adapter and its admin story, token usage view (US-2.8), save artifact to workspace (US-10.5) |

Sequential build order (Q6 A): U1, U2, U3, U4, U5, U6, U7, U8, U9, with U9 optional for the MVP.

## Further decisions from the answers
- **Story split**: US-2.2 mixes a Should part (Anthropic API) and a Could part (Agent SDK). In Part 2 it is split into **US-2.2** (Anthropic API, Should, U8) and a new **US-2.10** (Agent SDK, Could, U9), so every story maps to exactly one unit.
- **Repo layout (Q7 A)**: `crates/` (shared libraries), `apps/api`, `apps/worker`, `web/`. Units are delivery slices tracked by the story map and PR branches, not directories. This is recorded as the greenfield code organization strategy in `unit-of-work.md`.
- **Per-unit design stages**: all four (Functional Design, NFR Requirements, NFR Design, Infrastructure Design) apply per the execution plan. Infrastructure Design is light for U2 to U7 (dev-only) and full in U8 (packaging).
