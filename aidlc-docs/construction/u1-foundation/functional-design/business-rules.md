# U1 Foundation: Business Rules

Rules are numbered **BR-group.n** and referenced from `business-logic-model.md`. Numeric values marked (A) are assumptions listed under "Assumptions".

## BR-1 Accounts
- **BR-1.1** First-run setup is available only while no user exists. Once any user exists it behaves as if it does not exist ("not found").
- **BR-1.2** Email is the sign-in identifier: trimmed, lower-cased, unique across all users and pending invitations.
- **BR-1.3** Display name is required, 1 to 80 characters, no leading or trailing spaces.
- **BR-1.4** A deactivated user cannot sign in and every attempt fails with the same generic message as a wrong password.
- **BR-1.5** There is no self-registration. Accounts come only from first-run setup or an accepted invitation.

## BR-2 Password policy (FR-1.8)
- **BR-2.1** Length 10 to 128 characters. Passwords are never trimmed or truncated.
- **BR-2.2** Must contain an upper-case letter, a lower-case letter, a digit and a symbol (any character that is not a letter or digit).
- **BR-2.3** Must not appear in the breached-password list. The bundled offline list is always checked. If the admin enables the online k-anonymity check, it is also used; if the online service is unreachable the offline result stands and the event is logged (availability over strictness, because the offline list already applies).
- **BR-2.4** A changed password must differ from the current one.
- **BR-2.5** The same policy applies at setup, invitation acceptance, password change and reset completion.
- **BR-2.6** Failure messages state which requirement was not met (length, character kinds, breached) without echoing the password.

## BR-3 Sign-in protection (FR-1.10, US-13.3)
- **BR-3.1** **Progressive delay**: after the n-th failure within a rolling hour, the next attempt is accepted no sooner than min(2^(n-1), 60) seconds later (1 s, 2 s, 4 s, ... capped at 60 s) (A).
- **BR-3.2** **Lockout**: the 10th failure within an hour locks the account for 15 minutes (A). Attempts during the lock are refused without checking the password.
- **BR-3.3** The failure counter resets on a successful sign-in, or after one hour with no failures.
- **BR-3.4** **Per-source limit**: more than 20 sign-in attempts per minute from one source network address are refused with a rate-limit message (A).
- **BR-3.5** Responses and timing do not reveal whether an email belongs to an account. Unknown emails get the same delay and lock behaviour (tracked by normalised email in a short-lived guard) and a dummy password comparison.
- **BR-3.6** A refused attempt (delay, lock, rate limit) tells the user when to retry, using the same wording for all cases.
- **BR-3.7** An admin cannot manually unlock an account in the MVP; locks expire.

## BR-4 Multi-factor authentication (FR-1.5)
- **BR-4.1** An admin must hold a confirmed MFA credential. Until they do, their session has scope `mfa_enrolment_only`: only MFA enrolment, sign-out and the current-user lookup are allowed. A member promoted to admin is put into the same scope on their next admin-area request.
- **BR-4.2** MFA is optional for members. A member can enrol at any time.
- **BR-4.3** TOTP: 6 digits, 30-second steps, tolerance of one step either side.
- **BR-4.4** A code whose time-step was already accepted for this user is refused (no replay).
- **BR-4.5** Recovery codes: 10 per set, single-use, stored hashed, shown once at enrolment (and at regeneration). Generating a new set replaces all earlier codes.
- **BR-4.6** A member may switch MFA off by confirming their password and a current code. An admin cannot switch MFA off.
- **BR-4.7** A user who loses their device and codes recovers through an admin-issued reset link with "Also reset MFA" ticked (A), or, for the last admin, through the operations CLI (C27).
- **BR-4.8** A wrong code or recovery code counts as a failed sign-in attempt (BR-3).

## BR-5 Invitations (FR-1.3)
- **BR-5.1** Only admins invite. The email must not match an existing user or another pending invitation; role is `admin` or `member`.
- **BR-5.2** An invitation expires 7 days after creation (A).
- **BR-5.3** The link is shown to the issuing admin once. Tandem sends no email in the MVP. Only the token's hash is stored, so the link cannot be shown again (the admin revokes and re-invites instead).
- **BR-5.4** An unknown, expired, revoked or used token shows one generic "This link is not valid" message.
- **BR-5.5** Accepting creates the user with the invited role and makes the invitation single-use.
- **BR-5.6** Admins can revoke a pending invitation at any time; it then behaves like an invalid link.

## BR-6 Password resets (FR-1.6)
- **BR-6.1** Only admins issue reset links. A link expires after 24 hours and is single-use (A).
- **BR-6.2** An unknown, expired or used reset token shows the generic "not valid" message.
- **BR-6.3** Issuing a reset ends all of the user's sessions at once and invalidates any earlier unused reset link for that user.
- **BR-6.4** There is no self-service "forgot password" in the MVP; a user asks an admin.
- **BR-6.5** "Also reset MFA" (A) clears the user's MFA credential and recovery codes when the link is used. An admin whose MFA is cleared must re-enrol (BR-4.1).

## BR-7 Deactivation, reactivation and roles
- **BR-7.1** Only admins deactivate, reactivate, or change roles.
- **BR-7.2** The last active admin cannot be deactivated or demoted. The check runs inside the same transaction as the change so two admins cannot remove each other concurrently.
- **BR-7.3** An admin cannot deactivate themselves; another admin must do it.
- **BR-7.4** Reactivation restores sign-in with the existing password and MFA. Earlier sessions stay ended; the user signs in again.
- **BR-7.5** A deactivated user's past messages, approvals and artifacts stay attributed to them; the user list shows them as deactivated.
- **BR-7.6** Deactivation requires an explicit confirmation naming the user.

## BR-8 Sessions (sign-ins)
- **BR-8.1** A session token is a long random value (at least 256 bits), stored only hashed, delivered in a cookie that is Secure, HttpOnly and SameSite. State-changing requests are protected against cross-site forgery.
- **BR-8.2** Idle timeout and absolute duration are read from the instance settings **when the session is created** and stored with it. A later settings change affects only new sign-ins.
- **BR-8.3** Changing the password ends all of the user's other sessions.
- **BR-8.4** Deactivation, a forced reset, and an MFA reset end all of the user's sessions.
- **BR-8.5** A user may hold several sessions at once (for example phone and laptop).

## BR-9 Authorisation
- **BR-9.1** Role is read from the stored user record on every request. A token never carries authority, so demotion and deactivation take effect immediately.
- **BR-9.2** Admin-only resources (user administration, settings, audit log) respond "not found" to non-admins, so they are indistinguishable from resources that do not exist.
- **BR-9.3** Permission matrix:

| Action | Admin | Member | Not signed in | Enrolment-only session |
|---|---|---|---|---|
| First-run setup | only when no user exists | no | only when no user exists | n/a |
| Accept invitation, use reset link | n/a | n/a | with valid token | n/a |
| Sign in | yes | yes | yes | n/a |
| Change own password, manage own MFA | yes | yes | no | MFA enrolment only |
| Create session, post message (skeleton) | yes | yes | no | no |
| Invite, revoke, deactivate, reactivate, change role, issue reset | yes | no | no | no |
| Edit instance security settings | yes | no | no | no |
| View audit log | yes | no | no | no |

## BR-10 Instance settings
- **BR-10.1** Ranges: idle timeout 15 minutes to 30 days; absolute duration 1 hour to 90 days and at least the idle timeout.
- **BR-10.2** Defaults apply while no value is stored: idle 12 hours, absolute 14 days, online breach check off.
- **BR-10.3** Every change records old and new values in the audit log.

## BR-11 Audit
- **BR-11.1** Each state change is audited in the same transaction. If the audit entry cannot be written, the change does not happen (fail-safe).
- **BR-11.2** Entries are append-only. The running application never updates or deletes entries and there is no automatic pruning (SECURITY-14, decision in NFR Requirements). Operators prune only with a documented command that uses a privileged database credential they control.
- **BR-11.3** A failed sign-in is written as an entry only when the email matches an existing account, so mistyped credentials or passwords typed into the email field are never stored. Failures for unknown emails are counted in metrics only.
- **BR-11.4** `details` never contains passwords, tokens, secrets, recovery codes or full TOTP data.
- **BR-11.5** Event catalogue:

| Event type | Written when |
|---|---|
| `auth.setup` | First-run admin created |
| `auth.sign_in.success`, `auth.sign_in.failure`, `auth.sign_in.locked` | Sign-in outcomes (failure per BR-11.3) |
| `auth.sign_out` | User signs out |
| `auth.mfa.enrolled`, `auth.mfa.disabled`, `auth.mfa.recovery_used`, `auth.mfa.recovery_regenerated` | MFA changes |
| `auth.password.changed`, `auth.password.reset_issued`, `auth.password.reset_completed` | Password changes |
| `user.invited`, `user.invite_revoked`, `user.invite_accepted` | Invitation lifecycle |
| `user.deactivated`, `user.reactivated`, `user.role_changed` | User administration |
| `settings.changed` | Instance setting edited |
| `authz.denied` | A signed-in user attempted an action their role forbids |

## BR-12 Interface baseline (US-13.7)
- **BR-12.1** Layouts work from 360 px wide upward; navigation collapses on narrow screens; all primary actions are reachable without horizontal scrolling.
- **BR-12.2** Light and dark themes follow the system setting with a manual override that is remembered.
- **BR-12.3** Target WCAG 2.2 AA: sufficient contrast in both themes, keyboard operation, visible focus, labelled controls, errors announced to assistive technology, targets of at least 24 by 24 CSS pixels (44 for primary touch actions), and motion reduced when the user prefers it.
- **BR-12.4** Forms show errors next to the field and in a summary, keep what the user typed (except passwords), and never rely on colour alone.
- **BR-12.5** One-time secrets (invite links, reset links, recovery codes) are shown with a "shown once" warning and a copy action, and cannot be retrieved afterwards.

## BR-13 Walking skeleton
- **BR-13.1** A skeleton session is visible only to its owner. Title 1 to 120 characters.
- **BR-13.2** A message body is 1 to 4000 characters and rendered as plain text in U1 (Markdown arrives in U3).
- **BR-13.3** The `MockAgentReply` job retries up to 3 times with growing delay, then is marked failed; the user's message is never lost.
- **BR-13.4** Outbox publishing is at-least-once; handlers are idempotent (a message never produces two agent replies).
- **BR-13.5** The mock provider is deterministic: the same input always gives the same reply.

## Validation summary

| Input | Rule |
|---|---|
| Email | Valid address shape, at most 254 characters, normalised |
| Display name | 1 to 80 characters |
| Password | BR-2 |
| TOTP code | Exactly 6 digits |
| Recovery code | Fixed format, case-insensitive, single-use |
| Role | `admin` or `member` |
| Setting values | Ranges in BR-10.1 |
| Audit filter | Date range valid and at most 10 years; event type from the catalogue |
| Skeleton title and message | BR-13.1, BR-13.2 |

## Error handling

| Situation | User sees | Audit |
|---|---|---|
| Wrong credentials, unknown email, deactivated account | "That email and password do not match" | Failure entry only for existing accounts |
| Delay, lock or rate limit | "Too many attempts. Try again in N seconds" (same wording for all cases) | `auth.sign_in.locked` once when a lock starts |
| Invalid, expired or used link | "This link is not valid. Ask an admin for a new one" | None |
| Password breaks policy | The unmet requirement | None |
| Demoting or deactivating the last admin | "At least one active admin is required" | None |
| Role forbids action | "Not found" for admin resources; "You do not have access" otherwise | `authz.denied` |
| Audit write fails | "Something went wrong. Nothing was changed" | None (the failure is logged operationally) |
| Worker unavailable (skeleton) | Message shows normally; reply arrives later | None |

## Assumptions (confirm at approval)
- **A7** Invitation links expire after 7 days; reset links after 24 hours.
- **A8** Reactivation re-uses existing password and MFA.
- **A9** Delay doubles from 1 second, capped at 60 seconds; per-source limit is 20 attempts per minute.
- **A10** A lockout lasts 15 minutes.
- **A11** Forced reset offers "Also reset MFA", so a member or admin who lost their device and recovery codes can be recovered without operator access. (Adds a criterion to US-1.7.)

## Security Baseline check at functional level

| Rule | Status | How |
|---|---|---|
| SECURITY-05 Input validation | Compliant (design) | Validation summary above; messages and titles are length-limited |
| SECURITY-08 Authorisation | Compliant (design) | BR-9: role read per request, admin resources hidden from non-admins, enrolment-only scope |
| SECURITY-12 Authentication | Compliant (design) | Argon2id and breached-password check (BR-2), MFA required for admins (BR-4), cookie properties and expiry (BR-8), delay, lockout and rate limiting (BR-3), no account enumeration (BR-3.5) |
| SECURITY-14 Audit | Compliant (design) | BR-11 catalogue; same-transaction recording; append-only |
| SECURITY-15 Fail-safe | Compliant (design) | Audit failure blocks the change; invalid links give one generic answer; no partial setups |
| Other SECURITY rules | N/A at this stage | Hashing parameters, headers, TLS, logging format and dependency scanning are decided in NFR Requirements and NFR Design |
| RESILIENCY rules | N/A at this stage | Health, metrics and migration behaviour are covered in NFR stages; the skeleton's durable queue and outbox are designed for restart safety (BR-13.3, BR-13.4) |
