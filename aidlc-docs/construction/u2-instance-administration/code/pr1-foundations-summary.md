# U2 PR 1: foundations

Steps 1 to 10 of the code generation plan. No endpoints, tables or screens yet; this PR adds the
pure domain model and the one outbound HTTP path that later PRs build on.

## What was added

| Area | Change |
|------|--------|
| Workspace | New crates `outbound`, `search`, `github`, `sandbox` (the last three are skeletons) |
| `core::outbound_url` | `OutboundUrl` and the address classifier (BR-19): private networks allowed; link-local and cloud-metadata addresses always blocked, including IPv4-mapped, NAT64 and odd spellings; `http` only for loopback, private and local names |
| `core::config` | Provider, model capabilities (detected and declared kept apart, suitability warnings), search and connector settings, test run state machine |
| `core::sandbox_policy` | Tool registry (`CommandRun` can never be enabled), runner tiers, network rule, limit ranges and defaults, validation and the clamp rule for future per-session policies |
| `core::audit`, `core::job`, `core::permissions` | Eight `config.*` audit events (catalogue now 28), `JobKind::ConfigTest`, `Action::ManageIntegrations` |
| `store-postgres::jobs` | JSON mapping for `config_test` jobs. The worker has a placeholder arm until PR 3 |
| `outbound` | `OutboundClient`: resolves through a checking resolver (any blocked answer refuses the name, defeating DNS rebinding), no proxy, no cookies, no pooling, manual redirects limited to the same scheme, host and port, 1 MiB response cap, connect and total timeouts, metrics |
| `config` | Optional `TANDEM_CONTAINER_SOCKET`: an absolute Unix socket path. Unset means the container tier is unavailable |
| `telemetry` | `key`, `api_key` and `private_key` are redacted; `key_id` and `key_saved_at` are allowed. Metric helpers `record_outbound` and `count_config_test` |
| Lint | `clippy.toml` `disallowed-methods` bans building a `reqwest` client outside `outbound`. The `credentials` breach client is exempt with a reason (fixed host). `dev/scripts/check-outbound-lint.sh` plants a violation and proves clippy rejects it; it runs in CI and as `just check-outbound-lint` |

## Verification

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo deny check`, `cargo machete` and `cargo audit` all pass. New tests: outbound URL (8), config
(17), sandbox policy (16), outbound client (19), config socket (2), redaction (2), metrics (1), job
round trip (1), permissions.

## Notes for review

- The socket path is limited to 107 characters because it must fit `sockaddr_un`.
- `key` is redacted as a whole word, so names such as `sort_key` are also redacted. Over-redacting a log field is the safe direction.
- Two clippy entries for `reqwest::blocking` were left out because that feature is not compiled in.
