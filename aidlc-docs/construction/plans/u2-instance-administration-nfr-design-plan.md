# NFR Design Plan: U2 Instance administration

**Stage**: CONSTRUCTION - NFR Design, **Light** (unit `u2-instance-administration`)
**Inputs**: U2 NFR requirements (SC-5 to SE-26, T5 to T15), U2 functional design, U1 NFR design (patterns and components LC-1 to LC-23, which stand)

## Steps
- [x] 1. Analyse the NFR requirements
- [x] 2. Ask the questions below (answered in chat)
- [x] 3. Generate `nfr-design-patterns.md` and `logical-components.md`
- [ ] 4. Approval (pull request)

## Questions and your answers
| # | Question | Answer |
|---|---|---|
| Q1 | How is "one test at a time per target" (RL-8) enforced across several api instances? | **A PostgreSQL advisory lock per target** (recommended). With tests as jobs (Q3) it is paired with a unique "one active run per target" rule at enqueue, so a second request answers "already running" at once |
| Q2 | How do configuration changes reach the other processes (SC-6)? | **A time-limited cache only, 5 s** (recommended). The saving process clears its own cache immediately |
| Q3 | How do tests run (PF-8 up to 60 s, proxies cut at 60 s)? | **As a background job; the screen polls** (recommended). Starting a test answers at once with a run; the worker executes it and stores progress and results |

## Working assumptions (not questions; confirm at approval)
- **D1**: Every test action is a job of the same kind (`config_test`), not only the slow provider test: provider, search, GitHub and container-runtime tests share one state machine, one status endpoint and one screen behaviour. Added latency is the worker's pick-up time (about a second).
- **D2**: A failed *test* is a successful *job* (the failure is the result). A job that crashes is retried by the queue's normal rules (U1 BR-13.3); after the last attempt the run is recorded as `failed` with "The test could not complete".
- **D3**: Test runs are kept: the latest per target always, and up to the last 10 per target for 30 days (the U1 cleanup job prunes the rest). "Last test result" on the screens (A4) is the latest finished run.
- **D4**: The screen polls a run once a second while it is queued or running and stops when it finishes or after 90 s ("The test is taking longer than expected; reload to check later").
- **D5**: The policy table carries an integer `version` that increases with each saved change; a run records the version it captured, so an audit or a bug report can say which policy applied.
- **D6**: The config read port (LC-31) is the only code that unseals configuration secrets for runtime use. Callers get a short-lived in-memory connection object (base URL and secret) that does not implement `Debug`, `Display` or serialisation.
- **D7**: The outbound client resolves names with its own resolver on the async runtime (no blocking lookups) and does not honour proxy environment variables, so no call can bypass the address checks through a proxy.
