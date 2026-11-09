# U1 Foundation: Technology Stack Decisions

Decisions confirmed or made in NFR Requirements. NFR-1.5 deferred these choices to this stage. Items marked **proposed** were not asked as questions; they follow from the NFRs and can be changed at approval.

## 1. Decisions from your answers

| # | Decision | Rationale | Alternatives considered |
|---|---|---|---|
| T1 | **Argon2id benchmarked at first start** to about 250 ms, parameters stored per hash (Q1 C) | Fits the sign-in budget (PF-2), adapts to small or large hosts, can be raised later without locking users out | OWASP minimum profile fixed; stronger fixed profile |
| T2 | **No automatic audit pruning; app role insert and select only** on the audit table (Q2 B) | Meets SECURITY-14 (application cannot delete its own audit logs, 90-day minimum); simplest tamper resistance | Separate pruning job under its own role (more moving parts) |
| T3 | **TLS required to PostgreSQL; the reference Compose provisions a certificate** (Q3 A) | Meets SECURITY-01 without extra operator effort | Operator-supplied certificate only; optional TLS (needs an exception) |
| T4 | **Headless accessible primitives with our own CSS and design tokens** (Q4 A) | Strict CSP with no runtime style injection; full control of both themes; accessibility from tested primitives | Full component kit; hand-built components |
| T5 | **Performance targets** PF-1 to PF-3 (Q5 A) | Matches NFR-3.1 | Relaxed targets |
| T6 | **OpenTelemetry tracing, exported only when an endpoint is set** (Q6 A) | Satisfies tracing for a multi-service system without forcing a collector on every self-hoster | Correlation IDs only |
| T7 | **Integration tests use throw-away PostgreSQL containers** (Q7 A) | Isolated and repeatable; the same Docker dependency the unit and CI already have | Shared dev database |
| T8 | **Vault master key from a file path (preferred) or environment variable, with key IDs for rotation** (Q8 A) | Unattended restarts; rotation path; no key in the repository or images | Start-up passphrase; external KMS (later adapter) |

## 2. Back end (proposed unless noted)

| Area | Choice | Notes |
|---|---|---|
| Language and runtime | Rust, Tokio | NFR-1.2 |
| HTTP | Axum with Tower middleware | NFR-1.2; middleware order: request ID, access log, security headers, rate limit, session, CSRF, authorisation |
| Database access | SQLx, compile-time checked queries, reversible migrations | NFR-1.2, NFR-5.5; every statement parameterised (SECURITY-05) |
| IDs | UUIDv7 | Sortable, unguessable enough alongside authorisation checks |
| Password hashing | `argon2` crate (Argon2id) with per-hash parameters | T1 |
| Secrets | AES-256-GCM (`aes-gcm` or `ring`), envelope with key ID | T8 |
| TOTP | RFC 6238 crate, constant-time comparison, replay guard | BR-4 |
| Random tokens | OS random source, at least 256 bits | BR-8.1 |
| Sessions and CSRF | Own implementation: opaque tokens hashed in PostgreSQL; CSRF token per session in a header | N9, BR-8; avoids a general session library hiding security-critical behaviour |
| Rate limiting | `tower-governor` or `governor` keyed by source; account guard stored in PostgreSQL | BR-3.4, SE-12 |
| Validation | Typed request structs, validation crate, `serde` with `deny_unknown_fields` | SE-6 |
| Breached passwords | Bundled offline list (compact Bloom filter built from the Have I Been Pwned password list or a comparable list; the dataset and its licence and attribution are confirmed in Code Generation) plus the optional online k-anonymity check | BR-2.3; the online check uses a timeout and circuit breaker (RL-2) |
| TLS | `rustls` for database and outbound HTTP | SE-1 |
| Logging | `tracing` with JSON output and a redaction layer | SE-4 |
| Tracing | `tracing-opentelemetry` with OTLP exporter, off by default | T6 |
| Metrics | `metrics` with a Prometheus exporter on an internal port | OB-1, SE-10 |
| Config | Environment variables and files via a typed config loader; secrets only from files or environment | SE-13 |
| API description | `utoipa` (or equivalent) generating OpenAPI | MT-3 |
| Testing | `cargo test`, `testcontainers` for PostgreSQL, a scripted mock model provider | T7, MT-2, MT-6 |
| Static checks | `rustfmt`, `clippy` (deny warnings), `cargo audit`, `cargo deny`, unused-dependency check | SE-11, MT-7 |

## 3. Front end (proposed unless noted)

| Area | Choice | Notes |
|---|---|---|
| Language and build | TypeScript (strict), Vite, React | NFR-1.1, Application Design D7 |
| Routing and data | React Router, TanStack Query | Server state cached and invalidated by events |
| UI primitives | Radix UI or React Aria (final pick in Code Generation) plus CSS Modules and CSS custom properties for design tokens | T4; no CSS-in-JS runtime |
| Themes | Token sets for light and dark switched by an attribute on the root element, following the system by default | BR-12.2 |
| QR codes | A bundled client-side QR library rendering the `otpauth` URI | No external service; secret shown only to the enrolling user |
| API client | Generated from OpenAPI | MT-3 |
| Real time | One WebSocket, reconnect with backoff, reload on gap | U1 skeleton, deepened in U3 |
| Testing | Vitest and Testing Library; Playwright with axe-core | MT-4, MT-5 |
| Static checks | ESLint, Prettier, `tsc --noEmit`, `npm audit` | MT-7, SE-11 |
| Assets | Fonts and scripts bundled and served from the same origin | SE-5, SE-14 |

## 4. Delivery and tooling

| Area | Choice | Notes |
|---|---|---|
| CI | GitHub Actions: minimal in U1 (build, test, lint, dependency scan), extended in U8 | NFR-5.8, Units Generation answer |
| Branch protection | `main` requires pull request review and passing CI | SE-14; repository setting you enable |
| Dev environment | `dev/compose.yaml` runs PostgreSQL (with TLS) only; the app runs with `cargo run` and `vite dev` | Units Generation answer |
| Images, signing, SBOM, reference Compose | U8 | Units Generation answer |

## 5. Open points for Code Generation
- Final choice between Radix UI and React Aria.
- The breached-password dataset, its licence and attribution text.
- Exact Argon2id benchmark bounds (minimum profile, maximum memory).
