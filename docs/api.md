# API notes

The authoritative description is generated from the handlers: `apps/api/openapi.json` (also served at
`GET /api/openapi.json`). This file explains what the description cannot.

## Conventions
- JSON everywhere, UTF-8. Request bodies deny unknown fields.
- Authentication is the `__Host-tandem_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`).
- **State-changing requests** (anything but GET, HEAD, OPTIONS) from a signed-in caller need the
  `X-CSRF-Token` header (from `GET /api/auth/me`) and an `Origin` equal to `TANDEM_PUBLIC_URL`. Public
  state-changing routes (sign-in, setup, link endpoints) check `Origin` only when it is present.
- Errors are `{"error": {"code", "message", "fields"?, "run_id"?}}`. Codes are stable; messages are safe to show.
  | Status | Codes |
  |---|---|
  | 400 | `bad_request` |
  | 401 | `unauthenticated`, `invalid_credentials` |
  | 403 | `forbidden`, `csrf`, `bad_origin`, `missing_origin`, `mfa_enrolment_required` |
  | 404 | `not_found` (also for admin routes called by non-admins), `invalid_link` |
  | 409 | `already_running` with `run_id` (a test of that target is queued or running) |
  | 413, 415 | `payload_too_large`, `unsupported_media_type` |
  | 422 | `validation` with `fields` |
  | 429 | `rate_limited` with `Retry-After`, `too_many_connections` |
  | 503 | `busy` with `Retry-After` |
  | 500 | `internal` (detail is in the server log only) |
- Tokens (invitation, reset) travel in bodies, never in URLs.
- Every response carries `X-Request-Id`; quote it when reporting a problem.

## Sessions and scopes
A sign-in is `full` or `mfa_enrolment_only`. An admin who has not confirmed MFA can only reach
`/api/auth/*` MFA endpoints and `/api/auth/me`; everything else answers 403 `mfa_enrolment_required`.
Role and scope are read from the database on every request: a demotion or deactivation applies to the very
next call.

## Integrations (admin)
Routes under `/api/admin/` for providers, default models, search, the GitHub connector, the sandbox policy and
test runs are admin-only (a full-scope admin sign-in); anyone else gets 404, anonymous callers 401. The admin
guide is [integrations.md](integrations.md).

- **Secrets are write-only.** Requests carry `api_key` (providers, Brave, Tavily) or `token` (GitHub);
  responses carry only `api_key_saved` and `api_key_saved_at` (or `configured` and `saved_at`). An absent or
  empty key keeps the saved one; `"remove_api_key": true` deletes it. No response, audit entry, log line or
  test result ever contains a secret.
- **Tests run in the background.** `POST .../test` (and `POST /api/admin/runtime/check`) answers 202 with a
  test run: `{id, target, status, requested_at, started_at?, finished_at?, failure?, results}`. Poll
  `GET /api/admin/test-runs/{id}` until `status` is `succeeded` or `failed`; results arrive one per probe
  (`subject` is `provider`, `model`, `search`, `connector` or `runtime`) with a fixed `status`
  (`ok`, `unreachable`, `timeout`, `unauthorised`, `unknown_model`, `bad_response`, `refused_address`,
  `not_configured`), a fixed message and the facts learned (`details`). No remote text reaches a result.
  A second start while one is active answers 409 `already_running` with the active `run_id`. Test starts draw
  on their own rate-limit bucket (10 per minute per source).
- Each resource also reports `active_test_id` (a run in progress) and `last_test` (the newest finished run).
- `GET /api/admin/sandbox-policy` returns the policy with its factory values, the allowed ranges and the tool
  registry, so clients hard-code none of them. `PUT` ignores any version or scope; each save raises the version
  by one, and the container tier needs a fresh successful runtime check.
- Validation errors name the field (`name`, `base_url`, `models`, `api_key`, `token`, `runner_tier`,
  `tool_allow_list`, `network` or a limit's name); URL refusals carry the BR-19 reason.

## Event feed
`GET /api/events?after=<id>` upgrades to a WebSocket. The browser sends the cookie; `Origin` must match.
Frames from the server:

| Frame | Meaning |
|---|---|
| `{"type":"ready","latest_event_id":N}` | Connected; N is where the feed starts (or the `after` you sent) |
| `{"type":"event","id","kind":"message.posted","chat_session_id","message_id","author_is_agent"}` | Something you may see happened; fetch the message over HTTP |
| `{"type":"resync","latest_event_id":N}` | Your `after` is older than the retained history (24 h); reload your view |

Up to 10 connections per user. The server pings every 25 s and drops a connection after two missed pongs. On
reconnect, send the last event ID you handled.

## Operations endpoints
`/healthz` (process up), `/readyz` (database within 1 s, schema version, vault canary; 503 while shutting down;
no detail in the body), `/metrics` (Prometheus text; `Authorization: Bearer <TANDEM_METRICS_TOKEN>`, 404 when no
token is configured). The worker serves the same three on its own port.
