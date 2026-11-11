# U2 Instance Administration: Business Rules

Numbering continues from U1 (BR-1 to BR-13). "Admin" means a signed-in user with the `admin` role and a full-scope sign-in.

## BR-14 Providers and models (US-2.1, US-2.3)
- **BR-14.1** Only admins may view or change provider configuration; others get "not found" (BR-9.2).
- **BR-14.2** A provider needs a name (1 to 60 characters, unique ignoring case) and a valid base URL (BR-19). An API key is optional.
- **BR-14.3** A provider lists zero or more models by name (1 to 200 characters, unique within the provider). Saving does not require a test.
- **BR-14.4** Editing a provider never reveals its key. A submitted key replaces the stored one; an empty key field leaves it unchanged; an explicit "remove key" clears it (BR-20).
- **BR-14.5** Capabilities are stored with their source. A test sets `detected` values; an admin may declare `tool_calling` and `context_tokens`. A later successful test replaces a **detected** value but never overwrites a **declared** one (the result shows both so the admin can see a disagreement).
- **BR-14.6** A test sends a small chat request, then a tool-call probe with a forced trivial `echo` function, for each listed model, and reads context length from the provider's model listing when it offers one (A5, A6). It reports one of: success, `unreachable`, `timeout`, `unauthorised`, `unknown_model`, `bad_response` or `refused_address`, with a specific safe message. The last result per provider and per model is stored (A4).
- **BR-14.7** Tests are bounded: 15 s per outbound call, 60 s per test, one at a time per target, and never sent from a non-admin session. A test never persists prompts, keys or response bodies.
- **BR-14.8** A model lacking a needed capability is **flagged** wherever it is selected and is **never used silently** for a job that needs it (enforced by the runtime in U4; BR-15.2).

## BR-15 Default models (US-2.4)
- **BR-15.1** There is one default for the orchestrator and one for thread and task agents; either may be unset. A default points at a listed model, not at free text.
- **BR-15.2** A model is suitable when `tool_calling = supported` and `context_tokens >= 8,000` (A7). An unsuitable or untested default is **allowed with a warning** shown beside it and in the audit details. The runtime refuses to run a job on a model lacking a needed capability and says why.
- **BR-15.3** Removing a model or its provider clears any default that pointed at it. The confirmation names which defaults will be cleared.
- **BR-15.4** Changing a default affects sessions created afterwards only; models already chosen for existing sessions are unchanged.
- **BR-15.5** The screen shows, for each default, which provider it comes from.

## BR-16 Search (US-2.5)
- **BR-16.1** At most one search provider is active. With none active, `@search` is hidden or says search is not set up.
- **BR-16.2** SearXNG needs a base URL (default: the bundled optional service). Brave Search and Tavily need an API key and use their fixed endpoints.
- **BR-16.3** Keys are sealed and follow BR-20. Settings for inactive providers are kept.
- **BR-16.4** Activating a provider that has never passed a test is allowed with a warning; the test action sends one small query and reports success, `unreachable`, `timeout`, `unauthorised`, `bad_response` or `refused_address` (A3, A4). "No results" is a success with a note.
- **BR-16.5** Search results used by agents are cited with URLs in the thread (FR-10.3); that is enforced in U4.

## BR-17 GitHub connector (US-2.6)
- **BR-17.1** The connector is configured with a fine-grained personal access token. The API base is fixed to `https://api.github.com` (A11).
- **BR-17.2** The token is sealed, never returned to the browser, never written to logs, and never placed in a model prompt or a sandbox (FR-12.3, FR-16.5).
- **BR-17.3** The test calls GitHub with the token, reports the account login, the token's expiry when GitHub reports it, and the repositories it can reach (name list capped at 100, with the total), or one of `unreachable`, `timeout`, `unauthorised`, `bad_response`, `refused_address`.
- **BR-17.4** The screen states the minimum permissions: Contents read and write, Pull requests read and write, Metadata read, on the repositories to be used. A token with more access than needed is allowed but the screen advises against it.
- **BR-17.5** GitHub is reached only through the connector interface (FR-12.1), so other connectors and the GitHub App (US-2.13) can be added without changing core code.
- **BR-17.6** Removing the token turns the connector off: repo sessions cannot clone or push; managed-folder sessions are unaffected.

## BR-18 Sandbox policy (US-2.9)
- **BR-18.1** There is one instance-wide policy; a `scope` is stored so per-session overrides can be added later (FR-16.6; resolution in BR-18.8). A saved change applies to runs started afterwards; runs in progress keep the policy they started with (FR-16.5).
- **BR-18.2** `runner_tier` is `in_process` or `container`. Saving `container` requires that a runtime probe succeeds at that moment (A14); the choice is not offered when no runtime is reachable. If the container runner cannot start later, the run fails closed and never falls back to a looser tier (FR-16.4; behaviour delivered in U7).
- **BR-18.3** The tool allow-list may contain only registered tool identifiers (A1). `command_run` is shown as disabled and the server refuses to save a policy that contains it (FR-8.3).
- **BR-18.4** The network rule is `none` (the default) or an allow-list of host names (no schemes, paths or wildcards beyond a single leading `*.`; each at most 253 characters; at most 50 entries). Entries are lower-cased and unique. An allow-list does not weaken BR-19: addresses that are always blocked stay blocked.
- **BR-18.5** Every limit is configurable per instance: the admin may set any limit within its allowed range (A2), a saved value always wins over the factory default, and "Reset to default" returns a field to the factory value. The factory defaults are the starting values of a new instance and are not hard-wired into any run. Memory and CPU limits are stored but enforced only by the container runner; the screen says so.
- **BR-18.6** Concrete file paths cannot be entered in the policy (they come from session grants).
- **BR-18.7** Every change is audited with before and after values.
- **BR-18.8** *Resolution order (per-session limits are post-MVP, FR-16.6, but the rule is fixed now).* The effective policy for a run is the session policy where one exists, otherwise the instance policy. A session policy may **tighten but not exceed** the instance policy: every numeric limit at most the instance value, the tool allow-list a subset, the network rule no wider (`none` stays `none`; a host list a subset), the runner tier never looser than the instance tier. The instance policy is the ceiling; saving a session value above it is refused with the reason. If an admin later lowers the instance policy, existing session values above the new ceiling are clamped when a run starts. A run captures its effective policy when it starts (BR-18.1).

## BR-19 Outbound URLs (all of U2; Q1)
Applies to every URL the server calls on an admin's behalf: provider base URLs, the SearXNG URL and test calls.
- **BR-19.1** The URL must be `http` or `https`, have a host, and have no userinfo, query or fragment. A trailing slash is removed. No path is appended silently (A10).
- **BR-19.2** `http://` is accepted only when the host is a loopback address, a private-network address or a single-label or `.local` / `.internal` name; every public host must use `https://`.
- **BR-19.3** Link-local addresses and cloud-metadata endpoints (169.254.0.0/16, fe80::/10, fd00:ec2::254) are **always refused**, whatever the URL says.
- **BR-19.4** Each name is resolved when the call is made and every resolved address is checked against BR-19.3; the connection is made to the checked address, so a name that changes after saving cannot reach a blocked one.
- **BR-19.5** Redirects to a different host are not followed; redirects within the same host are followed at most 3 times and re-checked.
- **BR-19.6** Responses are read up to 1 MiB; anything larger is a `bad_response`. TLS certificates are always verified (private CAs are not supported in the MVP).
- **BR-19.7** A refused address is reported as `refused_address` with the reason ("link-local addresses are not allowed"), at save time for the URL and at call time for the resolved address.
- **BR-19.8** Admin allow and block lists are a Could-have (US-2.12) layered over these rules; until then there is no way to relax BR-19.3.

## BR-20 Secrets (US-2.1, US-2.5, US-2.6)
- **BR-20.1** API keys and tokens are sealed under their own vault purposes, bound to their row, and decrypted only inside the server call that needs them.
- **BR-20.2** No endpoint returns a secret. Responses carry `saved: true` or `false` and `saved_at` only.
- **BR-20.3** A secret field left empty on edit means "unchanged". "Remove" is a separate explicit action with a confirmation. A secret is never kept in the browser after submit.
- **BR-20.4** Secrets never appear in logs, audit details, error messages, metrics or test results.

## BR-21 Audit and permissions (all stories)
- **BR-21.1** Every create, change, removal, default change, activation and test is recorded (A8) in the same transaction as the change, except tests, which record their outcome after they run. If the audit write fails, the change does not happen (U1 BR-11.1).
- **BR-21.2** Audit details hold before and after values; secrets appear only as `set` or `not set`.
- **BR-21.3** Admin-only (BR-14.1), enforced on the server for every endpoint; a non-admin sees "not found".
- **BR-21.4** Read access to configuration for the runtime (providers, defaults, search, connector, policy) is a server-side read port used by later units; the browser never receives a secret through it.

## Error handling
| Situation | Admin sees |
|---|---|
| Invalid or disallowed URL | The reason, next to the field ("Use https:// for public servers", "That address is not allowed") |
| Duplicate provider name or model name | "A provider with that name already exists" / "That model is already listed" |
| Test cannot connect | The specific status and message (BR-14.6) |
| Test unauthorised | "The provider refused the key" (or token), never the key |
| Policy rejected | The unmet rule ("command_run cannot be enabled", "Network entries must be host names") |
| Container tier without a runtime | "No container runtime is reachable" and the tier stays unchanged |
| Audit write fails | "Something went wrong. Nothing was changed" (U1) |

## Security Baseline check at functional level
| Rule | Status | How |
|---|---|---|
| SECURITY-05 Input validation | Compliant (design) | BR-14.2, BR-18, BR-19 validate every field and URL |
| SECURITY-06 Credentials | Compliant (design) | BR-20, BR-17.2: sealed, never shown, never in prompts, logs or sandboxes |
| SECURITY-08 Authorisation | Compliant (design) | BR-21.3 |
| SECURITY-11 Secure design | Compliant (design) | BR-19 closes server-side request forgery; BR-18.2 fails closed |
| SECURITY-14 Audit | Compliant (design) | BR-21.1, BR-21.2 |
| SECURITY-15 Fail-safe | Compliant (design) | Audit failure blocks the change; tests are bounded |
