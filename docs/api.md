# API notes

The authoritative description is generated from the handlers: `apps/api/openapi.json` (also served at
`GET /api/openapi.json`). This file explains what the description cannot.

## Conventions
- JSON everywhere, UTF-8. Request bodies deny unknown fields.
- Authentication is the `__Host-tandem_session` cookie (`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`).
- **State-changing requests** (anything but GET, HEAD, OPTIONS) from a signed-in caller need the
  `X-CSRF-Token` header (from `GET /api/auth/me`) and an `Origin` equal to `TANDEM_PUBLIC_URL`. Public
  state-changing routes (sign-in, setup, link endpoints) check `Origin` only when it is present.
- Errors are `{"error": {"code", "message", "fields"?}}`. Codes are stable; messages are safe to show.
  | Status | Codes |
  |---|---|
  | 400 | `bad_request` |
  | 401 | `unauthenticated`, `invalid_credentials` |
  | 403 | `forbidden`, `csrf`, `bad_origin`, `missing_origin`, `mfa_enrolment_required` |
  | 404 | `not_found` (also for admin routes called by non-admins), `invalid_link` |
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
