# Requirements Clarification Questions: Tandem (Round 3, final)

Five short questions to resolve one contradiction and a few gaps from Round 2. After this,
the requirements document gets written.

Fill in the letter after each `[Answer]:` tag, or reply in chat (e.g. "1A 2C 3A 4A 5A").

---

## Contradiction 1: "No integrations" vs. repos and a GitHub connector
In Clarification 1 you chose **A** (no live integrations in the MVP, just a plugin/adapter interface).
But Clarifications 2 and 3 describe sessions that work in folders or repos, commit code, and reach
other local or remote repos through a **GitHub connector**. These can't both hold for the MVP.

### Question 1
How should Git and GitHub fit into the MVP?

A) Git is **core** (not an integration): sessions work in folders and Git repos, and agents can commit. The **GitHub connector is the first plugin**, built on the adapter interface and included in the MVP. Other integrations come later.

B) Git is core in the MVP, but remote repos use plain Git over HTTPS/SSH with a token. The GitHub connector (issues, PRs, repo discovery) comes after the MVP.

C) MVP generates artifacts only, as in the original Clarification 1 answer. Folder, repo and GitHub work all come after the MVP.

X) Other (please describe after [Answer]: tag below)

[Answer]: 

## Ambiguity 1: What "local" repos and folders mean
Tandem runs on a server (Docker), so "local" could mean the server's disk or each user's own computer.
Reaching a user's own computer would need a separate desktop helper app.

### Question 2
For the MVP, where do "local" folders and repos live?

A) On the Tandem host: an admin mounts one or more directories into the container, and sessions can use folders and repos inside them (subject to approval)

B) On each user's own computer, through a small desktop helper app that connects to Tandem

C) Host-mounted directories (A) for the MVP; the desktop helper (B) comes later

X) Other (please describe after [Answer]: tag below)

[Answer]: 

## Ambiguity 2: Session workspace
You said code "would be in the repo for that session", with other repos reachable on request.

### Question 3
How is a session's primary workspace set up?

A) Each session has **exactly one** primary workspace (a folder or repo), chosen when the session is created. Artifact-only sessions get an empty managed folder automatically.

B) The primary workspace is **optional**: sessions can be artifact-only, and a workspace can be attached later

X) Other (please describe after [Answer]: tag below)

[Answer]: 

## Ambiguity 3: What agents may execute
Writing and committing code is clear. Running code (installing dependencies, running tests or builds)
is a big security decision, because it means executing untrusted, AI-generated code on your server.

### Question 4
Can agents run commands in the workspace during the MVP?

A) No: agents can read, write and commit files only. People run and test the code themselves.

B) Yes, but only inside an isolated, throwaway sandbox container per task (no host access, limited network, resource limits), with each command set approved in a thread first

C) A for the MVP; sandboxed execution (B) is the first post-MVP milestone

X) Other (please describe after [Answer]: tag below)

[Answer]: 

## Ambiguity 4: Scope of the Resiliency extension
You enabled the Resiliency extension in Round 1, and in Round 2 said the deployment and recovery
decisions belong to whoever self-hosts Tandem. That leaves the question of whether the extension
still applies to the **product itself**.

### Question 5
How should the Resiliency extension apply?

A) Keep it, scoped to **product-level** resiliency: health/readiness endpoints, timeouts, retries and circuit breaking for model providers and search, graceful restart without losing session state, reversible database migrations, built-in backup/restore commands, and GitHub Actions CI that builds, tests and publishes versioned Docker images. Deployment topology, recovery targets and incident process go into a **self-hosting guide** for operators to decide.

B) Disable the Resiliency extension entirely

X) Other (please describe after [Answer]: tag below)

[Answer]: 
