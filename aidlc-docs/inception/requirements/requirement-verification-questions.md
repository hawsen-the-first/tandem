# Requirements Clarification Questions: Tandem

Tandem is a **shared workspace where a team and AI agents work on the same things together in real time**.
Please answer each question by filling in the letter after the `[Answer]:` tag. If no option fits,
choose the last one (Other) and describe what you want after the tag.

---

## Section A: Business Context

## Question 1
What is the main purpose of building Tandem?

A) A commercial SaaS product sold to teams

B) An internal tool for my own team or company

C) A portfolio or learning project to showcase skills

D) An open-source project others can self-host

X) Other (please describe after [Answer]: tag below)

[Answer]: D

## Question 2
Who is the primary target user?

A) Software development teams

B) Product, design and marketing teams (non-technical knowledge workers)

C) Small businesses and freelancers working with clients

D) Students and researchers

X) Other (please describe after [Answer]: tag below)

[Answer]: X) Any of the above, it should support teams working with AI on shared work in any context and plugin to existing workflows/tools where sensible.

## Question 3
What does a successful first release (MVP) look like?

A) A working demo I can show people, with one polished core workflow

B) A small group of real users (a pilot team) using it every week

C) Public launch with sign-up and at least a free tier

X) Other (please describe after [Answer]: tag below)

[Answer]: A) Working demo of core functionality would be great.

---

## Section B: Functional Scope

## Question 4
What is the main thing people and AI collaborate on inside a workspace?

A) Rich-text documents (like a shared doc editor with AI co-authors)

B) Tasks and projects (like a board where AI agents pick up and complete tasks)

C) A mix: documents plus tasks linked together

D) Code and technical artifacts

E) A freeform canvas or whiteboard

X) Other (please describe after [Answer]: tag below)

[Answer]: X) I think primarily text, code and artifacts to begin with, with rich  text or at least md if it's a small lift for the MVP.

## Question 5
How should AI take part in the workspace?

A) On request only: a person asks the AI to draft, summarize or edit, and reviews the result

B) As teammates: named AI agents with roles that can be assigned work and act on their own, with human approval before changes land

C) Both: an on-demand assistant plus assignable agent teammates

X) Other (please describe after [Answer]: tag below)

[Answer]: X) I see three levels: at a high level theres a session orchestrator agent. You would start a session with an initial prompt. This would trigger questions much like this current process, with each question allowing a thread for discussion. Inside the thread, a shortlived agent could be called with an @ command or similar to ask questions, perform code analysis, search the web etc. The thread would create the collaboration context to answer the question. Then there should be a configurable voting/approval system that marks a thread as completed. Then the orchestration agent can take this context and spawn tasks to implement what was agreed in the thread.

## Question 6
How "real time" does collaboration need to be for the MVP?

A) Live co-editing with presence (cursors, who's online), like Google Docs

B) Near real time: changes appear within a few seconds, no live cursors

C) Asynchronous: comments, activity feed and notifications are enough

X) Other (please describe after [Answer]: tag below)

[Answer]: X) Think like slack. More of a messaging/threads approach with some structure that supports cowork around a problem.

## Question 7
Which of these are in scope for the MVP? (List all letters that apply, e.g. "A, C, D")

A) Comments and threaded discussion on content

B) Version history, with the ability to review and undo AI changes

C) Workspace chat (team + AI in one channel)

D) Notifications (in-app and/or email)

E) Integrations with outside tools (Slack, GitHub, Google Drive, etc.)

F) File uploads that AI can read as context

X) Other (please describe after [Answer]: tag below)

[Answer]: A, C, F

## Question 8
How are users and teams organized?

A) Single team per account, invite members by email

B) Organizations containing multiple workspaces, with roles (owner, editor, viewer)

C) Individual users who share specific items with others

X) Other (please describe after [Answer]: tag below)

[Answer]: X) for MVP just a single team, with users added to sessions. We can address more complex RBAC and organisations of teams/projects etc later.

## Question 9
How should users sign in?

A) Email and password

B) Social login (Google, GitHub)

C) Magic link (passwordless email)

D) Enterprise SSO (SAML/OIDC) in addition to one of the above

X) Other (please describe after [Answer]: tag below)

[Answer]: A and/or C. But the intention is selfhosted, so probably require somekind of email server for the latter.

---

## Section C: AI and Technical Context

## Question 10
Which AI model provider(s) should Tandem use?

A) Anthropic Claude only

B) Amazon Bedrock (access to Claude and other models through AWS)

C) Multiple providers with a pluggable model layer (Claude, OpenAI, etc.)

D) Bring-your-own API key per workspace

X) Other (please describe after [Answer]: tag below)

[Answer]: X) If we can integrate with claude pro that is useful for testing. Also supporting opensource/openweight needs to be a first class citizen (i.e. ollama/LM Studio etc)

## Question 11
What platform should the MVP run on?

A) Web app (desktop browsers)

B) Web app that also works well on mobile browsers

C) Desktop app (Electron/Tauri) plus web

X) Other (please describe after [Answer]: tag below)

[Answer]: B) MVP can be browser based with mobile/native web app support ideally. We can look at dedicated apps/clients later.

## Question 12
Do you have a preferred tech stack?

A) TypeScript end to end (e.g. Next.js/React front end, Node back end, PostgreSQL)

B) React front end with a Python back end (e.g. FastAPI)

C) No preference: recommend one during Workflow Planning / NFR stages

X) Other (please describe after [Answer]: tag below)

[Answer]: X) Definitely typescript front end. Open for backend suggestions, but I have been curious about Rust so this could be a good way to dip my toes.

## Question 13
Where should Tandem be hosted?

A) AWS (e.g. ECS/Lambda, RDS, S3)

B) Managed platforms (e.g. Vercel + Supabase/Neon)

C) Self-hosted with Docker Compose for now

D) Local only for the MVP; decide hosting later

X) Other (please describe after [Answer]: tag below)

[Answer]: C) Docker is an easy starting point and suits my own selfhosted stack for testing.

---

## Section D: Non-Functional Requirements

## Question 14
What scale should the MVP be designed for?

A) Tiny: a single team (under 20 users)

B) Small: tens of teams (hundreds of users)

C) Growth-ready: thousands of users, designed to scale horizontally

X) Other (please describe after [Answer]: tag below)

[Answer]: X) Realistically we don't need mega scale, but I don't want to build myself into a corner. Building with scalability in mind makes sense.

## Question 15
How sensitive is the content people will put in Tandem?

A) Low: general work notes, no special handling needed

B) Moderate: business-confidential; needs encryption, access controls and clear AI data-use policy

C) High: regulated data (health, finance, personal data under GDPR/NZ Privacy Act), needs audit logs and data residency

X) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 16
How should AI usage costs be controlled?

A) Not a concern for the MVP

B) Per-workspace usage limits and a usage dashboard

C) Usage-based billing passed through to customers

X) Other (please describe after [Answer]: tag below)

[Answer]: X) it should be a BYO model design for now. So billing is a seperate concern.

---

## Section E: AI-DLC Extensions

## Question: Security Extensions
Should security extension rules be enforced for this project?

A) Yes — enforce all SECURITY rules as blocking constraints (recommended for production-grade applications)

B) No — skip all SECURITY rules (suitable for PoCs, prototypes, and experimental projects)

X) Other (please describe after [Answer]: tag below)

[Answer]: A) Security: yes security is important.

## Question: Resiliency Extensions
Should the resiliency baseline be applied to this project?

**What this extension is.** Enabling it applies a set of **directional, design-time best practices** for building resilient systems, derived from the **AWS Well-Architected Framework (Reliability Pillar)** and resilience-review guidance. It steers requirements, design, and code toward fault tolerance, high availability, observability, and recoverability — covering 15 practice areas across business goals, change management, observability, high availability, disaster recovery, and continuous improvement.

**What this extension is NOT.** Enabling it does **not** make your workload production-ready, nor does it certify or guarantee any availability, RTO, or RPO target. It is a **starting point** that scaffolds good resiliency decisions early — it is not a substitute for a formal **AWS Well-Architected Review** of the built system.

Treat the output as a well-grounded **first draft of your resiliency posture** to build on and validate — not a finished, production-certified result.

A) Yes — apply the resiliency baseline as directional best practices and design-time guidance (recommended for business-critical workloads, as an informed starting point that you can validate and harden before go-live)

B) No — skip the resiliency baseline (suitable for PoCs, prototypes, and experimental projects where rapid iteration matters more than reliability)

X) Other (please describe after [Answer]: tag below)

[Answer]: A) And yes let's use resiliency.

## Question: Property-Based Testing Extension
Should property-based testing (PBT) rules be enforced for this project?

A) Yes — enforce all PBT rules as blocking constraints (recommended for projects with business logic, data transformations, serialization, or stateful components)

B) Partial — enforce PBT rules only for pure functions and serialization round-trips (suitable for projects with limited algorithmic complexity)

C) No — skip all PBT rules (suitable for simple CRUD applications, UI-only projects, or thin integration layers with no significant business logic)

X) Other (please describe after [Answer]: tag below)

[Answer]: C) No property based testing.
