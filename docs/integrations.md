# Integrations: an admin guide

Admins configure what Tandem connects to under **Integrations** in the header. Members never see these
pages; the server answers "not found" to anyone who is not a signed-in admin with two-step sign-in
confirmed. Every change, removal, activation and test is recorded in the audit log, with before and
after values (secrets appear only as `set` or `not set`).

## Model providers

A provider is any service that speaks the OpenAI chat completions API: a hosted service, or a model server
on your own network such as Ollama, vLLM or llama.cpp.

1. **Add a provider.** Give it a name (up to 60 characters, unique ignoring case) and its base URL,
   including any path such as `/v1`. Tandem does not add a path for you. The API key is optional; local
   model servers often do not need one.
2. **List its models** by the exact names the provider expects. A label is optional and only changes what
   people see.
3. **Test it.** The test runs in the background (up to a minute; you can keep reading the page). For each
   model it sends one short chat request and a tool-call probe, and reads the context length from the
   provider's model listing when the provider offers one. Results arrive per model.
4. **Choose the default models** for the orchestrator and for thread and task agents. New sessions start
   with them; existing sessions keep the models they chose.

### Capabilities and warnings
A model suits agent work when it supports tool calling and has at least 8,000 tokens of context. A test
detects both. You can **declare** either value on the model to override what was detected, for example
when a provider does not report context length; a later test never overwrites a declared value, and the
page shows when the two disagree.

You may pick a model with a warning as a default, but you have to confirm it ("Use anyway"). Jobs that
need a capability the model lacks refuse to run on it and say why; Tandem never uses such a model silently.

### Keys
Keys are write-only. After saving, the page shows only that a key is saved and when. Leaving the key
field empty keeps the saved key; **Remove** deletes it when you save. Keys are encrypted at rest, never
returned to the browser, and never written to logs, audit entries or test results.

### Removing a provider
Removing a provider deletes its models and key. The confirmation names any default that will be cleared.

## Web search

One search service can be active at a time; agents use it for `@search` and cite the pages they read.
With none active, `@search` is unavailable.

| Service | What you supply |
|---|---|
| SearXNG | A base URL. Leave it empty to use the bundled service (`http://searxng:8080` in the reference deployment). Its JSON output must be enabled |
| Brave Search | An API key; the endpoint is fixed |
| Tavily | An API key; the endpoint is fixed |

Save a service's settings, **Test** it (one small query; "no results" still counts as working), then
**Make active**. You can activate a service that has never passed a test, but the page warns you.
Settings for inactive services are kept.

## GitHub

Repository sessions clone, push and open pull requests through a GitHub **fine-grained personal access
token**. Create one limited to the repositories Tandem should work on, with only these permissions:

- **Contents**: read and write
- **Pull requests**: read and write
- **Metadata**: read

A token with more access works, but is not recommended. **Test connection** shows the account the token
belongs to, its expiry when GitHub reports one, and the repositories it can reach (the first 100 names
and the total). Tandem calls only `https://api.github.com`.

Removing the token turns the connector off: repository sessions cannot clone or push until a new token
is saved. Managed-folder sessions are not affected. The token is never placed in a model prompt or a
sandbox.

## Sandbox policy

The policy controls what tools may do when agents run them. Changes apply to runs that start after you
save; runs in progress keep the policy they started with.

- **Runner.** *In-process* runs tools inside the worker with the limits below. *Container* gives each
  run its own container; it is offered only after **Check container runtime** succeeds, and saving it
  checks again. If the container runner later cannot start, the run fails; it never falls back to a
  looser runner. See the container-socket warning in [development.md](development.md#configuration).
- **Allowed tools.** Every registered tool is listed with what it does. `command_run` is shown but cannot
  be enabled in this version.
- **Network access.** *No network access* by default, or a list of allowed host names (no schemes or
  paths; a single leading `*.` is allowed; at most 50). Link-local and cloud-metadata addresses stay
  blocked whatever the list says.
- **Limits.** Time per run, output per run, largest file, files written, memory and CPU. Each field shows
  its default and allowed range, and **Reset to default** returns it to the factory value. Memory and CPU
  are enforced only by the container runner.

## Outbound addresses

Every URL Tandem calls for you (providers, SearXNG, GitHub) follows the same rules, checked when you save
and again when the call is made:

- `http` or `https` only, with a host and no user name, password, query or fragment.
- `http://` only for local and private-network hosts (loopback, private ranges, single-label names,
  `.local` and `.internal`); public hosts need `https://`.
- Link-local and cloud-metadata addresses (`169.254.0.0/16`, `fe80::/10`, `fd00:ec2::254`) are always
  refused, including when a name resolves to one later.
- Redirects to another host are not followed; responses over 1 MiB are refused; TLS certificates are
  always verified (private certificate authorities are not supported yet).

A refusal names its reason, for example "That address is not allowed (link-local and cloud-metadata
addresses are always blocked)".
