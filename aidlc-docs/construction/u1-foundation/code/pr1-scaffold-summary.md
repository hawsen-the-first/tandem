# U1 Code Generation: PR 1 summary (scaffold, development environment and CI)

Plan steps 1 to 8 of `construction/plans/u1-foundation-code-generation-plan.md`. Branch `unit-u1/scaffold`.

## What was created
| Area | Files |
|---|---|
| Rust workspace | `Cargo.toml` (workspace, shared lints: unsafe forbidden, clippy denied, no `unwrap`/`expect`/`todo`/`dbg!`), `Cargo.lock`, `rust-toolchain.toml` (Rust 1.99.0), `rustfmt.toml`, `clippy.toml`, `deny.toml` (advisories, GPL-3.0-compatible licences, crates.io only) |
| Crates (skeletons with a smoke test) | `crates/core`, `ports`, `app`, `support`, `config`, `credentials`, `store-postgres`, `vault`, `providers`, `telemetry` |
| Binaries (skeletons) | `apps/api` (`tandem-api`), `apps/worker` (`tandem-worker`), `apps/cli` (`tandem`), `tools/breach-filter` |
| Toolchain | `flake.nix`, `flake.lock` (Nix dev shell: Rust, Node, `just`, `cargo-audit`, `cargo-deny`, `cargo-machete`, `sqlx-cli`, `actionlint`, `psql`, Playwright browsers), `.node-version` |
| Task runner | `justfile` with every recipe from the infrastructure design; recipes for later layers fail with a clear message |
| Dev environment | `dev/compose.yaml` (PostgreSQL 18.6 with TLS, one-shot certificate generator), `dev/postgres/pg_hba.conf` (TLS only), `dev/postgres/init/01-roles.sh`, `dev/certgen/generate.sh`, `dev/scripts/dev-init.sh`, `dev/scripts/dev-env.sh`, `dev/.env.example` |
| CI and hygiene | `.github/workflows/ci.yml` (`rust`, `web`), `.github/workflows/codeql.yml`, `.github/dependabot.yml`, `CODEOWNERS`, `SECURITY.md`, `.editorconfig`, `.gitignore`, README developer section |
| Web skeleton | `web/` (Vite, React, strict TypeScript, ESLint, Prettier, Vitest, one smoke test, `package-lock.json`) |

## Verification performed
- `just check` passes (exit 0): rustfmt, clippy with warnings denied, 14 crate smoke tests, `cargo audit`, `cargo deny`, `cargo machete`, web lint, format check, type check, Vitest, `npm audit --audit-level=high`.
- `actionlint` passes on both workflow files; all third-party Actions are pinned to full commit SHAs.
- Dev database: `just dev-init` then `just dev-up` start PostgreSQL 18.6 with TLS. Confirmed: a plaintext connection is refused (`no pg_hba.conf entry ... no encryption`); a `verify-full` connection with the private CA works over TLS 1.3; a wrong password and a missing CA are rejected; the app role cannot create tables while the migration role can; `dev/.env`, `dev/secrets/master.key` and the certificate key are git-ignored and mode 600.
- The `web` production build succeeds.
- Not run here: the GitHub workflows themselves (they run when the PR is opened).

## Deviations and findings
| Item | Detail |
|---|---|
| Certificates as a bind mount | The certificate generator writes to `dev/secrets/certs/` (git-ignored) instead of a Docker volume, so the native apps read the CA file directly. The infrastructure design documents were updated |
| ESLint 10 without `eslint-plugin-jsx-a11y` | ESLint 9 is no longer supported and the a11y plugin does not support ESLint 10; the only ESLint 10-compatible fork is very new (0.2.0) and unvetted. Accessibility is instead checked with axe in component tests and Playwright (steps 55 and 56). Revisit when the official plugin supports ESLint 10 |
| Moderate dev-only advisory | `npm audit` reports a moderate `minimatch` denial-of-service advisory reached only through the lint tooling (ESLint and typescript-eslint). It does not affect the shipped client (`npm audit --omit=dev` is clean) and is below the CI threshold (high). The suggested fix is downgrading to unsupported ESLint 9, so it is accepted until upstream patches it |
| `actionlint` added to the dev shell | Workflow files can be linted locally |
| Dependabot covers `docker-compose` | The dev Compose images are kept current |
| Rust pinned at 1.99.0 | Matches the version in the pinned nixpkgs; `flake.nix` warns if `rust-toolchain.toml` and the shell disagree |

## Repository settings still to enable (owner)
Branch protection on `main` (pull request with one review, required checks `rust`, `web` and the CodeQL jobs, no force pushes), secret scanning, push protection, Dependabot alerts and security updates, private vulnerability reporting. They are listed in the README.

## Merge order
PR 1 is stacked on the design PRs: merge #4 (NFR Design and Infrastructure Design) and #5 (Code Generation plan) first.
