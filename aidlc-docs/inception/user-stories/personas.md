# Tandem: Personas

Four human personas, at brief depth (Q3). **Session Owner** is a per-session role that any member can hold, so it is its own persona. The **Orchestrator** and **thread/task agents** are not personas; they appear as actors only in system stories (see `stories.md`, Q8).

Codes used in `stories.md`: **ADM**, **OWN**, **NTM**, **DEV**.

---

## ADM: Instance Admin (Priya)

| | |
|---|---|
| **Role** | Installs and configures a Tandem instance for the team; manages users, model providers, search, and the GitHub connector |
| **Technical level** | High. Comfortable with Docker Compose, environment variables and API keys |
| **Goals** | A secure instance running in minutes; the team can use local or hosted models; control over who has access and what data leaves the building |
| **Frustrations** | Tools that need a vendor account, hide where data is sent, or break silently when a provider is down |
| **Matters most** | Admin settings, providers and connectors, security audit log, backup/restore, MFA |

## OWN: Session Owner (Marcus)

| | |
|---|---|
| **Role** | Starts a session, chooses the workspace, invites members, and steers the orchestrator. Any member can be an owner of their own session |
| **Technical level** | Medium to high. Often a team lead or developer, but the role does not require it |
| **Goals** | Turn a vague prompt into an agreed plan and shipped work without endless meetings; keep control of what the agents are allowed to touch |
| **Frustrations** | AI tools that act before the team agrees; losing the reasoning behind a decision |
| **Matters most** | Session creation, workspace choice, membership, model choice, plan and output approval, committing and ending sessions |

## NTM: Non-technical Member (Lena)

| | |
|---|---|
| **Role** | Product manager, designer, or domain expert who joins sessions to answer questions, review plans and approve outputs |
| **Technical level** | Low. Never uses Git or a terminal; uses a phone as often as a laptop |
| **Goals** | Understand what is being proposed in plain language; contribute and approve without needing a developer to translate |
| **Frustrations** | Jargon, diffs that are unreadable to them, and having to ask someone else to run the AI for them |
| **Matters most** | Threads and chat, `@ask` / `@search` / `@draft`, reviewing artifacts, approvals, notifications, mobile use |

## DEV: Developer Member (Sam)

| | |
|---|---|
| **Role** | Engineer who works in a repo workspace, analyses code with agents, reviews code changes and commits them |
| **Technical level** | High. Lives in Git and code review |
| **Goals** | Use agents to understand and change a codebase while keeping every change reviewable and nothing committed without explicit approval |
| **Frustrations** | Agents that rewrite files unchecked, run arbitrary commands, or lose track of where a change came from |
| **Matters most** | `@code`, extra-repo access requests, task diffs, rework with feedback, commit/push, pull requests |
