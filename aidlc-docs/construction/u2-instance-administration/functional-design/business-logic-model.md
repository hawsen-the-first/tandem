# U2 Instance Administration: Business Logic Model

Workflows continue the U1 numbering (W1 to W15). Rules are **BR-n** in `business-rules.md`; entities are in `domain-entities.md`. "Record audit" means one entry in the same transaction as the change (BR-21.1). Every workflow starts with "the caller is an admin" (BR-21.3); a non-admin gets "not found".

## W16 Add or edit a provider (US-2.1)
1. The admin enters name, base URL, optional key and the model names.
2. Validate: name (BR-14.2), URL rules (BR-19.1, BR-19.2, BR-19.3 on literal addresses), model names (BR-14.3).
3. Seal a new key if one was entered (BR-20.1); an empty key field keeps the stored one; "remove key" clears it (BR-14.4).
4. In one transaction: save the provider and its models (models no longer listed are removed; defaults pointing at them are cleared, BR-15.3); record audit `config.provider.created` or `config.provider.updated` with before and after values (secrets as set or not set).
5. Show the saved provider with "Key saved" status and an offer to run a test.

## W17 Test a provider (US-2.3)
1. The admin chooses Test on a provider.
2. One test runs at a time per provider; a second request while one runs is refused with "A test is already running".
3. Resolve and check the host (BR-19.4). If refused, store `refused_address` and stop.
4. Provider-level check: list models when the provider supports it (reachability, authorisation, context lengths).
5. For each listed model: send a small chat request (`unknown_model` if the provider rejects the name), then the tool-call probe (A5). Record `tool_calling` and any context length as **detected**, replacing earlier detected values and leaving declared ones (BR-14.5).
6. Store the last result per provider and per model (A4). Record audit `config.test.run` with the outcome (no bodies, no keys).
7. Return the results; models lacking a needed capability show the warning of BR-14.8.

## W18 Remove a provider (US-2.1)
1. The admin chooses Remove. The server computes the impact: defaults that point at the provider's models, plus the usage hook's answer for sessions (empty in U2; later units add counts).
2. The confirmation lists the impact in words ("The orchestrator default will be cleared. 3 sessions use this provider and will ask their owners to choose again.").
3. On confirmation, in one transaction: delete the provider, its models and sealed key; clear the affected defaults; record audit `config.provider.deleted` (including what was cleared).

## W19 Set default models (US-2.4)
1. The admin picks, from listed models of all providers, the orchestrator default and the agents default (either may be left unset).
2. For each pick, evaluate suitability (BR-15.2). Unsuitable or untested picks show a warning and need an explicit "Use anyway".
3. Save; record audit `config.defaults.updated` with before and after (provider and model names) and any warnings.
4. New sessions will read these at creation; existing sessions are untouched (BR-15.4).

## W20 Configure and activate search (US-2.5)
1. The admin selects a kind (SearXNG, Brave Search, Tavily).
2. SearXNG: enter or accept the base URL (BR-19). Brave and Tavily: enter an API key (BR-20).
3. Save the settings for that kind; record audit `config.search.updated`.
4. Test: send one small query (BR-16.4), store the last result, record audit `config.test.run`.
5. Activate: make that kind the single active one (BR-16.1); an untested provider can be activated after a warning. Deactivating leaves "not set up". Record audit `config.search.updated`.

## W21 Configure and test the GitHub connector (US-2.6)
1. The admin pastes a fine-grained token. The screen shows the minimum permissions (BR-17.4).
2. Seal and save it (BR-17.2); record audit `config.connector.updated`.
3. Test: call GitHub as the token's owner; store the account login and expiry; fetch up to 100 reachable repositories to show (not stored). Record audit `config.test.run`.
4. Remove: confirm, delete the sealed token, record audit `config.connector.updated`; the connector is off (BR-17.6).

## W22 Edit the sandbox policy (US-2.9)
1. The admin sees the current policy: runner tier, tool allow-list (with `command_run` disabled), network rule and limits, with a note that memory and CPU limits are enforced by the container runner only.
2. For the tier `container`: run the runtime probe (A14); if unreachable the option is unavailable and saving is refused (BR-18.2).
3. Validate the whole policy (BR-18.3 to BR-18.6).
4. Save; record audit `config.sandbox_policy.updated` with before and after. (Per-session policies, when added, are validated against this one as a ceiling, BR-18.8.) Runs already in progress are unaffected (BR-18.1).

## W23 Check the container runtime (US-2.9)
1. The admin chooses "Check container runtime".
2. The server probes the configured runtime socket without starting anything and returns `reachable` (runtime name and version) or `unreachable` (reason). The result is stored as the last probe result and audited as `config.test.run`.

## W24 Read configuration for the runtime (later units)
A server-side read port serves later units, never the browser:
1. `list_models()`: providers' models with capabilities, test status and suitability.
2. `defaults()`: the two default model choices.
3. `resolve_model(model_id)`: the provider connection (base URL and the decrypted key, held in memory for the call only) for the worker.
4. `active_search()`: the active kind and its connection.
5. `github_credentials()`: the decrypted token for clone, push and pull requests, for the worker only.
6. `sandbox_policy(scope)`: the current policy; a run captures it at start.
Results may be cached for at most 5 seconds (A12).

## Data flow summary
- **Browser to API**: names, URLs, secrets typed once, selections. **API to browser**: configuration without secrets, saved flags, last test results, warnings.
- **API to outside**: only the test actions and, later, the runtime, always through the outbound URL rules (BR-19).
- **Secrets**: browser → API → vault (sealed) → database; unsealed only in memory for one call.
