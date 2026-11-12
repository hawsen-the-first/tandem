# U2 Instance Administration: Frontend Components

Screens, components, state and the API each uses. Paths are **provisional** until the OpenAPI description is generated in Code Generation. Follows the U1 conventions: strict CSP, CSS Modules and design tokens, native dialogs, the shared components (`FormField`, `ConfirmDialog`, `ResponsiveTable`, `Toast`), WCAG 2.2 AA baseline and keyboard operation (BR-12).

## 1. Hierarchy

```text
RequireAdmin (U1)
  AdminIntegrationsLayout                 sub-navigation: Providers, Search, GitHub, Sandbox policy
    ProvidersPage                         US-2.1, 2.3, 2.4
      DefaultModelsCard                   orchestrator and agents defaults, warnings
      ProviderTable                       name, kind, URL, key status, last test, models, actions
      ProviderDialog                      add and edit: name, base URL, key, models
      ModelListEditor                     rows: model name, label, tool calling, context length (declared override)
      ProviderTestPanel                   run test, per-model results, capability badges
      RemoveProviderDialog                impact list, confirm
    SearchPage                            US-2.5
      SearchKindCard (x3)                 SearXNG, Brave, Tavily: settings, key status, test, "Make active"
    GitHubPage                            US-2.6
      TokenForm                           token field, saved status, remove
      PermissionsNotice                   minimum permissions
      ConnectionTestPanel                 account, expiry, reachable repositories
    SandboxPolicyPage                     US-2.9
      RunnerTierField                     in-process or container; "Check container runtime"
      ToolAllowListField                  registry checkboxes; command_run shown disabled
      NetworkRuleField                    none or host list editor
      LimitsFields                        time, output, file size, file count, memory, CPU (container only)
```

Navigation: the U1 header gains one admin link, "Integrations", opening `AdminIntegrationsLayout` (its sub-navigation is a labelled `nav` of links, not tabs that hide content).

## 2. Shared pieces
| Component | Purpose | Notes |
|---|---|---|
| `SecretField` | Write-only field for a key or token | Shows "Key saved on {date}" and never a value; empty means unchanged; separate "Remove" button with confirmation; clears its content after submit (BR-20.3) |
| `StatusBadge` | Test and capability status | Text plus icon, never colour alone; `ok`, `warning`, `error`, `unknown` |
| `TestResultView` | Last test result | Status, safe message, time, who; per-item details; announced politely when it changes |
| `UrlField` | Base URL input | Inline validation text from BR-19 (scheme, private versus public, forbidden addresses); the server decides |
| `HostListEditor` | Network allow-list | Add and remove rows, one host per row, accessible names, duplicate and format errors per row |
| `CapabilityWarning` | Warning beside a default or model | Explains what is missing ("This model has not shown tool calling") and that jobs will refuse to run on it |

## 3. Screens

### ProvidersPage (US-2.1, US-2.3, US-2.4)
- **State**: providers with models, defaults, test results, dialog state, per-provider test in progress.
- **Table columns**: name, kind, base URL, key (Saved / None), last test (badge, time), models (count), actions (Test, Edit, Remove).
- **ProviderDialog**: name, base URL (`UrlField`), key (`SecretField`), model rows (`ModelListEditor`). Errors beside fields (BR-14.2, BR-19); duplicate names reported against the right field.
- **Test**: starts a background run (NFR Design pattern 2) and polls its status once a second for at most 90 s; runs W17 in the worker; the button is disabled and a live region says "Testing" while the run is queued or running; results arrive per model as they land: reachable, tool calling (supported, unsupported, unknown, with source detected or declared) and context length. A model lacking a needed capability shows `CapabilityWarning`.
- **DefaultModelsCard**: two selects listing "{provider} / {model}" with the provider named; suitability warnings beside them (BR-15.2); an unsuitable pick needs "Use anyway".
- **Remove**: `RemoveProviderDialog` lists what will be cleared and which sessions are affected (BR-15.3, W18).
- **API**: `GET /api/admin/providers`; `POST /api/admin/providers`; `PUT /api/admin/providers/{id}`; `DELETE /api/admin/providers/{id}` (with `GET .../{id}/impact`); `POST /api/admin/providers/{id}/test` (starts a run), `GET /api/admin/test-runs/{id}`; `GET/PUT /api/admin/model-defaults`.

### SearchPage (US-2.5)
- **State**: three `SearchKindCard`s, active kind, test results.
- **Behaviour**: each card shows its settings (SearXNG URL, or key status), Save, Test and "Make active"; exactly one card is marked "Active". With none active the page says "Search is not set up, so @search is unavailable."
- **API**: `GET /api/admin/search`; `PUT /api/admin/search/{kind}`; `POST /api/admin/search/{kind}/test`; `PUT /api/admin/search/active`.

### GitHubPage (US-2.6)
- **State**: connector status (not configured, saved), account login, expiry, repositories.
- **Behaviour**: `TokenForm` with `SecretField`; the permissions notice is always visible (BR-17.4); "Test connection" shows the account, expiry and up to 100 repository names with the total; "Remove" asks for confirmation and states what stops working (BR-17.6).
- **API**: `GET /api/admin/connectors/github`; `PUT /api/admin/connectors/github`; `DELETE /api/admin/connectors/github`; `POST /api/admin/connectors/github/test`.

### SandboxPolicyPage (US-2.9)
- **State**: current policy, edited policy, dirty flag, container runtime probe result.
- **Behaviour**: `RunnerTierField` offers "In-process" always and "Container" only after a successful runtime check (button "Check container runtime"); `ToolAllowListField` lists the registry with descriptions, `command_run` disabled with "Commands are not available in this version"; `NetworkRuleField` defaults to "No network access" and offers a host list; `LimitsFields` show default, range and unit, a "Reset to default" button per field, with a note that memory and CPU are enforced by the container runner only. A banner states "Changes apply to runs that start after you save. Runs in progress keep their policy." Saving shows the audit-logged toast.
- **API**: `GET /api/admin/sandbox-policy`; `PUT /api/admin/sandbox-policy`; `POST /api/admin/sandbox-policy/check-runtime`.

## 4. Form validation rules (server always re-validates)
| Field | Client-side rule | Message pattern |
|---|---|---|
| Provider name | 1 to 60 characters | "Enter a name up to 60 characters" |
| Base URL | Valid http(s) URL, no credentials, query or fragment | The BR-19 reason |
| Model name | 1 to 200 characters, unique in the list | "Enter the model name exactly as the provider expects it" |
| Context length | Whole number, 1 to 10,000,000 | "Enter a whole number of tokens" |
| SearXNG URL | As Base URL | As above |
| API key, token | Any non-empty text; never trimmed in the middle | "Enter the key" (only when required to set one) |
| Network host | Host name, optional single leading `*.`, at most 253 characters | "Enter a host name such as api.example.com" |
| Limits | Ranges of A2; each field shows its factory default and a "Reset to default" action | The range and the unit |

## 5. Interaction flows
```text
New provider:   Providers -> Add -> save -> Test -> read capabilities -> choose defaults
Local runner:   Add (http://localhost:11434/v1, no key) -> Test -> defaults
Switch search:  Search -> configure Brave -> Test -> Make active
GitHub:         GitHub -> paste token -> Test -> see repositories
Sandbox:        Sandbox policy -> choose tier -> adjust tools, network, limits -> Save
```

## 6. Accessibility and state handling
- Everything meets the U1 baseline (US-13.7, BR-12): keyboard operable, visible focus, labelled controls, errors beside fields and in a summary, statuses never by colour alone, live regions for test progress and results, dialogs return focus to their opener.
- Tests that take seconds show progress and can be left running while the admin keeps reading; the result is announced when it arrives.
- Secrets live only in the field while being typed and are cleared on submit; no secret is ever in the URL, in browser storage or in a toast.
- Session state comes from the server (`/auth/me`); hiding an admin link is convenience, the server enforces access (BR-21.3).
