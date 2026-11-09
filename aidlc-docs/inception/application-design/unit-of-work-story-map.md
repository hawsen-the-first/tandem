# Tandem: Unit of Work to Story Map

Every one of the 87 stories in `stories.md` is assigned to **exactly one** unit: the unit in which the story is first fully demonstrable and its acceptance checklist can be passed. Priority is the story's first-listed MoSCoW priority.

## Summary

| Unit | Name | Stories | Must | Should | Could |
|---|---|---|---|---|---|
| U1 | Foundation and walking skeleton | 10 | 6 | 4 | 0 |
| U2 | Instance administration | 6 | 5 | 1 | 0 |
| U3 | Collaboration core | 21 | 12 | 9 | 0 |
| U4 | Agent runtime and question rounds | 15 | 9 | 6 | 0 |
| U5 | Workspaces and GitHub | 7 | 5 | 2 | 0 |
| U6 | Plan, tasks and artifacts | 20 | 16 | 4 | 0 |
| U7 | Container sandbox runner | 1 | 0 | 1 | 0 |
| U8 | Hardening and delivery | 4 | 0 | 4 | 0 |
| U9 | Backlog (Could-have) | 3 | 0 | 0 | 3 |
| **Total** | | **87** | **53** | **31** | **3** |

## Assignments

### U1 Foundation and walking skeleton

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-1.1 | First-run admin setup | ADM | Must |
| US-1.2 | Sign in with email and password | OWN, NTM, DEV, ADM | Must |
| US-1.3 | Invite users | ADM | Must |
| US-1.4 | Deactivate and remove users | ADM | Must |
| US-1.5 | Multi-factor authentication | ADM, OWN, NTM, DEV | Must |
| US-1.6 | Change my password | OWN, NTM, DEV | Should |
| US-1.7 | Force a password reset | ADM | Should |
| US-2.7 | View the security audit log | ADM | Must |
| US-13.3 | Lockout after repeated failed sign-ins | ADM, OWN, NTM, DEV | Should |
| US-13.7 | Use Tandem on a phone and in dark mode | NTM, OWN, DEV | Should |

### U2 Instance administration

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-2.1 | Configure an OpenAI-compatible provider | ADM | Must |
| US-2.3 | Test a provider connection | ADM | Must |
| US-2.4 | Set default models | ADM | Must |
| US-2.5 | Configure web search | ADM | Should |
| US-2.6 | Configure the GitHub connector | ADM | Must |
| US-2.9 | Configure the sandbox policy | ADM | Must |

### U3 Collaboration core

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-3.1 | Create a session | OWN | Must |
| US-3.3 | Use a managed folder as the workspace | OWN, NTM | Must |
| US-3.4 | Add members to a session | OWN | Must |
| US-3.5 | Members-only access and workspace isolation | OWN, NTM, DEV | Must |
| US-3.7 | Session overview | OWN, NTM, DEV | Must |
| US-5.1 | Post and read messages in a thread | OWN, NTM, DEV | Must |
| US-5.2 | Session chat channel | OWN, NTM, DEV | Should |
| US-5.3 | Reply to and comment on a message | OWN, NTM, DEV | Should |
| US-5.4 | Reference artifacts, files and other threads | OWN, NTM, DEV | Should |
| US-5.5 | Upload files as context | NTM, DEV, OWN | Should |
| US-5.6 | See thread status | OWN, NTM, DEV | Must |
| US-7.1 | Approve a thread | OWN, NTM, DEV | Must |
| US-7.2 | See who approved and when | OWN, NTM, DEV | Must |
| US-7.3 | Revoke an approval | OWN, NTM, DEV | Should |
| US-11.4 | End a session | OWN | Must |
| US-11.6 | Ended and archived sessions are read-only | OWN, NTM, DEV | Must |
| US-12.1 | Be notified when mentioned | OWN, NTM, DEV | Should |
| US-12.2 | Be notified when something needs me | OWN, NTM, DEV | Should |
| US-12.3 | See presence and unread | OWN, NTM, DEV | Should |
| US-13.1 | Clear limits on messages and uploads | NTM, DEV, OWN | Should |
| US-13.2 | Safe display of untrusted content | NTM, OWN, DEV | Must |

### U4 Agent runtime and question rounds

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-3.6 | Choose models for a session | OWN | Should |
| US-4.1 | Orchestrator proposes a question round (system) | OWN, NTM, DEV (beneficiaries) | Must |
| US-4.2 | Questions become threads (system) | OWN, NTM, DEV (beneficiaries) | Must |
| US-4.3 | Edit proposed questions | OWN, NTM, DEV | Should |
| US-4.4 | Orchestrator decides whether another round is needed (system) | OWN, NTM, DEV (beneficiaries) | Should |
| US-4.5 | Request another round | OWN, NTM, DEV | Should |
| US-5.7 | Thread outcome summary (system) | OWN, NTM, DEV (beneficiaries) | Must |
| US-5.8 | Edit the outcome summary before approving | OWN, NTM, DEV | Must |
| US-6.1 | Choose an agent with `@` | OWN, NTM, DEV | Must |
| US-6.2 | Ask a question with `@ask` | NTM, OWN, DEV | Must |
| US-6.4 | Search the web with `@search` | NTM, OWN, DEV | Must |
| US-6.5 | Draft with `@draft` | NTM, OWN | Should |
| US-6.6 | Watch agent output and cancel it | NTM, OWN, DEV | Must |
| US-6.7 | See what an agent did | DEV, OWN, NTM | Should |
| US-6.8 | Agents answer but do not change the workspace | OWN, DEV | Must |

### U5 Workspaces and GitHub

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-3.2 | Use a GitHub repo as the workspace | OWN, DEV | Must |
| US-6.3 | Analyse code with `@code` | DEV, OWN | Must |
| US-6.9 | Request and approve access to another repo | DEV, OWN | Should |
| US-11.1 | Changes stay in the worktree until committed | OWN, DEV | Must |
| US-11.2 | Commit and push to the session branch | DEV, OWN | Must |
| US-11.3 | Open a pull request | DEV, OWN | Should |
| US-11.5 | Workspace is cleaned up when a session ends | OWN, ADM | Must |

### U6 Plan, tasks and artifacts

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-8.1 | Orchestrator drafts the plan (system) | OWN, NTM, DEV (beneficiaries) | Must |
| US-8.2 | Read the plan | NTM, OWN, DEV | Must |
| US-8.3 | Discuss a plan step | OWN, NTM, DEV | Must |
| US-8.4 | Request a plan rework | OWN, NTM, DEV | Must |
| US-8.5 | Trace plan steps back to decisions | OWN, NTM, DEV | Should |
| US-8.6 | Approve the plan | OWN, NTM, DEV | Must |
| US-8.7 | No work before the plan is approved | OWN | Must |
| US-9.1 | Orchestrator spawns tasks (system) | OWN, DEV (beneficiaries) | Must |
| US-9.2 | Task agents create artifacts and edit workspace files | OWN, DEV, NTM | Must |
| US-9.3 | Task agents cannot run commands | OWN, ADM | Must |
| US-9.4 | Track task progress | OWN, NTM, DEV | Must |
| US-9.5 | Review code changes as a diff | DEV, OWN | Must |
| US-9.6 | Review a new artifact version | NTM, OWN | Must |
| US-9.7 | Approve task output | OWN, NTM, DEV | Must |
| US-9.8 | Reject output with feedback or discard it | OWN, NTM, DEV | Must |
| US-9.9 | Retry a failed task | OWN, DEV | Should |
| US-10.1 | Read rendered artifacts | NTM, OWN, DEV | Must |
| US-10.2 | Version history and comparison | OWN, NTM, DEV | Must |
| US-10.3 | Edit an artifact | NTM, OWN, DEV | Should |
| US-10.4 | Download an artifact | NTM, OWN, DEV | Should |

### U7 Container sandbox runner

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-2.11 | Run agents in container sandboxes | ADM, OWN | Should |

### U8 Hardening and delivery

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-2.2 | Configure the Anthropic API provider | ADM | Should |
| US-13.4 | Back up and restore | ADM | Should |
| US-13.5 | Know what data goes to which model provider | ADM, OWN | Should |
| US-13.6 | Understand degraded service | NTM, OWN, DEV | Should |

### U9 Backlog (Could-have)

| Story | Title | Personas | Priority |
|---|---|---|---|
| US-2.8 | See token usage per session | ADM, OWN | Could |
| US-2.10 | Configure the optional Agent SDK provider | ADM | Could |
| US-10.5 | Save an artifact into the workspace | DEV, OWN | Could |

## Assignment notes
- **Skeleton stubs are not story completion.** U1's walking skeleton includes a thin session, a posted message, and minimal queue, outbox and event bus so the path web, API, database, worker and WebSocket is proven. Stories US-3.1, US-5.1 and others are still completed in U3 and U4.
- **Acceptance bullets that depend on a later unit** stay on their story and are verified when that unit lands:
  - US-7.3 (revoke blocked when later steps depend on the thread): plan and task dependencies arrive in U6.
  - US-11.4 (warn on uncommitted changes): the workspace change check arrives in U5; U3 delivers the end-session action and confirmation for managed folders.
  - US-12.2 (notifications for plan ready and task output ready): those events arrive in U6.
  - US-5.7 and US-5.8 (outcome summary): U4; until then U3 approvals work without a generated summary.
  - US-2.9 (sandbox policy): U2 stores and validates the policy; enforcement is by the in-process runner in U4; the container option is delivered by US-2.11 in U7.
- **US-13.7 (mobile, themes, WCAG)** gets its baseline in U1's web shell; every later unit's UI must conform and is checked in its own Build and Test.
- **U7** owns only US-2.11 but also extends the enforcement tests for US-6.8 and US-9.3.
- **US-2.2** was split into US-2.2 (Anthropic API, Should, U8) and US-2.10 (Agent SDK, Could, U9); US-2.11 was added to give U7 an owner story.
