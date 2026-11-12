# U2 Instance Administration: NFR Requirements

Additions to the U1 baseline (`u1-foundation/nfr-requirements/nfr-requirements.md`), which still applies in full. IDs continue U1's numbering: **SC** scalability, **PF** performance, **AV** availability, **RL** reliability, **SE** security, **OB** observability, **MT** maintainability and testing, **US** usability.

## 1. Scalability
| ID | Requirement |
|---|---|
| SC-5 | Design size: up to 20 providers, 200 models in total, three search kinds, one connector, one policy (N1). Admin lists are not paginated |
| SC-6 | Configuration is read from PostgreSQL and cached in each process for at most 5 seconds (A12). Several `api` or `worker` instances may therefore show a change up to 5 seconds late; this is accepted for non-security configuration |

## 2. Performance
| ID | Requirement |
|---|---|
| PF-7 | Admin configuration reads and writes complete in under **200 ms at the 95th percentile** (PF-1), excluding test actions |
| PF-8 | A test action never exceeds **60 s** in total or **15 s** per outbound call. Provider tests probe models with a concurrency of 4; models not finished in time are recorded as `timeout` and the partial result is kept (N3) |
| PF-9 | Resolving a model, the active search provider, GitHub credentials or the policy for a run costs at most one database read per 5 seconds per process (SC-6); it makes no outbound call |

## 3. Availability
| ID | Requirement |
|---|---|
| AV-6 | The admin screens and API keep working when any provider, search service or GitHub is down. Outside failures appear only as test results, never as errors on unrelated screens |
| AV-7 | Configuration changes need no restart. Runs in progress keep the policy they started with (BR-18.1); later runs see the change within SC-6 |

## 4. Reliability
| ID | Requirement |
|---|---|
| RL-7 | Every outbound call has a connect timeout of 5 s, a total timeout of 15 s and a response cap of 1 MiB; a response over the cap is a `bad_response` (BR-19.6) |
| RL-8 | A test runs one at a time per target; a second request answers "already running" without starting another |
| RL-9 | A failed audit write blocks the change (U1 BR-11.1). A test whose own audit entry cannot be written still returns its result to the admin and logs the failure |
| RL-10 | `tandem vault rekey` re-seals the new sealed values too (provider keys, search keys, the connector token), verifying each opens first, in the same single transaction as U1's values |
| RL-11 | Removing a provider, replacing its models, and clearing the affected defaults happen in one transaction; a failure leaves everything as it was |

## 5. Security
| ID | Requirement |
|---|---|
| SE-18 | **One outbound path.** Every call the server makes on an admin's behalf goes through a single outbound client that applies BR-19 (scheme and host rules, always-blocked addresses, connect-time address check, redirect rules, size cap, TLS verification). Direct use of an HTTP client elsewhere in the application crates is forbidden and checked in CI (SECURITY-05, SECURITY-11) |
| SE-19 | The connect-time check validates **every address a name resolves to** and connects only to a validated address, so a name that changes after saving, or resolves to a mix, cannot reach a blocked address. Tests cover loopback and private addresses (allowed), link-local and metadata addresses in IPv4 and IPv6 forms (blocked), IPv4-mapped IPv6, decimal and octal host spellings, a rebinding resolver, cross-host redirects and userinfo |
| SE-20 | Secrets are sealed under separate vault purposes (`provider_api_key`, `search_api_key`, `connector_secret`) bound to their row, and unsealed only in memory for the call that needs them. No endpoint, log line, metric, audit detail, error message or test result carries a secret (BR-20; SECURITY-03, SECURITY-12) |
| SE-21 | The log redaction layer treats `key`, `api_key` and `private_key` as sensitive field names in addition to U1's list (N5). A test proves a provider save with a key logs no key |
| SE-22 | Test results show only the fixed statuses and messages of BR-14.6, BR-16.4 and BR-17.3. Remote response text, headers and bodies are never relayed or stored (N4), so a test cannot be used to read an internal service |
| SE-23 | The container socket is **opt-in**: `TANDEM_CONTAINER_SOCKET` is unset by default; when set the API makes only the read-only `GET /version` call (N6). The documentation warns that access to a container socket is root-equivalent on the host and recommends a socket proxy or a separate runner host. Docker only is supported (Q1) |
| SE-24 | All U2 endpoints require an admin with a full-scope sign-in and the U1 CSRF and Origin checks; non-admins receive "not found" (BR-21.3). Test endpoints use a dedicated rate-limit bucket of 10 per minute per source (N2) |
| SE-25 | Response parsing is strict: size-capped (RL-7), JSON depth and entry counts bounded (at most 1,000 models read from a listing), unknown fields ignored, nothing evaluated. Credentials are sent only to the configured host and are never forwarded on a redirect (BR-19.5) |
| SE-26 | Configuration tables get grants through migrations: the app role has select, insert, update and delete on them; they are not audit tables. Every change writes its audit entry in the same transaction (U1 SE-15) |

## 6. Observability
| ID | Requirement |
|---|---|
| OB-5 | Metrics: `outbound_requests_total{target,outcome}` and `outbound_request_duration_seconds{target}` with `target` in `provider`, `search`, `github`, `runtime` (never a host name); `config_test_total{target,status}` |
| OB-6 | A test logs target kind, status and duration, and the host name only (never path, query or credentials). The audit entry `config.test.run` carries the outcome (A8) |

## 7. Maintainability and testing
| ID | Requirement |
|---|---|
| MT-9 | Adapters are tested against in-process mock servers on loopback that imitate an OpenAI-compatible server (with and without tool support, with and without a model listing), SearXNG, Brave, Tavily and GitHub, including slow, oversized, malformed and redirecting answers |
| MT-10 | The outbound URL rules are pure functions with exhaustive table-driven tests; the factory defaults and ranges of the policy limits are constants in one module with tests (A2) |
| MT-11 | Business rules BR-14 to BR-21 each map to at least one test, extending the BR-to-test table |
| MT-12 | The OpenAPI description stays generated and committed; the web client types are regenerated and checked in CI (U1 MT-3) |

## 8. Usability and accessibility
| ID | Requirement |
|---|---|
| US-5 | The U2 screens meet the U1 baseline (WCAG 2.2 AA, keyboard, both themes, 360 px). Test progress and results are announced to assistive technology, and every secret field follows BR-20.3 |

## 9. Compliance summary

### Security Baseline (U2 additions)
| Rule | Status | Where |
|---|---|---|
| SECURITY-03 Application logging | Compliant (design) | SE-20, SE-21, OB-6 |
| SECURITY-05 Input validation | Compliant (design) | SE-18, SE-19, SE-25, BR-19 |
| SECURITY-06 Credentials | Compliant (design) | SE-20, SE-23 |
| SECURITY-08 Authorisation | Compliant (design) | SE-24 |
| SECURITY-11 Secure design | Compliant (design) | SE-18, SE-19, SE-22 (server-side request forgery and response exfiltration) |
| SECURITY-12 Authentication | Compliant (design) | SE-20 |
| SECURITY-14 Audit | Compliant (design) | SE-26, OB-6 |
| SECURITY-15 Fail-safe | Compliant (design) | RL-8, RL-9, RL-11, BR-18.2 |
| Others | Inherited from U1 | No change |

### Resiliency Baseline (U2 additions)
| Rule | Status | Where |
|---|---|---|
| RESILIENCY-05 Monitoring | Compliant (design) | OB-5, OB-6 |
| RESILIENCY-10 Dependency isolation | Compliant (design) | AV-6, RL-7, RL-8, PF-8 |
| Others | Inherited from U1 | No change |

Open blocking findings: **none**.
