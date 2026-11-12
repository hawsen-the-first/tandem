# U2 Instance Administration: Technology Stack Decisions

Decisions for U2 only; U1's stack (`u1-foundation/nfr-requirements/tech-stack-decisions.md`) stands. Items marked **proposed** were not asked as questions; they follow from the NFRs and can be changed at approval.

## 1. Decisions from your answers
| # | Decision | Rationale | Alternatives considered |
|---|---|---|---|
| T5 | **Docker-only container probe**, opt-in through `TANDEM_CONTAINER_SOCKET` (Q1) | Matches what the container runner (U7) will target; keeps the sensitive socket decision with the operator; the probe is read-only | Docker and Podman; no probe until U7 |
| T6 | **No SearXNG service in U2**; the reference Compose in U8 provides it (Q2) | Keeps U2 free of infrastructure; the adapter and test action are fully testable against mocks | An optional dev Compose profile |
| T7 | **Plain REST on our own outbound client for GitHub** (Q3) | Every call is covered by the outbound URL rules (SE-18); few dependencies; we need only a handful of endpoints | `octocrab` or another SDK |

## 2. Proposed decisions
| # | Decision | Rationale |
|---|---|---|
| T8 | **Outbound rules in two places**: the pure URL rules (scheme, host classes, blocked ranges, normalisation) in `core` as `OutboundUrl`; the client in a new crate `outbound` that wraps `reqwest` (already in the tree, rustls with `ring`) with a custom DNS resolver, a connect-time address check, a manual redirect policy and the size cap (SE-18, SE-19) | Keeps `core` free of I/O while making the rules testable as plain functions; one chokepoint for every outbound call. A CI check (clippy `disallowed-methods`) forbids constructing other HTTP clients in the application crates |
| T9 | **Adapters in the crates the unit plan reserved**: the OpenAI-compatible provider in `providers` (beside the mock), search adapters in a new `search` crate, the GitHub connector in a new `github` crate. Each speaks through the `outbound` client and implements a port in `ports` | Matches the planned layout (`unit-of-work.md` section 4); later units add adapters without touching core |
| T10 | **Provider calls use small typed JSON structs** for `GET {base}/models` and `POST {base}/chat/completions` (including a forced `echo` tool choice for the probe); no provider SDK | The OpenAI-compatible surface needed here is tiny and every compatible server differs in small ways; owning the types keeps the probe precise |
| T11 | **The Docker probe is a hand-written HTTP/1.1 `GET /version` over `tokio::net::UnixStream`**, 3 s timeout, 64 KiB cap | One read-only request does not justify a Docker client library, and it keeps the attack surface to a fixed request line |
| T12 | **Storage**: new reversible migrations create `providers`, `provider_models`, `model_defaults`, `search_configs`, `connectors`, `sandbox_policies` and `config_test_results`, with sealed `bytea` columns for secrets and foreign keys that clear or remove dependants as BR-15.3 and RL-11 require; the app role gets select, insert, update and delete on them | Same pattern as U1; the policy table has a `scope` column so session overrides need no migration (BR-18.8) |
| T13 | **Configuration cache**: a small in-process time-limited cache (5 s) in the read port, with the write path clearing it locally; no cache library | SC-6 needs nothing more |
| T14 | **No new frontend libraries**; the U2 screens reuse the U1 shared components, with the new `SecretField`, `StatusBadge`, `TestResultView`, `UrlField`, `HostListEditor` and `CapabilityWarning` built on them | Strict CSP and the established conventions |
| T15 | **Test doubles**: in-process mock servers on loopback built with `axum` (already a dependency of `api`) in each adapter crate's tests | MT-9 without adding a mocking library |
