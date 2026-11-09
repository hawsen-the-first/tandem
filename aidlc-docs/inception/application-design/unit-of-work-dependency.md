# Tandem: Unit of Work Dependencies

Derived from the component dependency matrix (`component-dependency.md`) and the unit ownership table (`unit-of-work.md`, section 3).

## 1. Dependency matrix

**X** = the row unit needs the column unit to be complete before it can be built or accepted. Blank = no direct dependency.

| Unit | U1 | U2 | U3 | U4 | U5 | U6 | U7 | U8 | U9 |
|---|---|---|---|---|---|---|---|---|---|
| **U1** Foundation | | | | | | | | | |
| **U2** Administration | X | | | | | | | | |
| **U3** Collaboration | X | | | | | | | | |
| **U4** Agents and rounds | X | X | X | | | | | | |
| **U5** Workspaces and GitHub | X | X | X | X | | | | | |
| **U6** Plan, tasks, artifacts | X | | X | X | X | | | | |
| **U7** Container runner | X | X | | X | X | X | | | |
| **U8** Hardening and delivery | X | X | X | X | X | X | X | | |
| **U9** Backlog | X | X | | X | X | X | | | |

## 2. Why each dependency exists

| Unit | Depends on | Reason |
|---|---|---|
| U2 | U1 | Identity and admin role, vault for provider keys, audit, persistence |
| U3 | U1 | Identity, persistence, event bus stub, web shell |
| U4 | U1, U2, U3 | Providers and defaults to call models; threads, messages, approvals and real-time to post into; job queue stub and outbox to deepen |
| U5 | U1, U2, U3, U4 | Connector config; sessions to attach workspaces to; agent runtime for `@code` and the tool layer |
| U6 | U1, U3, U4, U5 | Approvals and threads; orchestration and agents for plan and tasks; workspace snapshots for diffs |
| U7 | U1, U2, U4, U5, U6 | Policy storage, runner interface, workspaces to mount, and tasks to verify enforcement against |
| U8 | U1 to U7 | Packaging and the end-to-end test need every feature in place; Anthropic adapter needs U2 and U4 |
| U9 | U1, U2, U4, U5, U6 | Agent SDK uses the provider port and agent runtime; save to workspace needs artifacts and workspaces |

## 3. Build order and critical path

```text
U1 -> U2 -> U3 -> U4 -> U5 -> U6 -> U7 -> U8 -> U9 (optional)
```

- **Critical path**: U1, U3, U4, U5, U6, U8. These carry the core loop.
- **Possible parallelism (not used, decision Q6 A)**: U2 and U3 depend only on U1, so they could run in parallel if more people or agent sessions become available. U7 could also run alongside U8 once U6 is done.
- **Cut line**: if time runs short, the MVP can ship after U6 plus the packaging part of U8, with the in-process runner only (U7 deferred) and U9 dropped. U7 and the Anthropic adapter would then follow.

## 4. Boundary validation

| Check | Result |
|---|---|
| No cycles between units | Passed: every dependency points to an earlier unit in the build order |
| Every story is in exactly one unit | Passed: 87 of 87 (see `unit-of-work-story-map.md`) |
| Every component is introduced by exactly one unit | Passed: 27 of 27 (see `unit-of-work.md` section 3; stubs are replaced, not duplicated) |
| Every FR reaches a unit | Passed: all 80 FRs are covered by stories that each belong to a unit |
| Security-critical components get NFR stages in full | Passed: C4 (U1), C7 (U3), C16 (U4, U7), C17 (U5), C24 (U1) |
| Cross-unit contracts are ports, events or jobs only | Passed: units interact through the ports and the outbox and queue defined in Application Design |

## 5. Integration and testing checkpoints
- **After every unit**: that unit's unit and integration tests, plus the previous units' tests, run in CI (minimal CI from U1); the unit's demo is performed.
- **After U1**: the walking-skeleton test (sign-in, create session, post message, worker round trip) becomes the first end-to-end test and stays green throughout.
- **After U6**: the reference scenario runs end to end (this is the MVP core loop, goal G1).
- **After U7**: enforcement tests run against both runners with a deliberately hostile scripted model.
- **After U8**: install from the guide on a clean machine; backup and restore round trip.
- **Rollback**: each unit lands as reviewable PRs on its own branch; reversible migrations (NFR-5.5) mean a unit can be reverted without losing earlier units.
