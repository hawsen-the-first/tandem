# Shared Infrastructure

Infrastructure created in one unit and reused by later units. Created in U1 unless noted. Each later unit's Infrastructure Design references this file instead of redefining these elements.

| Element | Created in | Reused by | Owner of changes |
|---|---|---|---|
| Dev Compose file (`dev/compose.yaml`): PostgreSQL 18 with TLS, certificate generator | U1 | Every unit; U8 builds the reference Compose from it | Repository maintainers; changes need review (CODEOWNERS) |
| Dev secrets generator (`just dev-init`) and the git-ignored `dev/.env`, `dev/secrets/` | U1 | Every unit; CI end-to-end job | Same |
| `justfile` developer commands | U1 | Every unit adds recipes where needed | Same |
| Toolchain pins (`rust-toolchain.toml`, Node version file) and Nix flake with lock file | U1 | Every unit | Same; updated by Dependabot and deliberate pull requests |
| CI workflow `ci.yml` (jobs `rust`, `web`, `contract`, `e2e`, `perf`) | U1 | Every unit extends the jobs; U8 adds image, signing, SBOM and release jobs | Same; changes to `.github/` need owner review |
| CodeQL workflow, Dependabot configuration, secret scanning and push protection | U1 | Every unit | Same |
| Branch protection and required checks on `main` | U1 (owner enables) | Every unit; the list of required checks grows with the CI | Repository owner |
| Database roles `tandem_migrate` and `tandem_app` and the TLS requirement | U1 | Every unit adds migrations and grants under the same roles | Migrations under CODEOWNERS review |
| Certificate volume and CA | U1 | U8 reference Compose | Same |
| `SECURITY.md` and `CODEOWNERS` | U1 | Every unit | Repository owner |

## Rules for later units
- A unit may add services to the dev Compose file only as optional profiles unless every developer needs them (for example SearXNG in U4, a container runtime check in U7).
- New CI jobs must follow the same conventions: pinned Actions, read-only default permissions, explicit timeouts, and addition to the required checks.
- New database tables get grants through migrations; any new table that holds security evidence follows the audit table's insert-and-select-only pattern.
- No unit may introduce a hard-coded or default credential.
