# U2 PR 4: API

Steps 28 to 32 of the code generation plan. Every administration capability is now reachable over
HTTP by an admin; no screen uses it yet (PR 5).

## Endpoints (all `AdminUser`: admin with a full-scope sign-in; anyone else gets 404, anonymous 401)

| Area | Routes |
|------|--------|
| Providers | `GET/POST /api/admin/providers`, `GET/PUT/DELETE /api/admin/providers/{id}`, `GET .../{id}/removal-impact`, `POST .../{id}/test` |
| Default models | `GET/PUT /api/admin/model-defaults` |
| Search | `GET /api/admin/search`, `PUT /api/admin/search/kinds/{kind}`, `POST .../kinds/{kind}/test`, `PUT /api/admin/search/active` |
| GitHub | `GET/PUT/DELETE /api/admin/connectors/github`, `POST .../github/test` |
| Sandbox policy | `GET/PUT /api/admin/sandbox-policy` (the GET also returns factory values, ranges and the tool registry so the editor hard-codes none of them), `POST /api/admin/runtime/check` |
| Test runs | `GET /api/admin/test-runs/{id}` |

## Behaviour worth knowing

- **Secrets are write-only.** Requests carry `api_key` (or `token`); responses carry `api_key_saved` and `api_key_saved_at` only. An absent or blank key means unchanged; `remove_api_key: true` deletes it.
- **Test starts answer 202** with the run, **409 `already_running` with `error.run_id`** when one is active, and draw from a new rate-limit bucket `Test` (10 per minute per source) that is separate from the general bucket.
- **Test results hold fixed statuses and messages only.** No remote text, key or response body reaches a result.
- **Validation errors name the field** (`name`, `base_url`, `models`, `tool_allow_list`, `network`, each limit, `runner_tier`, `token`), including the URL reasons of BR-19.7.
- Malformed IDs and unknown search kinds in a path are 404, matching the other routes.
- `PUT /api/admin/sandbox-policy` ignores any version or scope; the saved version is one more than the version it replaces, and the container tier needs a fresh runtime probe.

## Also changed

`ErrorBody` gained an optional `run_id` (only set for `already_running`); OpenAPI and the generated web types were regenerated; the pinned-routes test already requires every `/api/admin` route to be `AdminUser`, so the new routes are covered by it.

## Verification

fmt, clippy `-D warnings`, `cargo test --workspace` (including the Docker suites), offline sqlx check,
deny, machete, the outbound lint check and the web typecheck, lint, format and unit tests pass. The
existing end-to-end suite was run as a smoke test: all 63 tests pass in Chromium and Firefox. New tests: 18 in-memory API
tests (access for anonymous, member and half-enrolled admins; CSRF; no secret in any response or
audit entry; key keep, replace and remove; URL refusals with reasons; field-level validation;
removal impact and clearing; the test-run lifecycle including 409 with the run id; rate limiting on
test starts with other routes unaffected; search, GitHub and policy flows; before and after values
in the audit log) and one Docker Postgres test that creates a provider, runs its test through the
real OpenAI-compatible adapter against a loopback mock server, and checks that the key reached the
provider, is sealed at rest, and appears in no response, audit entry or captured log line.
