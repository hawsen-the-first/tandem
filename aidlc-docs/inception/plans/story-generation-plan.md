# Story Generation Plan: Tandem

**Stage**: INCEPTION - User Stories, Part 1 (Planning)
**Inputs**: `aidlc-docs/inception/requirements/requirements.md` (approved), `user-stories-assessment.md`
**Role**: Product owner

Please answer every question by writing the letter after its `[Answer]:` tag (use the last option, "Other", to describe anything different). All questions must be answered before the plan can be approved. Your answers form the audit trail for how the stories are shaped.

---

## Story Breakdown Options (for reference, see Question 1)

| Approach | How stories are organised | Trade-offs |
|---|---|---|
| **User Journey-Based** | Follows a persona through a workflow (e.g. start session, resolve questions, approve plan, review output) | Mirrors the product's core loop and doubles as a demo script; cross-cutting needs (auth, admin) can be missed |
| **Feature-Based** | One group per FR area (auth, threads, agents, plan, tasks, ...) | Maps 1:1 to requirements and units; loses the end-to-end narrative |
| **Persona-Based** | Grouped by Admin, Owner, Member | Clarifies who needs what; duplicates stories that several personas share |
| **Epic-Based** | Epics containing smaller stories | Good for sizing and sequencing; adds hierarchy overhead |
| **Hybrid (recommended)** | Epics follow the core-loop journey; each epic holds feature-level stories tagged with persona and FR IDs | Best traceability and narrative; slightly more documentation |

---

## Questions

## Question 1
Which breakdown approach should organise the stories?

A) Hybrid: epics ordered along the core-loop journey (session, questions, threads, plan, tasks, output), with feature-level stories inside each epic, each tagged with persona and FR IDs

B) Feature-based only (one epic per FR group, in requirements order)

C) User journey-based only (one continuous narrative per persona)

D) Persona-based only (Admin, Owner, Member sections)

E) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 2
Which personas should be created?

A) Three, matching the requirements: Instance Admin, Session Owner, Session Member

B) The three above plus a distinct Self-Hosting Operator (installs, upgrades, backups; may be a different person from the Admin)

C) The three above plus the AI agents (Orchestrator, Thread Agent) as non-human actors with their own profiles

D) All of B and C

E) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 3
How detailed should each persona be?

A) Brief: role, goals, main frustrations, technical level (about half a page)

B) Rich: the above plus typical day, the tools they use today, and a short narrative scenario

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4
The requirements say Tandem suits "any team, technical or not". Should the personas reflect that?

A) Yes: make Session Member explicitly non-technical by default and add a note on a technical variant (e.g. a developer using `@code`)

B) Yes: keep two member variants as separate personas (Non-technical Member and Developer Member)

C) No: keep a single generic Session Member

D) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 5
What story size should we aim for?

A) Small: each story is one user-visible capability that a team could build and demo in a few days (expect roughly 60 to 80 stories)

B) Medium: each story covers a short flow (expect roughly 35 to 50 stories)

C) Coarse: one story per requirement group (expect roughly 15 to 20 stories)

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
Which story template should be used?

A) "As a [persona], I want [capability], so that [benefit]" plus ID, epic, persona, FR references and priority

B) Job-story style: "When [situation], I want to [motivation], so I can [outcome]"

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
Which acceptance criteria format should be used?

A) Given/When/Then scenarios (2 to 5 per story, including at least one failure or edge case where relevant)

B) A plain checklist of verifiable statements

C) Given/When/Then for flow-heavy stories (approvals, agents, tasks, workspace) and a checklist for simple ones (settings, admin screens)

D) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 8
How should agent behaviour be handled in stories?

A) Humans are always the story actor; agent behaviour (what `@ask`, the orchestrator or a task agent does) is described in the acceptance criteria

B) Add a separate set of system stories with the Orchestrator or an agent as the actor ("As the orchestrator, I propose questions, so that ...")

C) Humans are the actor for user-triggered behaviour; use system stories only for autonomous orchestrator behaviour (question rounds, plan drafting, task spawning)

D) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 9
Should security, resiliency and operations requirements (NFR-4, NFR-5, NFR-2) get their own stories?

A) Yes: operator and admin stories for self-hosting, backup/restore, health checks, security audit log, and similar, alongside the user-facing stories

B) Only where a person directly interacts with the capability (MFA enrolment, audit log viewer, backup command); other NFRs stay as constraints referenced in acceptance criteria

C) No: keep NFRs in requirements.md and reference them from stories where relevant

D) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 10
How should priority be shown within the MVP?

A) No priority tags: everything in requirements.md is MVP

B) MoSCoW tags (Must/Should/Could) so Workflow Planning and Units Generation can sequence work and cut scope if needed

C) Two tiers: "Core loop" (needed for the demo in goal G1) and "Supporting" (everything else)

D) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 11
Should stories trace back to requirement IDs?

A) Yes: every story lists its FR/NFR IDs, and the generated stories.md ends with a coverage table showing each FR mapped to at least one story

B) Yes, but only a short FR reference per story, with no coverage table

C) No traceability

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 12
Should the stories include a reference end-to-end scenario?

A) Yes: one worked example (e.g. a small team using Tandem to plan a feature in a GitHub repo) that threads through the epics and can serve as the MVP demo script

B) Yes: two scenarios, one repo-based (code) and one artifact-only (a non-technical team writing a document)

C) No reference scenario

D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Execution Checklist (Part 2: Generation, runs after the plan is approved)

### Phase A: Personas
- [x] A1. Draft `personas.md` with four personas (Instance Admin, Session Owner, Non-technical Member, Developer Member; see Clarification 1) at brief depth (Q3)
- [x] A2. For each persona record goals, frustrations, technical level, and which epics matter most to them

### Phase B: Epics and stories
- [x] B1. Define the epic structure per Q1, covering every FR group in requirements.md
- [x] B2. Write stories at the granularity chosen in Q5 using the template from Q6
- [x] B3. Apply the agent-behaviour convention from Q8
- [x] B4. Add operator/admin and security stories per Q9
- [x] B5. Write acceptance criteria in the format chosen in Q7, including failure and edge cases
- [x] B6. Tag each story with persona, epic, FR/NFR references (Q11) and priority (Q10)

### Phase C: Quality checks
- [x] C1. Check every story against INVEST (Independent, Negotiable, Valuable, Estimable, Small, Testable)
- [x] C2. Verify every MVP FR group is covered by at least one story (coverage table if Q11 = A)
- [x] C3. Verify assumptions A1 to A5 and the "no command execution" and "agents cannot approve" rules appear in the relevant acceptance criteria
- [x] C4. Map personas to stories (persona-to-story matrix at the end of `stories.md`)

### Phase D: Scenario and finish
- [x] D1. Write the reference scenario(s) per Q12 and link each step to story IDs
- [x] D2. Validate any Mermaid or ASCII diagrams per `common/content-validation.md`
- [x] D3. Save `aidlc-docs/inception/user-stories/stories.md` and `personas.md`; update `aidlc-state.md`
- [x] D4. Log the approval prompt in `audit.md` and present the completion message

### Extension compliance (Part 1)
- Security Baseline: N/A at this stage (no code or infrastructure); security requirements are carried into acceptance criteria via Q9
- Resiliency Baseline: N/A at this stage; operator stories per Q9
- Property-Based Testing: disabled

---

## Clarification 1 (resolved)
Q2 A (three personas, single Session Member) conflicted with Q4 B (separate Non-technical and Developer Member personas).
**Resolution**: four personas: Instance Admin, Session Owner, Non-technical Member, Developer Member. Session Owner remains a distinct persona because it is a per-session role that any member can hold.

## Approved methodology summary
Hybrid epics along the core-loop journey; four personas at brief depth; roughly 60 to 80 small stories in As a / I want / So that form with checklist acceptance criteria; humans as actors except autonomous orchestrator behaviour (system stories); NFR stories only where a person interacts; MoSCoW priority; FR/NFR tags plus a coverage table; one end-to-end reference scenario.
