# Requirements Clarification Questions: Tandem (Round 2)

Thanks for the detailed answers. Reviewing them turned up a few ambiguities and one tension between
answers (Part 1), and the Resiliency extension you enabled requires eight decisions to be made by you,
not the AI (Part 2). Each question notes which earlier answer it relates to, plus a suggested default
where one is obvious for a self-hosted MVP. Suggestions are only suggestions.

Fill in the letter after each `[Answer]:` tag (or reply in chat, e.g. "C1 B, C2 A, …").

---

# Part 1: Ambiguities in Round 1 Answers

## Tension 1: Integrations vs. "plug into existing tools"
You said Tandem should "plugin to existing workflows/tools where sensible" (Q2), but did not select
integrations for the MVP (Q7: A, C, F). At the same time, the orchestrator spawning tasks to
"implement what was agreed" (Q5) and agents doing "code analysis" suggests a connection to code
repositories.

### Clarification Question 1
How should integrations be handled in the MVP?

A) No live integrations in the MVP, but design a plugin/adapter interface now so they can be added without rework

B) Include one integration in the MVP: Git repositories (clone a repo into a session for analysis, and let tasks push branches)

C) Include Git plus MCP (Model Context Protocol) client support, so agents can use any MCP server as a tool

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Ambiguity 1: What spawned tasks actually do
Q5 says the orchestrator "can take this context and spawn tasks to implement what was agreed". The
scope of "implement" drives a lot of architecture (sandboxing, code execution, security).

### Clarification Question 2
For the MVP, what does a spawned task produce?

A) Artifacts inside Tandem only: an agent writes documents, specs or code files into the session, and people review them there

B) Real code changes: an agent works on a Git repo in an isolated sandbox container, runs commands/tests, and proposes a branch or pull request

C) A task list only: the orchestrator turns agreed threads into tasks that humans (or external tools) carry out

D) A then B: MVP produces artifacts in Tandem (A); sandboxed repo work (B) is the first post-MVP milestone

X) Other (please describe after [Answer]: tag below)

[Answer]: X) I think ideally this would be flexible. Maybe similar to the Github Copilot app, where we can work in folders and/or repositories and there is the ability to write and commit code in those repos, or simply generate artifacts. This should always be gated by the questions that are being worked through.

## Ambiguity 2: Source of code for "code analysis" in threads

### Clarification Question 3
When someone @-mentions an agent to analyse code in a thread, where does the code come from?

A) Files and artifacts already in the session (uploaded files or agent-created artifacts)

B) A Git repository linked to the session (cloned read-only by the server)

C) Both A and B

X) Other (please describe after [Answer]: tag below)

[Answer]: X) See answer to above - Code would be in the repo for that session. But it should have permission gated access to other repos on request/approval. These might be local or remote, make use of a github connector here.

## Ambiguity 3: Voting/approval rules for closing a thread
Q5 asks for a "configurable voting/approval system that marks a thread as completed".

### Clarification Question 4
Which approval rules should the MVP support? (Session creator picks one per session, with optional per-thread override.)

A) Minimal: session owner marks a thread as agreed

B) Configurable: owner-only, any N approvals, simple majority, or unanimous of session members

C) As B, plus the orchestrator AI can recommend closing (with a summary of the agreed outcome) but never vote

D) As C, plus AI agents can be given a vote

X) Other (please describe after [Answer]: tag below)

[Answer]: X) I think to start with, there can just be a simple approval for each discussion thread. We can flesh this out later to include more complicated approval workflows.

## Ambiguity 4: Human control over the orchestrator
Q5 describes the orchestrator generating questions (threads) from the initial prompt, then spawning tasks.

### Clarification Question 5
How much control do humans have over the orchestrator's flow?

A) Orchestrator proposes the list of questions/threads; humans can edit, add, remove or reorder them before discussion starts, and must approve before tasks are spawned

B) Orchestrator creates threads automatically; humans must approve before tasks are spawned

C) Fully automatic once each thread is approved

X) Other (please describe after [Answer]: tag below)

[Answer]: X) I think everything should work in threads with approvals. So after the initial questions are approved, there may be further rounds of clarification with new threads. Then we can follow a similar process to interactive mode, where the plan is presented with clear next steps on what will be built and each step can spawn a new thread if required (by the user not the agent) and there is the option to approve the plan, or rework it etc.

## Ambiguity 5: Sign-in methods (Q9: "A and/or C")

### Clarification Question 6
Given self-hosting, which sign-in approach should the MVP use?

A) Email + password always available; magic link turns on automatically when an SMTP server is configured (suggested default)

B) Email + password only for MVP; magic link later

C) Magic link only (SMTP required to run Tandem)

X) Other (please describe after [Answer]: tag below)

[Answer]: B

## Ambiguity 6: Model providers (Q10)
"Claude Pro for testing" and "open-weight as first class" need defining. Background: Anthropic currently
allows a Claude Pro/Max subscription to power third-party tools only through its Agent SDK / Claude Code
(`claude -p`), running on the server under your own login and drawing on a separate monthly Agent SDK
credit allowance. Using subscription OAuth tokens directly in another app is not allowed.

### Clarification Question 7
Which model provider adapters should the MVP include?

A) OpenAI-compatible API (covers Ollama, LM Studio, vLLM, llama.cpp, OpenRouter and OpenAI) + Anthropic API (API key) + optional "Claude Code / Agent SDK" adapter for personal testing on your subscription

B) OpenAI-compatible API + Anthropic API only (API keys)

C) OpenAI-compatible API only (local models first; Claude via an OpenAI-compatible gateway)

X) Other (please describe after [Answer]: tag below)

[Answer]: A

### Clarification Question 8
Where are model settings configured?

A) Per instance: an admin sets up providers and models once; sessions pick from them

B) Per instance, with per-session choice of which model plays the orchestrator vs. thread agents

C) Per user: each user brings their own keys and models

X) Other (please describe after [Answer]: tag below)

[Answer]: B with predefined defaults set by the admin.

## Ambiguity 7: Web search for thread agents (Q5)

### Clarification Question 9
How should agents search the web in a self-hosted setup?

A) Self-hosted SearXNG container bundled in Docker Compose (no API keys)

B) Bring-your-own search API key (e.g. Brave Search, Tavily)

C) Pluggable search interface: SearXNG by default, API providers optional

D) Defer web search until after the MVP

X) Other (please describe after [Answer]: tag below)

[Answer]: C

## Ambiguity 8: Back-end language (Q12)
You're curious about Rust. Trade-off: Rust is excellent for a long-running, concurrent server
(WebSockets, agent orchestration) but will slow a first demo if you're new to it. A TypeScript
back end lets you share types with the front end and move faster.

### Clarification Question 10
Which back end should Tandem use?

A) Rust (e.g. Axum + Tokio + SQLx + PostgreSQL) for the whole back end, TypeScript/React front end

B) TypeScript back end (e.g. Node/Bun with Fastify or Hono) for speed; revisit Rust later

C) Hybrid: Rust core server (API, realtime, persistence) with agent/LLM logic in a TypeScript worker service

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Ambiguity 9: Artifacts and rich text (Q4)

### Clarification Question 11
For the MVP, what should artifacts and messages support?

A) Markdown everywhere (messages, threads, artifacts) with code highlighting and Mermaid diagrams; versioned artifacts (markdown documents and code files)

B) As A, plus a rich-text (WYSIWYG) editor for artifacts

C) As A, plus rendered previews of HTML artifacts in a sandboxed frame

X) Other (please describe after [Answer]: tag below)

[Answer]: A

---

> **User note (2026-09-29):** "the resiliency questions aren't relevant to the build as this is pitched as a self hosted app, so those concerns are for the end user to decide for themselves."

# Part 2: Resiliency Extension Decisions (required by RESILIENCY rules)

These come from the enabled Resiliency Baseline and are written with cloud deployments in mind. For a
self-hosted Docker Compose MVP, the suggested defaults are noted in each heading.

## Question: RTO/RPO Goals and Disaster Recovery Strategy (suggested: A)
What are your Recovery Time Objective (RTO) and Recovery Point Objective (RPO) goals? These determine the appropriate Disaster Recovery strategy and infrastructure redundancy level.

A) RPO/RTO: Hours — Backup & Restore strategy. Lowest cost ($). Data backed up, no services deployed. Redeploy from IaC and restore from backups on failure. Suitable for non-critical workloads.

B) RPO/RTO: 10s of minutes — Pilot Light strategy. Cost: $$. Data live, services idle. Infrastructure deployed but not running, scaled up on failover. Suitable for important workloads.

C) RPO/RTO: Minutes — Warm Standby strategy. Cost: $$$. Data live, services run at reduced capacity. Scaled up during failover. Suitable for business-critical applications.

D) RPO/RTO: Near real-time — Multi-site Active/Active strategy. Highest cost ($$$$). Data live, live services in multiple regions simultaneously. Suitable for mission-critical, zero-downtime requirements.

E) N/A — Single-region deployment is acceptable, no cross-region DR needed. Rely on multi-zone availability within one region.

X) Other (please describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Change Management Process (suggested: B)
How should production changes for this workload be governed? AI-DLC will conform the design to your answer rather than inventing a process.

A) Use our existing organizational change management process — provide the name/tool (e.g., ServiceNow, Jira Change, internal CAB). AI-DLC will reference it and ensure deployable artifacts fit that process (change records, approval gates).

B) No formal process exists yet — AI-DLC should propose a lightweight change management process (change record + approval + rollback note) for the team to adopt.

C) N/A — this workload is exempt from formal change management (e.g., internal tooling). Document the exemption rationale.

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: CI/CD and Deployment Tooling (suggested: B, GitHub Actions)
What CI/CD tooling and deployment process should this workload use?

A) Use our existing CI/CD pipeline — provide the tool (e.g., GitHub Actions, GitLab CI, Jenkins, CodePipeline). AI-DLC will produce artifacts compatible with it.

B) No pipeline exists — AI-DLC should propose a CI/CD pipeline definition appropriate to the chosen IaC and runtime.

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Rollback Mechanism (suggested: A, plus D noted for database migrations)
How should a failed production deployment be rolled back?

A) Redeploy previous IaC/artifact version (version-pinned rollback)

B) Blue/green swap back to the previous environment

C) Canary auto-rollback on health/metric regression

D) Database-aware rollback required (schema/data migration reversal) — flag for explicit design

E) Use our organization's existing rollback procedure — provide reference

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Deployment Style (suggested: A)
What deployment strategy is acceptable for this workload's risk profile?

A) Direct / in-place (lowest cost, highest blast radius) — acceptable for non-critical workloads

B) Rolling (gradual instance replacement)

C) Blue/green (zero-downtime cutover, higher cost)

D) Canary (progressive traffic shift with automated rollback)

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Regional Topology (suggested: X, "single host for MVP, stateless services so it can scale out later")
Does this workload require multi-region deployment, or is single-region with multi-zone redundancy sufficient?

A) Single-region, multi-zone — tolerates zone failure, not full-region failure. Lower cost. (Aligns with RTO/RPO options A/B/E.)

B) Multi-region active-passive — survives region failure with failover. Higher cost. (Aligns with Warm Standby / Pilot Light cross-region.)

C) Multi-region active-active — survives region failure with no downtime. Highest cost. (Aligns with Active/Active.)

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Resiliency Testing Approach (suggested: C)
How will resiliency mechanisms (failover, recovery) be validated?

A) Use our existing DR testing / game day / chaos engineering practice — provide the reference. AI-DLC will document test scenarios that fit it.

B) No practice exists — AI-DLC should propose a DR testing schedule and chaos experiment plan for adoption.

C) Defer to the Operations phase — capture test scenarios now, execute during Operations.

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.

## Question: Incident Response Process (suggested: B)
How are production incidents handled for this workload?

A) Use our existing incident response process — provide the reference (e.g., PagerDuty runbooks, internal IR/on-call process). AI-DLC will align alerting and runbooks to it.

B) No formal process exists — AI-DLC should propose a lightweight incident response and Correction of Errors (COE) process for adoption.

X) Other (describe after [Answer]: tag below)

[Answer]: X) Not applicable to the build: Tandem is a self-hosted app, so this is the end user's (operator's) decision.
