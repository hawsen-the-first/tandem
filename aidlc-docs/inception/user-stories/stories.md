# Tandem: User Stories (MVP)

**Method**: Hybrid. Epics follow the core-loop journey; feature-level stories sit inside each epic (Q1).
**Format**: As a / I want / So that, with checklist acceptance criteria (Q6, Q7). Each story is tagged with persona, requirement IDs and MoSCoW priority (Q10, Q11).
**Actors**: Humans are the actor unless the behaviour is autonomous orchestrator work, which uses system stories marked **(system)** (Q8). Personas are defined in `personas.md`: **ADM**, **OWN**, **NTM**, **DEV**.
**Scope of NFR stories**: only where a person directly interacts with the capability (Q9). Other NFRs remain constraints in `requirements.md`.
**Standing rules that apply to every story**: agents never approve anything (FR-3.5); agents never run commands in the MVP (FR-8.3); every server action is authorised against session membership or admin role (SECURITY-08).

---

## E1. Accounts and access

### US-1.1 First-run admin setup
**As an** Instance Admin, **I want** to create the first admin account when I first open a new instance, **so that** the instance is secured from the start.
**Personas**: ADM · **Reqs**: FR-1.2, SECURITY-09 · **Priority**: Must
- [ ] On a fresh instance with no users, the first visit shows a setup screen
- [ ] The setup screen creates an account with the admin role and then disappears for good
- [ ] There is no default password and the setup screen cannot be reached once an admin exists
- [ ] The password meets the password policy (US-1.2)
- [ ] The admin must enrol MFA (US-1.5) immediately: nothing else is reachable until it is done

### US-1.2 Sign in with email and password
**As a** member, **I want** to sign in with my email and password, **so that** I can reach my sessions.
**Personas**: OWN, NTM, DEV, ADM · **Reqs**: FR-1.1, FR-1.8, SECURITY-12 · **Priority**: Must
- [ ] Valid credentials sign me in; invalid credentials show a generic error that does not reveal whether the email exists
- [ ] A password must be at least 10 characters with upper case, lower case, a digit and a symbol, and must not appear in the breached-password list
- [ ] Passwords are stored with Argon2id
- [ ] Session cookies are Secure, HttpOnly and SameSite, with a server-side expiry
- [ ] Deactivated users cannot sign in

### US-1.3 Invite users
**As an** Instance Admin, **I want** to invite people and give them the member or admin role, **so that** my team can join.
**Personas**: ADM · **Reqs**: FR-1.3, FR-1.4 · **Priority**: Must
- [ ] An invitation creates a single-use link that expires (7 days) and lets the invitee set a password; it is shown once for the admin to copy and send, because the MVP sends no email
- [ ] The two roles available are admin and member; no other roles exist in the MVP
- [ ] The instance is a single team: invited users all belong to the same instance with no organisation concept
- [ ] The user list shows pending invitations and can revoke them

### US-1.4 Deactivate and reactivate users
**As an** Instance Admin, **I want** to deactivate a user and reactivate them later, **so that** people who leave lose access immediately and returning people can be restored.
**Personas**: ADM · **Reqs**: FR-1.3, FR-1.7, SECURITY-14 · **Priority**: Must
- [ ] Deactivation ends the user's active logins at once and blocks sign-in
- [ ] A deactivated user's past messages, approvals and artifacts remain attributed to them
- [ ] Deactivation requires confirmation and is recorded in the audit log
- [ ] Reactivation restores sign-in with the user's existing password and MFA; their earlier logins stay ended
- [ ] Permanent removal of users is not part of the MVP
- [ ] The last remaining admin cannot be deactivated

### US-1.5 Multi-factor authentication
**As an** Instance Admin, **I want** to enrol a TOTP authenticator, and **as a** member I want the option to, **so that** accounts are harder to take over.
**Personas**: ADM, OWN, NTM, DEV · **Reqs**: FR-1.5, SECURITY-12 · **Priority**: Must
- [ ] Enrolment shows a QR code and requires a valid code to complete
- [ ] MFA is required for admins, who must enrol immediately (nothing else is reachable until they do), and optional for members
- [ ] Sign-in asks for the code after the password when MFA is enrolled
- [ ] Recovery codes are shown once at enrolment

### US-1.6 Change my password
**As a** member, **I want** to change my own password, **so that** I can keep my account secure.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-1.6, FR-1.8 · **Priority**: Should
- [ ] Changing the password requires the current password
- [ ] The new password meets the password policy and is not in the breached-password list
- [ ] Other active logins for my account end after the change

### US-1.7 Force a password reset
**As an** Instance Admin, **I want** to force a user to reset their password, **so that** I can respond to a suspected compromise.
**Personas**: ADM · **Reqs**: FR-1.6, SECURITY-14 · **Priority**: Should
- [ ] Forcing a reset ends that user's logins and requires a new password at next sign-in
- [ ] The reset is delivered as a one-time link (valid 24 hours) shown once for the admin to copy; no email is sent
- [ ] The admin can tick "Also reset MFA", so a user who lost their device and recovery codes can be recovered; an admin whose MFA is reset must enrol again (US-1.5)
- [ ] The action is recorded in the audit log with who performed it

### US-1.8 Change a user's role
**As an** Instance Admin, **I want** to promote a member to admin or demote an admin to member, **so that** responsibilities can change without removing anyone.
**Personas**: ADM · **Reqs**: FR-1.4, FR-1.7, SECURITY-14 · **Priority**: Should
- [ ] I can change another user's role between admin and member
- [ ] The last remaining admin cannot be demoted
- [ ] A demoted admin loses admin access immediately; a promoted member must enrol MFA before using admin screens
- [ ] Every role change is recorded in the audit log

### US-1.9 Set sign-in duration and the breached-password check
**As an** Instance Admin, **I want** to set how long sign-ins last and whether passwords are also checked online against breach data, **so that** the instance matches my organisation's rules.
**Personas**: ADM · **Reqs**: FR-1.9, FR-15.1, SECURITY-14 · **Priority**: Should
- [ ] I can set the idle timeout and total duration of a sign-in (defaults 12 hours and 14 days)
- [ ] I can switch the optional online breached-password check on or off (the offline list is always used)
- [ ] Changes to sign-in duration apply to new sign-ins; existing sign-ins keep their limits
- [ ] Every change is recorded in the audit log

---

## E2. Instance administration

### US-2.1 Configure an OpenAI-compatible provider
**As an** Instance Admin, **I want** to add an OpenAI-compatible provider (Ollama, LM Studio, vLLM, OpenRouter, OpenAI) with its base URL and key, **so that** my team can use open-weight or hosted models.
**Personas**: ADM · **Reqs**: FR-9.1, FR-9.2, FR-15.1, NFR-4.2 · **Priority**: Must
- [ ] I can add a provider with a name, base URL, optional API key and list of models
- [ ] The key is stored encrypted and is never shown again or sent to the browser after saving
- [ ] A provider with no key (e.g. local Ollama) can be saved
- [ ] I can edit and remove a provider; removing one in use by a session warns me first

### US-2.2 Configure the Anthropic API provider
**As an** Instance Admin, **I want** to add an Anthropic API key, **so that** I can use Claude models.
**Personas**: ADM · **Reqs**: FR-9.1, FR-9.2 · **Priority**: Should
- [ ] The API key is stored encrypted and is never sent back to the browser after saving
- [ ] Once tested, its models appear in the default and per-session model choices
- [ ] The admin screen shows which provider each default and session uses

### US-2.3 Test a provider connection
**As an** Instance Admin, **I want** to test a provider and see what each model supports, **so that** I find problems before a session does.
**Personas**: ADM · **Reqs**: FR-9.2, FR-9.4 · **Priority**: Must
- [ ] A test action sends a small request and reports success, or a specific error (unreachable, unauthorised, unknown model)
- [ ] The result lists detected capabilities such as tool calling and context length where known
- [ ] A model that lacks a needed capability is flagged with a clear warning and is not silently used for that job

### US-2.4 Set default models
**As an** Instance Admin, **I want** to set default models for the orchestrator and for thread/task agents, **so that** owners do not have to choose each time.
**Personas**: ADM · **Reqs**: FR-9.3 · **Priority**: Must
- [ ] I can set one default for the orchestrator and one for thread/task agents
- [ ] New sessions start with these defaults pre-selected
- [ ] Changing a default does not alter models already chosen for existing sessions

### US-2.5 Configure web search
**As an** Instance Admin, **I want** to enable a search provider (bundled SearXNG by default, or an API-key provider), **so that** `@search` works.
**Personas**: ADM · **Reqs**: FR-10.1, FR-10.2 · **Priority**: Should
- [ ] SearXNG is the default and runs as an optional internal Compose service
- [ ] I can choose Brave Search or Tavily instead and store its key encrypted
- [ ] A test action confirms search works
- [ ] With no provider configured, `@search` is hidden or explains that search is not set up

### US-2.6 Configure the GitHub connector
**As an** Instance Admin, **I want** to connect GitHub with a fine-grained token or a GitHub App, **so that** sessions can clone repos, push session branches and open pull requests.
**Personas**: ADM · **Reqs**: FR-12.1, FR-12.2, FR-12.3, SECURITY-06 · **Priority**: Must
- [ ] I can enter credentials, see which repos they reach, and test the connection
- [ ] Credentials are stored encrypted and never appear in the browser, logs or model prompts
- [ ] The screen states the minimum GitHub permissions required
- [ ] GitHub is delivered through the connector interface so other connectors can be added later without core changes

### US-2.7 View the security audit log
**As an** Instance Admin, **I want** to see a log of security-relevant events, **so that** I can investigate problems.
**Personas**: ADM · **Reqs**: FR-15.2, SECURITY-14 · **Priority**: Must
- [ ] The log records sign-ins, failed sign-ins, admin changes, approvals, commits/pushes and extra-repo grants
- [ ] Each entry shows who, what, when and the affected object
- [ ] I can filter by user, event type and date range
- [ ] Entries cannot be edited or deleted from the UI
- [ ] The application cannot update or delete entries and never prunes them automatically; operators prune only with a documented command using a privileged credential

### US-2.8 See token usage per session
**As an** Instance Admin, **I want** to see token usage per session, **so that** I understand how heavily models are used.
**Personas**: ADM, OWN · **Reqs**: FR-9.5 · **Priority**: Could
- [ ] Usage per session is listed by model and by orchestrator, thread agent and task
- [ ] The screen shows no billing or cost limits (out of scope)

### US-2.9 Configure the sandbox policy
**As an** Instance Admin, **I want** to set the instance-wide sandbox policy, **so that** agents can only reach the files, tools and network I allow.
**Personas**: ADM · **Reqs**: FR-16.1, FR-16.5, FR-16.6, SECURITY-11, SECURITY-14 · **Priority**: Must
- [ ] I can choose the default runner tier: in-process, or container (offered only when a container runtime is reachable, with a test action; container behaviour is covered by US-2.11)
- [ ] I can set the tool allow-list, the network rule (default: none) and resource limits
- [ ] The command tool is shown as disabled in the MVP
- [ ] A saved change applies to runs started afterwards; runs already in progress are unaffected
- [ ] Every change is recorded in the audit log
- [ ] Concrete file paths are not edited here; they come from each session's grants (US-3.2, US-6.9)

### US-2.10 Configure the optional Agent SDK provider
**As an** Instance Admin, **I want** to enable the Claude Agent SDK provider, **so that** I can try Tandem on my own Claude login for personal testing.
**Personas**: ADM · **Reqs**: FR-9.1, FR-9.2 · **Priority**: Could
- [ ] The provider is off by default and is marked as intended for personal testing under the operator's own login
- [ ] The adapter invokes Claude Code on the server rather than bundling it
- [ ] A test action reports whether the server's Claude login is usable

### US-2.11 Run agents in container sandboxes
**As an** Instance Admin, **I want** agent runs to execute in isolated containers when I choose the container runner, **so that** operating-system limits back up the sandbox policy.
**Personas**: ADM, OWN · **Reqs**: FR-16.1, FR-16.2, FR-16.3, FR-16.4, SECURITY-06, SECURITY-11 · **Priority**: Should
- [ ] With the container runner selected, each run starts in its own container with only policy-bound paths mounted (task agents: workspace read-write; thread agents and extra repos: read-only)
- [ ] The container has no network by default, runs as a non-root user with a read-only root filesystem, and has CPU, memory and process limits
- [ ] No model or connector credentials are present inside the container
- [ ] If the container cannot start, the run fails closed with a clear reason and never falls back to the in-process runner
- [ ] Containers and scratch space are removed when the run ends

---

## E3. Sessions and workspaces

### US-3.1 Create a session
**As a** Session Owner, **I want** to create a session with a title and an initial prompt, **so that** the team can start working on a problem.
**Personas**: OWN · **Reqs**: FR-2.1, FR-2.2 · **Priority**: Must
- [ ] Any member can create a session; the creator becomes its owner
- [ ] Title and prompt are required; I must also choose a workspace (US-3.2 or US-3.3)
- [ ] After creation, the orchestrator starts the first question round (US-4.1)

### US-3.2 Use a GitHub repo as the workspace
**As a** Session Owner, **I want** to pick a GitHub repo as the session workspace, **so that** agents can analyse and, once approved, change real code.
**Personas**: OWN, DEV · **Reqs**: FR-11.1, FR-11.2, FR-12.2 · **Priority**: Must
- [ ] I can choose from repos the connector can reach and pick a base branch
- [ ] Tandem makes an isolated, temporary clone on a session-specific branch
- [ ] Host-mounted and local-machine folders are not offered; the UI does not suggest they exist
- [ ] If the clone fails, I see a clear error and the session is not created half-built

### US-3.3 Use a managed folder as the workspace
**As a** Session Owner, **I want** to choose an empty managed folder, **so that** a team can produce documents without any repo.
**Personas**: OWN, NTM · **Reqs**: FR-11.1 · **Priority**: Must
- [ ] A managed-folder workspace starts empty and needs no GitHub connection
- [ ] Task output is still versioned as artifacts
- [ ] Commit and push actions are unavailable for this workspace type

### US-3.4 Add members to a session
**As a** Session Owner, **I want** to add members from the instance's users, **so that** the right people take part.
**Personas**: OWN · **Reqs**: FR-2.2 · **Priority**: Must
- [ ] I can add and remove members from the list of active instance users
- [ ] A new member immediately sees the session and its existing threads
- [ ] Removing a member does not delete their past messages or approvals

### US-3.5 Members-only access and workspace isolation
**As a** Session Owner, **I want** only members to see a session and its files, **so that** confidential work stays confidential.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-2.3, FR-11.7, SECURITY-08 · **Priority**: Must
- [ ] A non-member cannot view, list, or fetch anything from the session, including via direct links; the response does not confirm that the session exists
- [ ] One session can never read another session's workspace files
- [ ] Authorisation is checked on the server for every request and WebSocket message

### US-3.6 Choose models for a session
**As a** Session Owner, **I want** to choose the orchestrator and agent models for my session, **so that** I can match the model to the work.
**Personas**: OWN · **Reqs**: FR-2.4, FR-9.3 · **Priority**: Should
- [ ] The selection defaults to the admin-set models and can be overridden at creation
- [ ] Only models from configured providers are offered
- [ ] The session shows which model is being used for each kind of agent

### US-3.7 Session overview
**As a** member, **I want** an overview of a session's state, **so that** I can see what has been decided and what is outstanding.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-2.7 · **Priority**: Must
- [ ] The overview shows the prompt, question rounds, thread statuses, plan status, tasks and artifacts
- [ ] Each item links to its thread, plan step or artifact
- [ ] Threads awaiting approval are called out

---

## E4. Orchestrator question rounds

### US-4.1 Orchestrator proposes a question round (system)
**As the** Orchestrator, **I propose** a round of clarifying questions from the prompt (and workspace, for a repo), **so that** the team resolves ambiguity before planning.
**Personas**: OWN, NTM, DEV (beneficiaries) · **Reqs**: FR-3.1, FR-3.5 · **Priority**: Must
- [ ] The first round starts automatically after session creation
- [ ] For a repo workspace, the proposal draws on the workspace as well as the prompt
- [ ] The orchestrator never approves threads, the plan or changes

### US-4.2 Questions become threads (system)
**As the** Orchestrator, **I open** each question as its own thread with suggested options where useful, **so that** each decision is discussed in one place.
**Personas**: OWN, NTM, DEV (beneficiaries) · **Reqs**: FR-3.2 · **Priority**: Must
- [ ] One thread per proposed question, showing the question and any multiple-choice options
- [ ] Options are suggestions only; members can answer in free text
- [ ] Threads in a round are grouped so progress through the round is visible

### US-4.3 Edit proposed questions
**As a** member, **I want** to edit, add or remove proposed questions before discussion starts, **so that** the discussion covers what matters.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-3.3 · **Priority**: Should
- [ ] I can edit the wording and options of a proposed question
- [ ] I can add my own question as a new thread and remove one that is not needed
- [ ] Changes are possible only until the first message is posted in that thread

### US-4.4 Orchestrator decides whether another round is needed (system)
**As the** Orchestrator, **I decide** whether further clarification is needed once a round is approved and explain why, **so that** planning starts only when enough is settled.
**Personas**: OWN, NTM, DEV (beneficiaries) · **Reqs**: FR-3.4 · **Priority**: Should
- [ ] When every thread in a round is approved, the orchestrator posts its decision and reasoning
- [ ] If it proposes another round, new question threads appear
- [ ] If not, it offers to draft the plan (US-8.1)

### US-4.5 Request another round
**As a** member, **I want** to request another question round, **so that** I can raise a gap the orchestrator missed.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-3.4 · **Priority**: Should
- [ ] Any member can request a round before the plan is approved
- [ ] The orchestrator proposes questions informed by the request and existing outcomes
- [ ] Requesting a round after plan approval is not offered

---

## E5. Threads and messaging

### US-5.1 Post and read messages in a thread
**As a** member, **I want** to post and read messages in near real time with Markdown formatting, **so that** we can discuss a question together.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.1, NFR-1.4 · **Priority**: Must
- [ ] Messages appear for other members within about half a second on a LAN, without refreshing
- [ ] Each message shows its author (person or agent) and timestamp
- [ ] Markdown renders safely (headings, lists, links, code blocks)

### US-5.2 Session chat channel
**As a** member, **I want** a general chat channel for the session, **so that** off-topic or cross-cutting talk does not clutter threads.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.2 · **Priority**: Should
- [ ] Each session has one chat channel visible to its members
- [ ] People and agents can post in it, and agents can be called with `@` as in threads
- [ ] Chat messages are separate from threads and do not affect approvals

### US-5.3 Reply to and comment on a message
**As a** member, **I want** to reply to a specific message, **so that** side points stay attached to what they refer to.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.3 · **Priority**: Should
- [ ] A reply appears nested under its parent message
- [ ] The parent shows a reply count
- [ ] Replies work in both threads and session chat

### US-5.4 Reference artifacts, files and other threads
**As a** member, **I want** to link artifacts, files and other threads in a message, **so that** context is one click away.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.4 · **Priority**: Should
- [ ] I can pick an artifact, workspace file or thread from a search list while writing
- [ ] The reference renders as a link only members of the session can follow
- [ ] A reference to a deleted or ended item shows a clear "unavailable" state

### US-5.5 Upload files as context
**As a** member, **I want** to upload files to a thread or chat, **so that** agents and colleagues can read them.
**Personas**: NTM, DEV, OWN · **Reqs**: FR-4.5, SECURITY-05 · **Priority**: Should
- [ ] Uploads are limited in size and rejected with a clear message when too large
- [ ] An uploaded file can be read by agents called in that thread
- [ ] Files are stored with a content hash and visible only to session members

### US-5.6 See thread status
**As a** member, **I want** to see whether a thread is open, approved or reopened, **so that** I know what still needs a decision.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.7 · **Priority**: Must
- [ ] Each thread shows one of: open, approved, reopened
- [ ] Status changes show up in the session overview and thread list without refresh

### US-5.7 Thread outcome summary (system)
**As the** Orchestrator, **I write** a thread outcome summary when a thread is approved, **so that** the agreement is captured as context for later steps.
**Personas**: OWN, NTM, DEV (beneficiaries) · **Reqs**: FR-4.8 · **Priority**: Must
- [ ] On approval, a summary of what was agreed is posted to the thread
- [ ] The approved summary becomes part of the context given to later agents and the plan
- [ ] The summary is clearly labelled as agent-written until a member confirms it

### US-5.8 Edit the outcome summary before approving
**As a** member, **I want** to edit the outcome summary before I approve, **so that** it states what we really decided.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.8, FR-6.1 · **Priority**: Must
- [ ] The summary is editable up to the moment of approval
- [ ] The final text is the one stored and used downstream
- [ ] Edits are shown in the thread so the change is visible to others

---

## E6. Thread agents

### US-6.1 Choose an agent with `@`
**As a** member, **I want** typing `@` to offer the available agent types, **so that** I can call the right helper without memorising commands.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-5.1 · **Priority**: Must
- [ ] The menu lists `@ask`, `@code`, `@search` and `@draft` with a one-line description of each
- [ ] Agents that are not available (e.g. `@code` on a managed folder, `@search` with no provider) are hidden or explain why
- [ ] The agent receives the thread and session context

### US-6.2 Ask a question with `@ask`
**As a** member, **I want** to ask `@ask` a question, **so that** I get an answer grounded in the thread and session.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-5.1, FR-5.2 · **Priority**: Must
- [ ] The answer is posted as a message from the agent in the same thread
- [ ] The agent stops after answering and does not act again until called
- [ ] The answer does not change the thread's status

### US-6.3 Analyse code with `@code`
**As a** developer, **I want** `@code` to analyse the session workspace and approved extra repos, **so that** the team's discussion is grounded in the actual code.
**Personas**: DEV, OWN · **Reqs**: FR-5.1, FR-11.5, NFR-4.1 · **Priority**: Must
- [ ] `@code` can read the session workspace and any approved extra repos, read-only
- [ ] It cites the files it used
- [ ] It cannot reach repos or paths outside those granted to the session

### US-6.4 Search the web with `@search`
**As a** member, **I want** `@search` to search the web and cite sources, **so that** we can check facts and prior art.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-5.1, FR-10.3 · **Priority**: Must
- [ ] Results are posted with the URL for each source used
- [ ] Web content is treated as untrusted input and never changes the agent's permissions
- [ ] If search fails or is unavailable, the thread shows a clear message instead of an empty answer

### US-6.5 Draft with `@draft`
**As a** member, **I want** `@draft` to produce a draft answer or artifact, **so that** I have something concrete to react to.
**Personas**: NTM, OWN · **Reqs**: FR-5.1 · **Priority**: Should
- [ ] The draft is posted in the thread and can be saved as an artifact (E10)
- [ ] A draft is never treated as agreed until the thread is approved

### US-6.6 Watch agent output and cancel it
**As a** member, **I want** to watch an agent's answer stream in and cancel it, **so that** I am not stuck waiting on the wrong thing.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-5.3 · **Priority**: Must (streaming), Should (cancel)
- [ ] Output appears incrementally as it is generated
- [ ] The person who called the agent can cancel a running call
- [ ] A cancelled call keeps the partial output, labelled as stopped

### US-6.7 See what an agent did
**As a** member, **I want** a collapsible activity log on an agent message, **so that** I can judge how much to trust it.
**Personas**: DEV, OWN, NTM · **Reqs**: FR-5.4 · **Priority**: Should
- [ ] The log lists tools used, files read and searches made
- [ ] It is collapsed by default and expandable
- [ ] It never shows credentials or secrets

### US-6.8 Agents answer but do not change the workspace
**As a** Session Owner, **I want** thread agents to be short-lived and unable to change the workspace, **so that** discussion can never alter files by accident.
**Personas**: OWN, DEV · **Reqs**: FR-5.2, FR-5.5, FR-16.2, FR-16.3, SECURITY-11 · **Priority**: Must
- [ ] A thread agent runs for one request, posts its result and stops
- [ ] A thread agent has no write access to workspace files
- [ ] A thread agent cannot approve a thread, the plan or any output
- [ ] Write access is denied by the runner policy (read-only binding), not by instructions to the model

### US-6.9 Request and approve access to another repo
**As a** developer, **I want** to request access to an additional repo in a thread and have the team approve it, **so that** agents can read related code.
**Personas**: DEV, OWN · **Reqs**: FR-11.5, SECURITY-14 · **Priority**: Should
- [ ] A request names the repo and the reason, inside a thread
- [ ] A member approves it through the same approval action as a thread
- [ ] An approved repo is cloned read-only; writes are never possible to it
- [ ] The grant is recorded in the audit log and its clone is deleted when the session ends

---

## E7. Approvals

### US-7.1 Approve a thread
**As a** member, **I want** to approve a thread, **so that** the team has an explicit agreement before moving on.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-6.1, FR-3.5 · **Priority**: Must
- [ ] Any session member can approve; a single approval closes the thread
- [ ] Only humans can approve; agents and the orchestrator cannot
- [ ] The approve action shows the outcome summary that will be recorded

### US-7.2 See who approved and when
**As a** member, **I want** to see who approved a thread and when, **so that** decisions are accountable.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-6.2 · **Priority**: Must
- [ ] The approver's name and timestamp are shown on the thread
- [ ] The same information is recorded in the audit log

### US-7.3 Revoke an approval
**As a** member, **I want** to revoke an approval and reopen the thread, **so that** a mistaken decision can be corrected.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-6.2 · **Priority**: Should
- [ ] Revoking sets the thread to reopened and records who did it
- [ ] Revocation is blocked, with a reason shown, once later steps (an approved plan or task) depend on the thread
- [ ] Steps that depend on a reopened thread are flagged

---

## E8. Plan

### US-8.1 Orchestrator drafts the plan (system)
**As the** Orchestrator, **I draft** a plan once the question rounds are settled, **so that** the team can agree what will be built.
**Personas**: OWN, NTM, DEV (beneficiaries) · **Reqs**: FR-7.1 · **Priority**: Must
- [ ] The plan has numbered steps, what each builds, expected outputs (artifacts and/or code changes) and dependencies
- [ ] The draft uses the approved thread outcome summaries as its context
- [ ] The orchestrator does not start any task from a draft

### US-8.2 Read the plan
**As a** member, **I want** the plan as a structured document with clear next steps, **so that** I can review it without technical help.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-7.2 · **Priority**: Must
- [ ] Steps are listed in order with dependencies visible
- [ ] Each step describes its output in plain language
- [ ] The plan is readable on a phone

### US-8.3 Discuss a plan step
**As a** member, **I want** to open a thread on any plan step, **so that** I can question or refine it.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-7.3 · **Priority**: Must
- [ ] Only members can open plan-step threads; the orchestrator cannot
- [ ] A step thread works like any other thread and is approved the same way
- [ ] Steps show the status of their threads

### US-8.4 Request a plan rework
**As a** member, **I want** to ask for the plan to be reworked and see earlier versions, **so that** it gets better without losing history.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-7.4 · **Priority**: Must
- [ ] A rework request includes feedback the orchestrator uses
- [ ] Each revision is saved as a new plan version
- [ ] I can view and compare earlier versions

### US-8.5 Trace plan steps back to decisions
**As a** member, **I want** each plan step linked to the threads it came from, **so that** I can see why it is there.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-7.6 · **Priority**: Should
- [ ] Every step lists the approved threads it derives from
- [ ] A link opens the thread and its outcome summary

### US-8.6 Approve the plan
**As a** member, **I want** to approve the plan, **so that** work can begin.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-7.5, FR-7.7, FR-6.1 · **Priority**: Must
- [ ] Plan approval follows the same rule as thread approval (any member, single approval)
- [ ] The approver and time are recorded
- [ ] Approving a plan version locks it; later changes create a new version that needs approval again

### US-8.7 No work before the plan is approved
**As a** Session Owner, **I want** no tasks to start until the plan is approved, **so that** agents never act on an unapproved idea.
**Personas**: OWN · **Reqs**: FR-7.5, SECURITY-15 · **Priority**: Must
- [ ] No task is created or run while the plan is a draft or in rework
- [ ] A failure in the orchestrator cannot bypass this gate

---

## E9. Tasks and output review

### US-9.1 Orchestrator spawns tasks (system)
**As the** Orchestrator, **I spawn** one or more tasks per approved plan step and run them in dependency order, **so that** the plan is carried out.
**Personas**: OWN, DEV (beneficiaries) · **Reqs**: FR-8.1, NFR-3.3 · **Priority**: Must
- [ ] Tasks are created only after plan approval
- [ ] A task does not start until the tasks it depends on are approved
- [ ] Task work runs off the request path so the interface stays responsive

### US-9.2 Task agents create artifacts and edit workspace files
**As a** Session Owner, **I want** task agents to create or edit artifacts and files in the workspace, **so that** the plan produces real output.
**Personas**: OWN, DEV, NTM · **Reqs**: FR-8.2, FR-16.2, FR-16.4 · **Priority**: Must
- [ ] A task agent can create and edit artifacts and read and write files in the session workspace
- [ ] A task agent cannot touch anything outside the session workspace
- [ ] If the required sandbox runner cannot start, the task fails with a clear reason and never runs under a looser tier
- [ ] Changes stay in the session worktree until committed (US-11.1)

### US-9.3 Task agents cannot run commands
**As a** Session Owner, **I want** task agents unable to run commands, **so that** the MVP is safe to run on a shared host.
**Personas**: OWN, ADM · **Reqs**: FR-8.3, FR-16.1, FR-16.3, SECURITY-11 · **Priority**: Must
- [ ] No tool for installs, builds, tests or scripts is available to any agent
- [ ] If a task needs a command, it reports that clearly instead of working around the limit
- [ ] The sandbox policy disables the command tool, and the runner enforces this outside the model

### US-9.4 Track task progress
**As a** member, **I want** to see each task's status, **so that** I know what is running and what needs me.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-8.4 · **Priority**: Must
- [ ] Status is one of: queued, running, needs review, approved, failed
- [ ] Each task has its own thread
- [ ] Status shows on the session overview and updates live

### US-9.5 Review code changes as a diff
**As a** developer, **I want** to review a task's workspace changes as a diff, **so that** I can judge them like a code review.
**Personas**: DEV, OWN · **Reqs**: FR-8.5 · **Priority**: Must
- [ ] The diff shows added, changed and removed files with line-level changes
- [ ] I can comment on the task thread while reviewing
- [ ] Changes are not kept in the approved state until approved

### US-9.6 Review a new artifact version
**As a** non-technical member, **I want** to read a task's new artifact in rendered form, **so that** I can review without reading a diff.
**Personas**: NTM, OWN · **Reqs**: FR-8.5 · **Priority**: Must
- [ ] The rendered artifact is shown, with a way to see what changed from the previous version
- [ ] The review works on a phone

### US-9.7 Approve task output
**As a** member, **I want** to approve a task's output, **so that** it is kept.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-8.5, FR-6.3 · **Priority**: Must
- [ ] Approval follows FR-6 (any member, single approval)
- [ ] Approved output becomes part of the workspace or artifact history
- [ ] Only humans can approve output

### US-9.8 Reject output with feedback or discard it
**As a** member, **I want** to reject output with feedback and have it reworked, or discard it, **so that** poor results are not kept.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-8.6 · **Priority**: Must
- [ ] Rejecting with feedback sends the feedback to the task agent for another attempt
- [ ] Discarding removes the output from the worktree or marks the artifact version as discarded
- [ ] Prior attempts stay visible in the task thread

### US-9.9 Retry a failed task
**As a** member, **I want** a failed task to tell me why and let me retry, **so that** a transient fault does not block the session.
**Personas**: OWN, DEV · **Reqs**: FR-8.7, NFR-5.3 · **Priority**: Should
- [ ] The task shows the failure reason in plain language
- [ ] Retry restarts it without losing earlier approved work
- [ ] A job interrupted by a restart resumes or shows as failed and can be retried; it is never left stuck as running

---

## E10. Artifacts

### US-10.1 Read rendered artifacts
**As a** member, **I want** artifacts rendered with code highlighting and Mermaid diagrams, **so that** they are easy to read.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-13.2 · **Priority**: Must
- [ ] Markdown, highlighted code blocks and Mermaid diagrams render correctly
- [ ] Rendering never runs scripts from artifact content
- [ ] A diagram that fails to render shows its source instead of an error page

### US-10.2 Version history and comparison
**As a** member, **I want** an artifact's full version history with who created each version and a diff between versions, **so that** I can see how it evolved.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-13.1, FR-13.3, FR-13.4 · **Priority**: Must
- [ ] Every change creates a new version; none is overwritten
- [ ] Each version shows its creator (a person, or a named agent/task)
- [ ] I can compare any two versions

### US-10.3 Edit an artifact
**As a** member, **I want** to edit an artifact's Markdown source, **so that** I can correct or extend it myself.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-13.3 · **Priority**: Should
- [ ] Saving an edit creates a new version attributed to me
- [ ] Rich-text (WYSIWYG) editing is not offered in the MVP

### US-10.4 Download an artifact
**As a** member, **I want** to download an artifact, **so that** I can use it outside Tandem.
**Personas**: NTM, OWN, DEV · **Reqs**: FR-13.3 · **Priority**: Should
- [ ] I can download the current or any earlier version as a file
- [ ] The download includes only that artifact's content

### US-10.5 Save an artifact into the workspace
**As a** developer, **I want** to save an artifact as a file in the workspace, **so that** it can be committed with the code.
**Personas**: DEV, OWN · **Reqs**: FR-13.5, FR-11.3 · **Priority**: Could
- [ ] I choose the target path in the workspace
- [ ] The saved file appears as a change awaiting commit
- [ ] The action is unavailable for managed-folder workspaces

---

## E11. Commit and end of session

### US-11.1 Changes stay in the worktree until committed
**As a** Session Owner, **I want** workspace changes to stay in the session worktree until I explicitly commit, **so that** nothing reaches the remote by accident.
**Personas**: OWN, DEV · **Reqs**: FR-11.3 · **Priority**: Must
- [ ] No agent action pushes to the remote
- [ ] The session shows which files have uncommitted changes

### US-11.2 Commit and push to the session branch
**As a** developer, **I want** to commit approved changes and push them to the session branch, **so that** the work survives the session.
**Personas**: DEV, OWN · **Reqs**: FR-11.3, SECURITY-14 · **Priority**: Must
- [ ] Commit requires an explicit action by a member and a commit message
- [ ] The commit is pushed to the session-specific branch on GitHub
- [ ] Only the primary workspace can be committed; extra repos are read-only
- [ ] The commit and push are recorded in the audit log

### US-11.3 Open a pull request
**As a** developer, **I want** to open a pull request from the session branch, **so that** the change goes through normal review.
**Personas**: DEV, OWN · **Reqs**: FR-11.4 · **Priority**: Should
- [ ] The PR is opened through the GitHub connector against a base branch I choose
- [ ] Its description can be pre-filled from the plan and approved outcomes
- [ ] A link to the PR appears in the session

### US-11.4 End a session
**As a** Session Owner, **I want** ending a session to warn me about uncommitted changes, **so that** I do not lose work by mistake.
**Personas**: OWN · **Reqs**: FR-2.6, FR-2.5 · **Priority**: Must
- [ ] If uncommitted changes exist, I must confirm and am told they will be lost
- [ ] With no uncommitted changes, ending needs only a simple confirmation
- [ ] Only the owner can end the session

### US-11.5 Workspace is cleaned up when a session ends
**As a** Session Owner, **I want** the workspace clones deleted when the session ends, **so that** no copy of the repo lingers on the server.
**Personas**: OWN, ADM · **Reqs**: FR-11.6 · **Priority**: Must
- [ ] The primary clone and any extra-repo clones are removed
- [ ] If deletion fails, the failure is recorded and retried rather than silently ignored

### US-11.6 Ended and archived sessions are read-only
**As a** member, **I want** an ended session to remain readable, **so that** I can look back at decisions.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-2.5 · **Priority**: Must
- [ ] Threads, plan, artifacts and approvals are viewable but cannot be changed
- [ ] The owner can archive an ended session to remove it from the active list

---

## E12. Notifications and presence

### US-12.1 Be notified when mentioned
**As a** member, **I want** an in-app notification when I am mentioned, **so that** I notice requests directed at me.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-14.1 · **Priority**: Should
- [ ] A mention creates a notification linking to the message
- [ ] Notifications can be marked read
- [ ] Email notifications are not part of the MVP

### US-12.2 Be notified when something needs me
**As a** member, **I want** notifications for threads awaiting approval, a plan ready for review, and task output ready for review, **so that** the session keeps moving.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-14.1 · **Priority**: Should
- [ ] Each of the three events produces a notification to session members
- [ ] Clicking it opens the item to review

### US-12.3 See presence and unread
**As a** member, **I want** to see who is online and which threads have unread messages, **so that** I know where to look.
**Personas**: OWN, NTM, DEV · **Reqs**: FR-4.6 · **Priority**: Should
- [ ] A member list shows who is online
- [ ] Threads and chat show an unread indicator that clears when read
- [ ] Live co-editing cursors are not shown (out of scope)

---

## E13. Security, resilience and usability touchpoints

### US-13.1 Clear limits on messages and uploads
**As a** member, **I want** clear errors when a message or upload is too large, **so that** I know what to change.
**Personas**: NTM, DEV, OWN · **Reqs**: SECURITY-05 · **Priority**: Should
- [ ] Oversized messages and files are rejected with the limit stated
- [ ] Nothing partial is stored after a rejection

### US-13.2 Safe display of untrusted content
**As a** member, **I want** content from the web, files and agents displayed safely, **so that** a malicious document cannot harm me.
**Personas**: NTM, OWN, DEV · **Reqs**: SECURITY-05, SECURITY-04, NFR-4.1 · **Priority**: Must
- [ ] Rendered Markdown is sanitised: no script execution and no unsafe embedded HTML
- [ ] A content security policy is in force for all pages
- [ ] Instructions found inside web pages, files or repos cannot change what an agent is permitted to do

### US-13.3 Lockout after repeated failed sign-ins
**As an** Instance Admin, **I want** repeated failed sign-ins limited, **so that** passwords cannot be guessed freely.
**Personas**: ADM, OWN, NTM, DEV · **Reqs**: FR-1.10, SECURITY-12 · **Priority**: Should
- [ ] Each failed attempt adds a growing delay before the next try is accepted
- [ ] After 10 failures in an hour the account is locked temporarily, and each source is rate-limited
- [ ] The user sees a clear, non-revealing message and when to retry
- [ ] Lockouts appear in the audit log

### US-13.4 Back up and restore
**As an** Instance Admin, **I want** built-in backup and restore commands for the database and stored files, **so that** I can recover from a failure.
**Personas**: ADM · **Reqs**: NFR-5.6, NFR-2.3 · **Priority**: Should
- [ ] One command produces a backup of the database and stored files
- [ ] A restore command recovers an instance from that backup
- [ ] The self-hosting guide documents both commands

### US-13.5 Know what data goes to which model provider
**As an** Instance Admin, **I want** documentation of what data is sent to which provider per session, **so that** I can make privacy decisions.
**Personas**: ADM, OWN · **Reqs**: NFR-4.3, NFR-4.2 · **Priority**: Should
- [ ] Each session shows which providers its orchestrator and agents use
- [ ] Documentation states what content (prompts, thread text, file excerpts) is sent
- [ ] No credentials are ever included in prompts

### US-13.6 Understand degraded service
**As a** member, **I want** a clear message when a model provider, search, or GitHub is unavailable, **so that** I can tell it is not my mistake.
**Personas**: NTM, OWN, DEV · **Reqs**: NFR-5.2 · **Priority**: Should
- [ ] The affected action shows a specific explanation and a retry option
- [ ] Unrelated parts of the app keep working
- [ ] Retries back off rather than hammering a failing service

### US-13.7 Use Tandem on a phone and in dark mode
**As a** non-technical member, **I want** to read, discuss and approve on a phone, in light or dark theme, **so that** I can take part from anywhere.
**Personas**: NTM, OWN, DEV · **Reqs**: NFR-1.1, NFR-6.1, NFR-6.2, NFR-6.3 · **Priority**: Should
- [ ] Reading threads, reviewing the plan and approving all work on a phone-sized screen
- [ ] Light and dark themes are available
- [ ] Core flows meet WCAG 2.2 AA (keyboard use, contrast, labels)





---

## Requirements Coverage

Every functional requirement (FR) in `requirements.md` maps to at least one story. NFR touchpoints are listed after the table.

| Requirement | Stories |
|---|---|
| FR-1.1 | US-1.2 |
| FR-1.2 | US-1.1 |
| FR-1.3 | US-1.3, US-1.4 |
| FR-1.4 | US-1.3, US-1.8 |
| FR-1.5 | US-1.5 |
| FR-1.6 | US-1.6, US-1.7 |
| FR-1.7 | US-1.4, US-1.8 |
| FR-1.8 | US-1.2, US-1.6 |
| FR-1.9 | US-1.9 |
| FR-1.10 | US-13.3 |
| FR-2.1 | US-3.1 |
| FR-2.2 | US-3.1, US-3.4 |
| FR-2.3 | US-3.5 |
| FR-2.4 | US-3.6 |
| FR-2.5 | US-11.4, US-11.6 |
| FR-2.6 | US-11.4 |
| FR-2.7 | US-3.7 |
| FR-3.1 | US-4.1 |
| FR-3.2 | US-4.2 |
| FR-3.3 | US-4.3 |
| FR-3.4 | US-4.4, US-4.5 |
| FR-3.5 | US-4.1, US-7.1 |
| FR-4.1 | US-5.1 |
| FR-4.2 | US-5.2 |
| FR-4.3 | US-5.3 |
| FR-4.4 | US-5.4 |
| FR-4.5 | US-5.5 |
| FR-4.6 | US-12.3 |
| FR-4.7 | US-5.6 |
| FR-4.8 | US-5.7, US-5.8 |
| FR-5.1 | US-6.1, US-6.2, US-6.3, US-6.4, US-6.5 |
| FR-5.2 | US-6.2, US-6.8 |
| FR-5.3 | US-6.6 |
| FR-5.4 | US-6.7 |
| FR-5.5 | US-6.8 |
| FR-6.1 | US-5.8, US-7.1, US-8.6 |
| FR-6.2 | US-7.2, US-7.3 |
| FR-6.3 | US-9.7 |
| FR-7.1 | US-8.1 |
| FR-7.2 | US-8.2 |
| FR-7.3 | US-8.3 |
| FR-7.4 | US-8.4 |
| FR-7.5 | US-8.6, US-8.7 |
| FR-7.6 | US-8.5 |
| FR-7.7 | US-8.6 |
| FR-8.1 | US-9.1 |
| FR-8.2 | US-9.2 |
| FR-8.3 | US-9.3 |
| FR-8.4 | US-9.4 |
| FR-8.5 | US-9.5, US-9.6, US-9.7 |
| FR-8.6 | US-9.8 |
| FR-8.7 | US-9.9 |
| FR-9.1 | US-2.1, US-2.2, US-2.10 |
| FR-9.2 | US-2.1, US-2.2, US-2.3, US-2.10 |
| FR-9.3 | US-2.4, US-3.6 |
| FR-9.4 | US-2.3 |
| FR-9.5 | US-2.8 |
| FR-10.1 | US-2.5 |
| FR-10.2 | US-2.5 |
| FR-10.3 | US-6.4 |
| FR-11.1 | US-3.2, US-3.3 |
| FR-11.2 | US-3.2 |
| FR-11.3 | US-10.5, US-11.1, US-11.2 |
| FR-11.4 | US-11.3 |
| FR-11.5 | US-6.3, US-6.9 |
| FR-11.6 | US-11.5 |
| FR-11.7 | US-3.5 |
| FR-12.1 | US-2.6 |
| FR-12.2 | US-2.6, US-3.2 |
| FR-12.3 | US-2.6 |
| FR-13.1 | US-10.2 |
| FR-13.2 | US-10.1 |
| FR-13.3 | US-10.2, US-10.3, US-10.4 |
| FR-13.4 | US-10.2 |
| FR-13.5 | US-10.5 |
| FR-14.1 | US-12.1, US-12.2 |
| FR-15.1 | US-1.9, US-2.1 |
| FR-15.2 | US-2.7 |
| FR-16.1 | US-2.9, US-2.11, US-9.3 |
| FR-16.2 | US-2.11, US-6.8, US-9.2 |
| FR-16.3 | US-2.11, US-6.8, US-9.3 |
| FR-16.4 | US-2.11, US-9.2 |
| FR-16.5 | US-2.9 |
| FR-16.6 | US-2.9 |

**NFR touchpoints with stories**: SECURITY-04/05 (US-13.1, US-13.2), SECURITY-06 (US-2.11), SECURITY-11 (US-2.9, US-2.11, US-6.8, US-9.3), SECURITY-12 (US-1.2, US-1.5, US-13.3), SECURITY-14 (US-1.4, US-1.8, US-1.9, US-2.7, US-2.9), NFR-4.2/4.3 (US-2.1, US-13.5), NFR-5.2 (US-13.6), NFR-5.3 (US-9.9), NFR-5.6 (US-13.4), NFR-6 (US-13.7). Remaining NFRs stay constraints in `requirements.md`.

---

## Persona to Story Matrix

| Persona | Stories (primary or beneficiary) | Count |
|---|---|---|
| ADM | US-1.1, US-1.2, US-1.3, US-1.4, US-1.5, US-1.7, US-1.8, US-1.9, US-2.1, US-2.2, US-2.3, US-2.4, US-2.5, US-2.6, US-2.7, US-2.8, US-2.9, US-2.10, US-2.11, US-9.3, US-11.5, US-13.3, US-13.4, US-13.5,  | 24 |
| OWN | US-1.2, US-1.5, US-1.6, US-2.8, US-2.11, US-3.1, US-3.2, US-3.3, US-3.4, US-3.5, US-3.6, US-3.7, US-4.1, US-4.2, US-4.3, US-4.4, US-4.5, US-5.1, US-5.2, US-5.3, US-5.4, US-5.5, US-5.6, US-5.7, US-5.8, US-6.1, US-6.2, US-6.3, US-6.4, US-6.5, US-6.6, US-6.7, US-6.8, US-6.9, US-7.1, US-7.2, US-7.3, US-8.1, US-8.2, US-8.3, US-8.4, US-8.5, US-8.6, US-8.7, US-9.1, US-9.2, US-9.3, US-9.4, US-9.5, US-9.6, US-9.7, US-9.8, US-9.9, US-10.1, US-10.2, US-10.3, US-10.4, US-10.5, US-11.1, US-11.2, US-11.3, US-11.4, US-11.5, US-11.6, US-12.1, US-12.2, US-12.3, US-13.1, US-13.2, US-13.3, US-13.5, US-13.6, US-13.7,  | 73 |
| NTM | US-1.2, US-1.5, US-1.6, US-3.3, US-3.5, US-3.7, US-4.1, US-4.2, US-4.3, US-4.4, US-4.5, US-5.1, US-5.2, US-5.3, US-5.4, US-5.5, US-5.6, US-5.7, US-5.8, US-6.1, US-6.2, US-6.4, US-6.5, US-6.6, US-6.7, US-7.1, US-7.2, US-7.3, US-8.1, US-8.2, US-8.3, US-8.4, US-8.5, US-8.6, US-9.2, US-9.4, US-9.6, US-9.7, US-9.8, US-10.1, US-10.2, US-10.3, US-10.4, US-11.6, US-12.1, US-12.2, US-12.3, US-13.1, US-13.2, US-13.3, US-13.6, US-13.7,  | 52 |
| DEV | US-1.2, US-1.5, US-1.6, US-3.2, US-3.5, US-3.7, US-4.1, US-4.2, US-4.3, US-4.4, US-4.5, US-5.1, US-5.2, US-5.3, US-5.4, US-5.5, US-5.6, US-5.7, US-5.8, US-6.1, US-6.2, US-6.3, US-6.4, US-6.6, US-6.7, US-6.8, US-6.9, US-7.1, US-7.2, US-7.3, US-8.1, US-8.2, US-8.3, US-8.4, US-8.5, US-8.6, US-9.1, US-9.2, US-9.4, US-9.5, US-9.7, US-9.8, US-9.9, US-10.1, US-10.2, US-10.3, US-10.4, US-10.5, US-11.1, US-11.2, US-11.3, US-11.6, US-12.1, US-12.2, US-12.3, US-13.1, US-13.2, US-13.3, US-13.6, US-13.7,  | 60 |

---

## Priority Summary

| Priority (first listed) | Stories |
|---|---|
| Must | 53 |
| Should | 33 |
| Could | 3 |

---

## Reference Scenario (MVP demo script)

**Setting**: A small product team uses Tandem to plan and build a "CSV export" feature in an existing GitHub repo. People: **Priya** (ADM), **Marcus** (OWN, runs the session), **Lena** (NTM, product manager), **Sam** (DEV).

| Step | What happens | Stories |
|---|---|---|
| 1 | Priya installs Tandem, creates the admin account, enrols MFA, connects an Ollama provider and the GitHub connector, tests both, sets default models, and chooses the sandbox policy | US-1.1, US-1.5, US-2.1, US-2.3, US-2.4, US-2.6, US-2.9 |
| 2 | Priya invites Marcus, Lena and Sam; each sets a password and signs in | US-1.3, US-1.2 |
| 3 | Marcus creates a session "CSV export", writes the prompt, picks the repo as the workspace, and adds Lena and Sam | US-3.1, US-3.2, US-3.4, US-3.5 |
| 4 | The orchestrator reads the prompt and repo and proposes a round of questions; each becomes a thread. Marcus removes one and Lena adds one | US-4.1, US-4.2, US-4.3 |
| 5 | In the "which columns?" thread, Lena uses `@search` for competitor behaviour; Sam uses `@code` to find the existing report code; output streams in and cites sources and files | US-6.1, US-6.3, US-6.4, US-6.6, US-6.7 |
| 6 | Sam asks for read-only access to a shared "reporting-lib" repo; Marcus approves | US-6.9, US-7.1 |
| 7 | Lena reads the outcome summary on her phone, edits one line, and approves the thread; the others approve theirs | US-5.7, US-5.8, US-7.1, US-7.2, US-13.7 |
| 8 | The orchestrator explains that one gap remains, opens a short second round, and then drafts the plan | US-4.4, US-8.1 |
| 9 | Lena opens a thread on step 2 of the plan, asks for a rework, reads the new version, and approves the plan | US-8.2, US-8.3, US-8.4, US-8.6 |
| 10 | The orchestrator spawns tasks; Sam reviews the code diff and rejects one with feedback; the reworked output is approved; Lena reviews the new "export guide" artifact | US-9.1, US-9.4, US-9.5, US-9.8, US-9.7, US-9.6 |
| 11 | Sam commits and pushes to the session branch and opens a pull request | US-11.1, US-11.2, US-11.3 |
| 12 | Marcus ends the session; Tandem confirms there is nothing uncommitted and deletes the workspace clone. The session stays readable | US-11.4, US-11.5, US-11.6 |

```text
1 Admin setup -> 2 Invite users -> 3 Create session -> 4 Question round
-> 5 Threads with @agents -> 6 Extra repo access -> 7 Approve threads
-> 8 Second round + plan draft -> 9 Plan rework + approval -> 10 Tasks + review
-> 11 Commit, push, PR -> 12 End session and clean up
```
