# Tandem

A collaboration workspace for teams to build with AI.

> Status: **Construction, unit U1 (foundation and walking skeleton).** Early scaffold; not usable yet.

## How this project is built

Tandem is scoped and built with **AI-DLC (AI-Driven Development Life Cycle)**, using the
rules-based release **v1.0.1** of [awslabs/aidlc-workflows](https://github.com/awslabs/aidlc-workflows/tree/v1.0.1)
(commit `e40f6a9`). That is the last release before AI-DLC was repackaged as harness plugins and skills in v2.7.0.

| Path | Purpose |
|---|---|
| `CLAUDE.md` / `AGENTS.md` | AI-DLC core workflow (identical copies, for Claude Code and other agents) |
| `.aidlc-rule-details/` | Stage-by-stage rule details and opt-in extensions (MIT-0, see `LICENSE-AIDLC`) |
| `aidlc-docs/` | Workflow artifacts: state, audit log, requirements, designs, plans |
| workspace root | Application code (created during Construction) |

### Continuing the workflow

Open the repo in Claude Code (or another agent that reads `CLAUDE.md` / `AGENTS.md`) and say
something like *"Continue the AI-DLC workflow"*. The agent reads `aidlc-docs/aidlc-state.md`
and picks up from the current stage.

Questions are answered in the question files by filling in the `[Answer]:` tags.

## Development

Tandem is a Rust back end (`api` and `worker` services plus an operations CLI) and a React web client.
The development shell pins every tool; Docker (or Podman with Compose v2) provides PostgreSQL.

### Prerequisites
- [Nix](https://nixos.org/download) with flakes enabled (recommended): `nix develop` provides Rust, Node, `just`,
  `cargo-audit`, `cargo-deny`, `sqlx-cli`, `psql` and browsers for the end-to-end tests.
  Without Nix, install the versions pinned in `rust-toolchain.toml` and `.node-version`, plus `just`.
- Docker with Compose v2.

### First-time setup
```sh
nix develop            # or install the pinned tools yourself
just dev-init          # generates random dev secrets (git-ignored); never reuse them elsewhere
just dev-up            # PostgreSQL 18 with TLS; creates certificates on first start
just migrate           # applies migrations under the migration role
just api               # in separate terminals: just worker, just web
```
Open <http://localhost:5173>; the first-run setup screen appears.

### Everyday loop
`just check` runs everything CI runs (format, clippy, tests, web checks, dependency and licence audits).
`just dev-reset` gives a clean database and fresh secrets. `just --list` shows every command.

### Notes
- Development uses plain `http://localhost` behind the Vite dev server. Chrome and Firefox accept `Secure`
  cookies on localhost; **Safari does not**, so use Chrome or Firefox locally.
- The database accepts only TLS connections; the apps verify its certificate against the private CA that
  `just dev-up` generates in `dev/secrets/certs/`.
- Nothing under `dev/secrets/` or `dev/.env` is ever committed.
- Tests that contain credential-like literals (passwords, keys) live in `tests/` directories, `tests.rs` or `*_tests.rs` files. CodeQL ignores those paths (`.github/codeql/codeql-config.yml`) because it reports test passwords as hard-coded credentials; production code is fully scanned.

### Repository settings (owner)
Enable these in GitHub so the checks below are enforced:
- Protect `main`: require a pull request with one approving review, require the status checks
  (`rust`, `web`, `contract`, `perf` and the CodeQL jobs; `e2e` is added with the web client), block force pushes and deletion.
- Enable secret scanning, push protection, Dependabot alerts and security updates, and private vulnerability reporting.

### Layout
```text
crates/   shared libraries (core, ports, app, support, config, credentials, store-postgres, vault, providers, telemetry)
apps/     api, worker, cli
web/      Vite + React client
tools/    breach-filter (builds the offline breached-password filter)
tests/    cross-crate integration, end-to-end and performance tests
dev/      development Compose file and scripts
aidlc-docs/  AI-DLC workflow artifacts (requirements, designs, plans)
```

## License

Tandem is licensed under the [GNU General Public License v3.0](LICENSE).
The vendored AI-DLC rules in `.aidlc-rule-details/` are MIT-0 (see `.aidlc-rule-details/LICENSE-AIDLC`).
