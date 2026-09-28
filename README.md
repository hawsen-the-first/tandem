# Tandem

A collaboration workspace for teams to build with AI.

> Status: **Inception, Requirements Analysis.** No application code yet.

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

## License

Tandem is licensed under the [GNU General Public License v3.0](LICENSE).
The vendored AI-DLC rules in `.aidlc-rule-details/` are MIT-0 (see `.aidlc-rule-details/LICENSE-AIDLC`).
