# U2 PR 5: web, end-to-end, documentation

Steps 33 to 41 of the code generation plan. Admins can now do everything from PR 4 in the web client,
under **Integrations** in the header.

## What was added

| Area | Contents |
|---|---|
| Foundations (step 33) | `Integrations` header link (admins only) and `AdminIntegrationsLayout`, a labelled `nav` of links rather than tabs. The `messages.integrations.ts` catalogue. Shared components in `web/src/components/integrations/`: `SecretField`, `StatusBadge`, `TestResultView`, `UrlField`, `HostListEditor` and `CapabilityWarning`. Client mirrors of BR-19 and BR-18.4 (`outboundRules.ts`). `useTestRun`, which polls a run every 1 s for at most 90 s and follows the active run when a start answers 409. `ApiError.runId` |
| ProvidersPage (step 34) | Provider table, with the last-test badge and Test, Edit and Remove per row. `ProviderDialog` with `ModelListEditor`, which takes declared overrides for tool calling and context length and shows capability warnings and disagreements. `ProviderTestPanel`, with per-model results arriving live. `DefaultModelsCard`, where each option names its provider and an unsuitable pick needs "Use anyway". `RemoveProviderDialog`, which names the defaults it clears |
| SearchPage (step 35) | A card for each of SearXNG, Brave Search and Tavily, with settings, key status, Save, Test and Make active. A page banner when no service is active, and "Turn search off" |
| GitHubPage (step 36) | Token form, an always-visible minimum-permissions notice, the test result with account, expiry and repositories, and removal with a confirmation of what stops working |
| SandboxPolicyPage (step 37) | Runner tier: "Container" is offered only after a successful runtime check. Tool list, with `command_run` disabled and its reason. Network rule with the host list editor. Limits show default, range and unit, each with "Reset to default". The "applies to new runs" banner, a version and an unsaved-changes marker |
| Accessibility (step 38) | An axe check on every page and dialog. Keyboard tests for each page. The four pages added to the e2e axe run in both themes and to the 360 px check |
| End to end (step 39) | `web/e2e/mocks/server.mjs`, a Node stand-in on loopback with an OpenAI-compatible provider (a capable and a weak model) and SearXNG, started and stopped by `dev/scripts/e2e.sh`. `web/e2e/integrations.spec.ts` covers 7 scenarios (details below) |
| Documentation (step 40) | `docs/integrations.md`, a new admin guide. `docs/api.md` gains the integrations section and the 409 `already_running` code. `docs/development.md` gains `TANDEM_CONTAINER_SOCKET` with its warning, outbound calls and the e2e mocks. README links |

The 7 end-to-end scenarios:
- provider add, test and capabilities, then defaults with an acknowledged warning, then removal that clears both defaults;
- a key never shown again, and a public `http://` URL refused with its reason;
- SearXNG save, test, activate and turn off;
- GitHub token save and remove;
- policy edit, then reset, then the container tier unavailable without a socket;
- members get "not found" in the browser and 404 from the API;
- dialog axe in both themes, with focus moving into the dialog and back to its opener.

## Behaviour worth knowing

- **Secrets never stay in the browser.**
  - The key or token field is cleared as the request is sent, success or failure.
  - Tests check that the value is in neither the DOM nor browser storage afterwards.
  - Secrets never appear in a URL or a toast.
- **Tests are followed, not awaited.**
  - The page keeps working while a run is queued or running.
  - The summary line is a polite live region.
  - Polling stops after 90 s with a sentence saying the test keeps running.
  - A list with an unfollowed active run refreshes every 2 s until it ends.
- **The server decides.**
  - Client checks catch typing mistakes only.
  - Server field errors are shown beside the field they name: `name`, `base_url`, `api_key`, `models`, `token`, `runner_tier`, `tool_allow_list`, `network` and each limit.

## Defects the end-to-end run found (fixed here)

1. **"Make active" and "Reset to default" broke WCAG 2.5.3 (label in name).** Their accessible names did not
   contain the visible text. The names are now "Make active: {kind}" and "Reset to default: {limit}".
   jsdom's axe does not run that rule; the browser run does.
2. **`accessibility.spec.ts` sent every page load from one client address.** With four more pages it went over the
   300-per-minute per-source allowance, which also covers static assets, and a lazy chunk failed to load. Each test
   now uses its own address, as each spec file already did. Real browsers cache the immutable assets, so this is a
   test harness limit, not a product defect.
3. **The skip-link keyboard test pressed Tab before the lazy-loaded shell had rendered.** It failed in local Chromium
   on macOS. It now waits for the main content first, as the accessibility spec does.

## Deviations from the plan

- **Shared components** are in `web/src/components/integrations/` rather than directly in `components/`, to keep
  the U2 set together.
- **The pages were written in this session, one after another,** over the shared components, not by parallel agents
  (risk R7). The review safeguard is unchanged: every page has unit, axe and keyboard tests, and is covered end to end.
- **The GitHub test action** is covered by adapter and web tests only, as planned. Its API base is fixed to
  `api.github.com`.

## Verification

- Web: typecheck, ESLint, Prettier, and 228 unit tests (61 of them new) all pass.
- End to end: the full suite passes, 93 tests in Chromium and Firefox, including 7 new scenarios.
  - It ran locally on macOS against Docker PostgreSQL.
  - Playwright's browsers were installed with `npx playwright install`, because the flake provides them on Linux only.
- `just check` (every CI gate) passes: see the U2 summary.
  - On macOS it first failed in a U1 test. `pipeline.rs` expected `process_resident_memory_bytes`, a gauge that is
    read from `/proc` and so exists only on Linux. That one assertion now runs on Linux only, matching the telemetry
    unit test. CI runs on Linux, so it was never affected.
