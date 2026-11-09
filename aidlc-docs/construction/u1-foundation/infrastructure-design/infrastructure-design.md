# U1 Foundation: Infrastructure Design

Maps the U1 logical components (`nfr-design/logical-components.md`) to the infrastructure Tandem's developers and CI use. Tandem is self-hosted software: there is no cloud provider and no hosted environment. Production packaging (images, reference Compose, signing, SBOM, release) is U8.

## 1. Element overview

| Element | Where it runs | Purpose | Satisfies |
|---|---|---|---|
| PostgreSQL 18 container | Developer machine (Compose) and CI | Single database with TLS and two roles | SE-1, SE-7, SC-3 |
| Certificate generator (one-shot) | Developer machine (Compose) | Creates a private CA and server certificate for the database | SE-1 |
| Dev secrets generator | Developer machine (`just dev-init`) | Creates random database passwords, vault master key and metrics token | SE-2, SE-13, SECURITY-09 |
| Native `api`, `worker`, Vite dev server | Developer machine | Run the apps with fast rebuilds | Assumption I3 |
| Minimal CI | GitHub Actions | Build, test, lint, dependency scan, contract check, end-to-end and performance smoke | SE-11, MT-1 to MT-8, PF-6 |
| Dependabot | GitHub | Dependency and Action update PRs | SECURITY-10 |
| CodeQL and secret scanning | GitHub | Static analysis, committed-secret prevention | SECURITY-10, SECURITY-13 |
| Repository settings | GitHub (owner action) | Branch protection and required checks | SE-14, RESILIENCY-03 |
| Toolchain pins and Nix dev shell | Repository | Reproducible tool versions | SECURITY-10 |

## 2. PostgreSQL (dev and CI)

- **Image**: official PostgreSQL 18 image pinned to an exact version tag (to be fixed at Code Generation; by digest in U8). Runs as the image's non-root `postgres` user.
- **Settings**: `ssl=on` with certificate and key from the shared certificate volume; `ssl_min_protocol_version=TLSv1.2`; `password_encryption=scram-sha-256`; access rules accept only `hostssl` connections with `scram-sha-256`.
- **Roles** (created on first start by an init script using the generated passwords):
  - `tandem_migrate`: owns the schema; used only by `tandem migrate`.
  - `tandem_app`: used by `api` and `worker`; granted data access on application tables by migrations, and `INSERT` and `SELECT` only on the audit table (NFR design 5.4).
  - A superuser exists only inside the container for initialisation; its password is generated and never used by the apps.
- **Health**: Compose health check with `pg_isready`; the apps wait for readiness through `/readyz`, not through Compose ordering.
- **Storage**: named volume `tandem_dev_pgdata`; `just dev-reset` removes it.
- **Network**: a Compose network `tandem_dev`; the database port is published on `127.0.0.1` only (default 5432, configurable in `dev/.env`).
- **Disk encryption**: not applicable to developer machines; in production it is an operator control documented in the self-hosting guide (NFR-5.4).

## 3. Certificate generator

- A one-shot container (small pinned image with OpenSSL) that creates, if absent, a private CA and a server certificate valid for `localhost`, `127.0.0.1` and the Compose service name, into the bind mount `dev/secrets/certs` (so the native apps can read the CA file directly). It is idempotent and sets file permissions so only the database user can read the key.
- The CA certificate is exposed read-only to the apps as `TANDEM_DATABASE_CA_FILE`; the apps connect with certificate verification (`verify-full`).
- The same generator is reused by the reference Compose file in U8; operators can replace the certificates with their own.

## 4. Secrets in development

- `just dev-init` writes, on first run only, a git-ignored `dev/.env` (random passwords for the three database users and the metrics token) and `dev/secrets/master.key` (random 256-bit key, file mode 0600). `dev/.env.example` lists variable names with no values.
- Nothing secret is committed; nothing is a default or fixed value (SECURITY-09, SECURITY-12). Secret scanning with push protection is a second barrier (section 8).
- The apps read secrets only through the `*_FILE` variables or the environment (configuration pattern in NFR design 9).

## 5. Developer workflow commands (`just`, Q6 A)

| Recipe | Does |
|---|---|
| `just dev-init` | One-time: generate dev secrets and certificates |
| `just dev-up` / `just dev-down` | Start or stop PostgreSQL (and the certificate generator on first start) |
| `just dev-reset` | Stop and delete the database and certificate volumes, then re-initialise |
| `just migrate` / `just migrate-down` | Apply or revert migrations under the migration role |
| `just api`, `just worker`, `just web` | Run the apps natively with the dev configuration |
| `just test`, `just test-web`, `just e2e` | Rust tests (throw-away PostgreSQL containers), web unit tests, Playwright with accessibility checks |
| `just lint` | `rustfmt` check, `clippy` with warnings denied, ESLint, Prettier check, TypeScript check |
| `just audit` | `cargo audit`, `cargo deny`, `npm audit` |
| `just openapi` | Regenerate the OpenAPI description and the TypeScript client |
| `just perf` | Run the performance smoke test |
| `just check` | Everything CI runs, in the same order |

## 6. Toolchains (Q2 B)

- `rust-toolchain.toml` pins an exact stable Rust version with `rustfmt` and `clippy`; `rustup` honours it automatically.
- A Node version file and the `engines` field pin Node.
- A Nix flake (`flake.nix` and a committed `flake.lock`) provides a dev shell with the pinned Rust, Node, `just`, `cargo-audit`, `cargo-deny`, `sqlx`-related tooling and Playwright dependencies. It is optional: developers without Nix use the pinned files and the documented steps. Docker (or Podman) is assumed to be installed on the host for PostgreSQL and the tests.

## 7. Local network and HTTPS (Q7 A)

- Browser to `http://localhost:5173` (Vite dev server). Vite proxies `/api` and the event WebSocket to the `api` process on `127.0.0.1:8080`, so the browser sees one origin and the `__Host-` cookie, CSRF and Origin rules behave as in production.
- `TANDEM_PUBLIC_URL` is `http://localhost:5173` in development. The configuration loader accepts `http://` public URLs only when the host is a loopback address; any other host must use `https://`, so a production misconfiguration fails at start-up.
- Known limitation: Safari does not accept Secure cookies on `http://localhost`; developers use Chrome or Firefox for local work. CI and the end-to-end suite use Chromium and Firefox.

## 8. GitHub: CI, scanning and settings

### 8.1 CI (minimal in U1)
One workflow, `ci.yml`, on pull requests and pushes to `main`. All third-party Actions are pinned to full commit SHAs, workflow permissions default to `contents: read`, jobs have timeouts, and a concurrency group cancels superseded runs. Runner image `ubuntu-24.04`.

| Job | Steps |
|---|---|
| `rust` | Format check, `clippy` (deny warnings), unit and integration tests (throw-away PostgreSQL 18 containers started by the tests, including migrations up and down), `cargo audit`, `cargo deny` (advisories, licences GPL-3.0 compatible, sources), unused-dependency check |
| `web` | `npm ci`, TypeScript check, ESLint, Prettier check, Vitest, `npm audit` |
| `contract` | Regenerate OpenAPI and the TypeScript client and fail if the committed files differ |
| `e2e` | Start PostgreSQL with TLS through the dev compose file, run migrations, start `api` and `worker`, run Playwright (Chromium and Firefox) with axe accessibility checks and the walking-skeleton test; upload traces on failure |
| `perf` | Short load test against sign-in, a typical read and the skeleton round trip; fail beyond the PF-1 to PF-3 targets |

Required status checks for `main`: `rust`, `web`, `contract`, `e2e`, `perf` and CodeQL.

### 8.2 Scanning and updates
- **CodeQL** workflow for JavaScript/TypeScript and Rust on pull requests and weekly, with `security-events: write` permission only for that job (Q4 A).
- **Secret scanning and push protection** enabled in the repository settings (owner action).
- **Dependabot**: `dependabot.yml` for `cargo` (repository root), `npm` (`web/`) and `github-actions` (`/`), weekly and grouped, plus Dependabot security updates (Q3 A).

### 8.3 Repository settings to enable (owner action, listed here so they are not forgotten)
- Protect `main`: require a pull request with one approving review, require the status checks above, block force pushes and deletion.
- Enable secret scanning, push protection, Dependabot alerts and security updates.
- Add `CODEOWNERS` entries (committed in U1) so changes under `.github/`, `deny.toml`, `flake.nix` and the database migrations need owner review (SECURITY-13: pipeline changes are access-controlled).
- Add `SECURITY.md` (committed in U1) with a private vulnerability-reporting route; enable GitHub private vulnerability reporting.

## 9. Repository scaffold created in U1
```text
Cargo.toml  Cargo.lock  rust-toolchain.toml  rustfmt.toml  deny.toml
flake.nix  flake.lock  justfile  .node-version  .editorconfig  .gitignore
CODEOWNERS  SECURITY.md  README.md (developer section)
crates/   apps/   web/   tests/
dev/compose.yaml  dev/certgen/  dev/.env.example
.github/workflows/ci.yml  .github/workflows/codeql.yml  .github/dependabot.yml
```
The crate and app directories follow `unit-of-work.md` section 4; U1 creates the crates it needs and leaves the others to later units.

## 10. Security and resiliency evaluation (infrastructure level)

| Rule | Status | Evidence |
|---|---|---|
| SECURITY-01 Encryption | Compliant | Dev and CI databases require TLS 1.2+ with verification (sections 2, 3) |
| SECURITY-06 Least privilege | Compliant | Two database roles; audit table insert and select only; PostgreSQL runs as a non-root user; CI permissions read-only by default |
| SECURITY-07 Network | Compliant | Published ports bind to `127.0.0.1`; database reachable only on the internal network and the loopback port |
| SECURITY-09 Hardening | Compliant | No default or hard-coded credentials; generated secrets (section 4); loader rejects non-loopback `http` URLs |
| SECURITY-10 Supply chain | Compliant | Lock files; pinned toolchains, images and Action SHAs; `cargo audit`, `cargo deny`, `npm audit`; Dependabot; SBOM for production images in U8 |
| SECURITY-12 Credentials | Compliant | No credentials in source; secrets only from files or environment |
| SECURITY-13 Integrity | Compliant | SHA-pinned Actions, protected branch, CODEOWNERS on pipeline files, CodeQL, secret scanning |
| SECURITY-02, 03, 04, 05, 08, 11, 14, 15 | N/A at infrastructure level | Covered in NFR Requirements and NFR Design (no deployed network intermediary in U1) |
| RESILIENCY-06 Health checks | Compliant | Database health check; app health endpoints in NFR design |
| RESILIENCY-03 Change management | Compliant | Protected `main`, required review and checks |
| RESILIENCY-04, 12 | N/A for U1, delivered in U8 | Deployment automation, rollback by image version, backup and restore |
| Other RESILIENCY rules | Operator-owned | NFR-5.4 |
