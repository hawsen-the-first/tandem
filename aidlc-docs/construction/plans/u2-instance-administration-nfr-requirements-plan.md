# NFR Requirements Plan: U2 Instance administration

**Stage**: CONSTRUCTION - NFR Requirements, **Light** (unit `u2-instance-administration`)
**Inputs**: U2 functional design (BR-14 to BR-21, W16 to W24), U1 NFR requirements and technology decisions (the baseline this unit inherits)
**Scope**: only what U2 adds or changes. Everything in U1's `nfr-requirements.md` still applies (security headers, rate limits, audit, redaction, TLS, observability, accessibility); this file does not repeat it.

## Steps
- [x] 1. Analyse the functional design
- [x] 2. Ask the questions below (answered in chat)
- [x] 3. Generate `nfr-requirements.md` and `tech-stack-decisions.md`
- [ ] 4. Approval (pull request)

## Questions and your answers
| # | Question | Answer |
|---|---|---|
| Q1 | Which container runtimes does the probe support, and how is it configured? | **Docker only.** An optional setting `TANDEM_CONTAINER_SOCKET` names the socket; unset by default, so the container tier is unavailable until an operator opts in. Podman's Docker-compatible socket may work but is unsupported |
| Q2 | Should U2 add an optional SearXNG service to the development Compose file? | **No.** The bundled service arrives with the reference Compose in U8. U2 tests search against mock servers and any SearXNG a developer runs. US-2.5's first acceptance bullet is therefore verified in U8 (noted in the story map) |
| Q3 | How does the server talk to the GitHub REST API? | **Plain REST calls on our own outbound HTTP client**, so every call is covered by the outbound URL rules (BR-19); no GitHub SDK |

## Working assumptions (not questions; confirm at approval)
- **N1**: Scale is small: at most 20 providers, 200 models in total, three search kinds, one connector and one policy. Lists are not paginated.
- **N2**: Test actions use a dedicated rate-limit bucket of 10 per minute per source, in addition to one-at-a-time per target.
- **N3**: A provider test probes models with a concurrency of 4; if the 60 s budget runs out, unfinished models are recorded as `timeout` and the partial result is stored.
- **N4**: Remote error text is never relayed to the admin. A test result carries only the fixed statuses and messages of BR-14.6 (a defence against using a test to read internal services).
- **N5**: The U1 log redaction layer treats `key` and `api_key`, `private_key` as sensitive names; it did not (a gap found while designing U2) and is extended in code generation.
- **N6**: Docker is probed with a single read-only `GET /version` over the Unix socket, with a 3 s timeout and a 64 KiB response cap; the API process never starts, lists or stops containers.
