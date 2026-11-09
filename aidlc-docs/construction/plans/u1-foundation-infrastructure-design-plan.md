# Infrastructure Design Plan: U1 Foundation and walking skeleton

**Stage**: CONSTRUCTION - Infrastructure Design (unit `u1-foundation`, light)
**Inputs**: U1 functional design, NFR requirements and design (approved), `unit-of-work.md`, execution plan
**Scope in U1**: the developer environment, the repository scaffold and the minimal CI. Image building, the reference Compose file, signing, SBOM and release automation belong to U8 (Units Generation answer: minimal CI now, packaging last).

## Categories evaluated
| Category | Applicability in U1 | Treatment |
|---|---|---|
| Deployment environment | Tandem is self-hosted software; there is no cloud target and no hosted environment for the project | Developer machines and GitHub Actions only; operator deployment is U8 |
| Compute | Processes run natively in development (`cargo run`, `vite dev`); containers only for dependencies | Assumptions I1 to I3 |
| Storage | One PostgreSQL database with TLS and two roles (NFR design 5.4) | Question 1 and assumptions I4 to I6 |
| Messaging | PostgreSQL queue and `LISTEN/NOTIFY` (decided); no broker | No infrastructure to add |
| Networking | Localhost-only bindings; internal container network for the database | Question 7 and assumption I7 |
| Monitoring | Metrics, logs and traces exist in the app; local viewing tools are optional | Question 5 |
| Shared infrastructure | The dev compose file and CI are shared by every later unit | `shared-infrastructure.md` records them |

## Working assumptions (not questions)
- **I1**: CI runs on GitHub-hosted runners with a pinned image (`ubuntu-24.04`). Every third-party GitHub Action is pinned to a full commit SHA, workflow permissions default to read-only, and a concurrency group cancels superseded runs (SECURITY-10, SECURITY-13).
- **I2**: Docker Compose v2 syntax is used so the files also work with Podman Compose.
- **I3**: `api` and `worker` run natively during development; Compose provides only PostgreSQL, the certificate generator, the migration one-shot and the optional observability profile.
- **I4**: The dev database uses the same TLS and two-role setup as production, with generated credentials: the first start creates random passwords into a git-ignored `dev/.env`. There are no default or hard-coded credentials (SECURITY-09, SECURITY-12).
- **I5**: The database image is pinned to an exact version tag (by digest in U8), and its data lives in a named volume that a reset command can drop.
- **I6**: A one-shot certificate generator creates a private CA and server certificate into a volume shared read-only with the database and used as `TANDEM_DATABASE_CA_FILE` by the apps (NFR design 5.4).
- **I7**: All published ports bind to `127.0.0.1`; only the database port is published, for local tools.
- **I8**: CI jobs: Rust (format, clippy with warnings denied, tests using throw-away PostgreSQL containers, `cargo audit`, `cargo deny`), web (type check, lint, unit tests, `npm audit`), contract (generated OpenAPI and client must match), end-to-end (Playwright with accessibility checks against a stack started in the job), and a performance smoke job. Branch protection on `main` requires a reviewed pull request and these checks (a repository setting you enable).
- **I9**: Repository scaffold follows the layout in `unit-of-work.md` section 4 (`crates/`, `apps/`, `web/`, `tests/`, `dev/`, `.github/workflows/`); toolchain versions are pinned in the repository (Rust via `rust-toolchain.toml`, Node via an engines field and version file).

## Questions

## Question 1
Which PostgreSQL major version should development, CI and (later) the reference Compose file target?

A) PostgreSQL 18: the newest major with the longest remaining support window

B) PostgreSQL 17: one major behind, widely deployed

C) PostgreSQL 16: older, still widely available on operator hosts

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 2
How should developer toolchains (Rust, Node, database tools) be provisioned?

A) Pinned files only: `rust-toolchain.toml`, a Node version file and documented install steps; developers bring their own tools

B) Pinned files plus a Nix flake dev shell that provides every tool at the pinned versions (suits NixOS and Nix users; others can ignore it)

C) A dev container definition that provides everything inside a container

D) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 3
How should dependency and GitHub Action updates be handled (SECURITY-10 patch management)?

A) Dependabot (built into GitHub): scheduled grouped update pull requests for Cargo, npm and GitHub Actions, plus security alerts

B) Renovate: more configurable, needs a separate app or runner

C) No automation in U1: updates by hand

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4
Should CI include static security analysis beyond dependency scanning?

A) Yes: CodeQL for JavaScript/TypeScript and Rust on pull requests and weekly, plus GitHub secret scanning and push protection

B) Secret scanning and push protection only

C) Not in U1

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
What local monitoring tools should the dev environment offer (the dashboard and alert files themselves are written in U8)?

A) An optional Compose profile `observability` (off by default) running Prometheus, Grafana and an OpenTelemetry collector with a trace viewer, pre-wired to the apps' metrics and traces

B) Nothing extra: developers inspect `/metrics` and logs directly

C) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 6
Which task runner should wrap the common developer commands (start dependencies, reset the database, run migrations, run all checks)?

A) `just`: a simple, cross-platform command runner with a short `justfile`

B) `make`: available almost everywhere, more awkward syntax

C) `cargo xtask` plus npm scripts: no extra tool, but commands are split between two ecosystems

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
How should local development handle HTTPS? (The session cookie is `Secure` with the `__Host-` prefix.)

A) Plain `http://localhost`: Chrome and Firefox accept Secure cookies on localhost; Safari does not, which is documented as a known limitation

B) A local TLS reverse proxy in the dev Compose file with a locally generated certificate, matching production behaviour on every browser

C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Execution Checklist (generation, after answers are approved)
- [x] 1. Map the U1 logical components and NFR design to infrastructure elements (dev compose, certificate generator, migration one-shot, optional observability profile, CI); write `infrastructure-design.md`
- [x] 2. Describe the development and CI deployment architecture, networking, secrets handling and lifecycle commands; write `deployment-architecture.md`
- [x] 3. Record the shared infrastructure that later units reuse; write `aidlc-docs/construction/shared-infrastructure.md`
- [x] 4. Re-evaluate Security Baseline (SECURITY-01, 06, 07, 09, 10, 12, 13) and applicable Resiliency rules against the infrastructure
- [x] 5. Update state, log the approval prompt, present the completion message

## Extension compliance (planning)
- Security Baseline: enforced; infrastructure evidence recorded at checklist step 4
- Resiliency Baseline: RESILIENCY-04 and RESILIENCY-12 delivery remains in U8; operator-owned decisions unchanged
- Property-Based Testing: disabled

---

## Consequences of the answers
| Answer | Consequence |
|---|---|
| Q1 A | PostgreSQL 18 everywhere (dev, CI, reference Compose in U8) |
| Q2 B | `rust-toolchain.toml`, a Node version file and a Nix flake dev shell with a lock file |
| Q3 A | `dependabot.yml` for Cargo, npm and GitHub Actions, grouped and weekly, plus security updates |
| Q4 A | A CodeQL workflow for JavaScript/TypeScript and Rust (pull requests and weekly) and repository secret scanning with push protection |
| Q5 B | No observability profile in the dev compose; developers read `/metrics` (with a dev token) and logs. U8, when it writes dashboards, may add one |
| Q6 A | A `justfile` wraps developer commands; `just` is included in the Nix dev shell |
| Q7 A | Development runs on `http://localhost` through the Vite dev server. The configuration loader rejects an `http://` public URL unless its host is a loopback address, so production cannot run without HTTPS. Safari's refusal of Secure cookies on localhost is documented as a known limitation |

## Additional assumptions made while generating
- **I10**: In development the migration runs natively (`just migrate`) because the apps run natively; the one-shot migration service appears in the reference Compose file in U8.
- **I11**: The dev database port is published on `127.0.0.1` only, default 5432, changeable in `dev/.env`.
- **I12**: A `SECURITY.md` with a private vulnerability-reporting route and a `CODEOWNERS` file covering `.github/`, `deny.toml`, `flake.nix` and the database migrations are committed in U1.
