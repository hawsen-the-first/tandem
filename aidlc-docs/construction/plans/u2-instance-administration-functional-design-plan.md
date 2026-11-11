# Functional Design Plan: U2 Instance administration

**Stage**: CONSTRUCTION - Functional Design, **Light** (unit `u2-instance-administration`)
**Inputs**: `unit-of-work.md` (U2), `unit-of-work-story-map.md`, `stories.md`, `requirements.md` (FR-9, FR-10, FR-12, FR-15, FR-16), U1 artifacts
**Stories in scope**: US-2.1 provider, US-2.3 test a provider, US-2.4 default models, US-2.5 web search, US-2.6 GitHub connector (fine-grained token), US-2.9 sandbox policy
**Scope**: business logic, domain model and rules, technology-agnostic. Libraries, schema and wire formats belong to Code Generation.

## Steps
- [x] 1. Analyse the unit context (stories, requirements, components C12, C20, C21, C22, C16)
- [x] 2. Ask the questions below (answered in chat)
- [x] 3. Record decisions and the changes they cause to earlier artifacts
- [x] 4. Domain entities (`domain-entities.md`)
- [x] 5. Business rules (`business-rules.md`)
- [x] 6. Business logic model (`business-logic-model.md`)
- [x] 7. Frontend components (`frontend-components.md`)
- [ ] 8. Approval of the functional design (pull request)

## Decisions (questions answered in chat)
| # | Question | Decision |
|---|---|---|
| Q1 | How strict is outbound URL safety for provider, search and connector URLs? | The recommended option: private networks allowed (Ollama, LM Studio), `http://` only for loopback or private hosts, `https://` otherwise, link-local and cloud-metadata addresses always blocked, no redirects to a different host, the connected address re-checked. **Admin allow and block lists are a Could-have**: new story **US-2.12** (U9), requirement FR-15.3 |
| Q2 | How are models and capabilities learned? | The admin lists model names by hand; a test probes them (a chat request, a tool-call probe, context length from the provider's model listing when offered). Each capability is stored with its source (detected or declared); the admin may declare context length |
| Q3 | A default model lacks a needed capability: what at save time? | Warn, allow, and refuse at run time. The runtime (U4) must not run a job on a model lacking the capability and must say why |
| Q4 | GitHub authentication in U2? | **Fine-grained token now; GitHub App in a later unit.** US-2.6 narrowed; new story **US-2.13** (Should, U5); FR-12.2 annotated. The connector interface keeps room for the App |
| Q5 | What may the screen show about a saved secret? | "Saved" and the date only. Never the value, not even a hint. Empty field keeps the old secret; "Remove" deletes it |
| Q6 | How many search providers are active? | One. Settings of the others are kept so switching back is easy |
| Q7 | Sandbox network rule and limits? | Network `none` (default) or a host allow-list. Limits: wall-clock seconds per run, maximum output size, maximum size and count of files written; memory and CPU limits are stored too but only the container runner (U7) can enforce them, and the screen says so |
| Q8 | Removing a provider that is still in use? | Allowed after a confirmation that names what will happen: defaults that point at it are cleared, and sessions that used it will ask their owner to choose again. (A usage check is a hook that later units extend, so a session using the provider shows in the warning.) |

## Changes to earlier artifacts caused by these decisions
- `stories.md`: US-2.6 narrowed; **US-2.12** and **US-2.13** added; coverage table, persona matrix and priority summary updated (91 stories: Must 53, Should 34, Could 4).
- `unit-of-work-story-map.md`: US-2.13 assigned to U5 (Should), US-2.12 to U9 (Could); totals and an assignment note updated. `unit-of-work-dependency.md`: 91 of 91.
- `requirements.md`: FR-12.2 annotated; **FR-15.3** (Could) added.

## Working assumptions (not questions; confirm at approval)
- **A1**: Tool identifiers form a registry in code that later units extend: `workspace_read`, `workspace_write`, `artifact_edit`, `web_search`, `repo_read`, and `command_run`. `command_run` is listed but can never be enabled in the MVP (FR-8.3); the server rejects it.
- **A2**: Resource limit defaults and ranges: wall-clock 600 s (30 to 3,600), output 1 MiB (64 KiB to 16 MiB), file size 5 MiB (64 KiB to 100 MiB), files written 200 (1 to 10,000); container only: memory 512 MiB (64 to 16,384), CPU 1,000 millicores (100 to 8,000).
- **A3**: Test actions are admin-only, run one at a time per target, time out after 15 s per outbound call and 60 s in total, and are audited as an outcome (never logging prompts, keys or response bodies).
- **A4**: The last test result per target (status, time, safe message, detected capabilities) is stored and shown; nothing else from a test is kept.
- **A5**: The tool-call probe forces a trivial `echo` function. A model is `supported` if it returns a tool call for it, `unsupported` if the provider rejects tools or answers in plain text, and `unknown` if inconclusive (timeout, malformed answer).
- **A6**: Context length is read from the provider's model listing when it exposes one (`context_length`, `max_model_len` or `context_window`); otherwise unknown until the admin declares it.
- **A7**: Jobs need `tool_calling = supported` and a context length of at least 8,000 tokens. A default that fails either shows a warning (Q3).
- **A8**: Audit gains these event types: `config.provider.created`, `config.provider.updated`, `config.provider.deleted`, `config.defaults.updated`, `config.search.updated`, `config.connector.updated`, `config.sandbox_policy.updated`, `config.test.run`. Details hold before and after values with secrets reduced to "set" or "not set".
- **A9**: Secrets are sealed with the vault under their own purposes and bound to the row. Decryption happens only server-side, inside the call that needs it (U2 test actions; U4 and U5 runtime later). Credentials are never available inside a sandbox (FR-16.5).
- **A10**: Base URLs are entered in full (for OpenAI-compatible servers that normally ends in `/v1`); no path is appended silently. No userinfo, query or fragment is accepted.
- **A11**: Provider kinds are an extensible enum; U2 ships `openai_compatible` only (Anthropic arrives in U8, the Agent SDK in U9). GitHub is the only connector kind; the API base is fixed to `https://api.github.com` (GitHub Enterprise Server is not in the MVP).
- **A12**: Changes apply to runs started afterwards (FR-16.5); readers may cache configuration for at most 5 seconds (NFR design, non-security reads).
- **A13**: Everything here is admin-only and full-scope; non-admins get "not found" (BR-9.2).
- **A14**: Announcing "container available" uses a runtime probe of the configured container socket (`reachable` or `unreachable`, with runtime name and version). The probe runs on demand and when saving the `container` tier; it does not start anything.
- **A15**: Existing U1 audit and permission machinery is reused; U2 adds actions and event types, not new mechanisms.
