# U2 PR 2: adapters

Steps 11 to 16 of the code generation plan. The four "Test" operations now exist as adapters
behind ports, each tested against loopback doubles. Nothing is wired to the application, the
database or the API yet (PR 3 and 4).

## What was added

| Area | Change |
|------|--------|
| `ports::probe` | `ProviderProbe`, `SearchProbe`, `ConnectorProbe`, `RuntimeProbe`, the `ResultSink`, and connection types. Secrets are held in `Secret` and every connection's `Debug` hides them |
| `support::Secret` | Moved from `config` (which re-exports it) so ports and adapters can use the redacting type without depending on `config` |
| `providers::OpenAiCompatibleProbe` | Lists models (`GET /models`), then per model a chat probe and a forced-`echo` tool probe (A5), reading context length from the listing (A6). 60 s budget, four models at a time, a result sent to the sink as each lands |
| `search::SearchTester` | SearXNG (admin URL, JSON format), Brave and Tavily (fixed endpoints, overridable in tests only). "No results" is a success with a note |
| `github::GithubConnector` | Token test over the outbound client: login, expiry header, up to 100 repositories and an exact total (from the last page number; link URLs are never followed) |
| `sandbox::DockerRuntimeProbe` | One `GET /version` over the Unix socket: 3 s, 64 KiB cap, name and version kept only if they match a strict character set |

## Behaviour worth knowing

- A refused key or an unreachable provider fails the provider and every model without calling any model.
- A provider with no model listing still passes through its models; the provider is `ok` if any model answered (including `unknown_model`, which proves the key works).
- A `404` from the chat endpoint is reported as `unknown_model`. A wrong base path can look the same; the listing result (or its absence) is what tells the two apart on screen.
- SearXNG answering `403` means its JSON format is switched off, so it is a `bad_response`, not `unauthorised`.
- Remote text never reaches a result: tests plant a marker string in error bodies and assert it appears nowhere in results.

## Verification

`cargo fmt --check`, workspace clippy with `-D warnings`, `cargo test --workspace`, `cargo deny check`,
`cargo machete`, `cargo audit` and `dev/scripts/check-outbound-lint.sh` pass. New tests: providers
22 (including 3 timing runs), search 10, github 10, sandbox 10, ports 1.
