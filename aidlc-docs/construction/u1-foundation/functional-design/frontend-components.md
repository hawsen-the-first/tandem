# U1 Foundation: Frontend Components

Screens, components, state, validation and the API each uses. The API paths are **provisional**: the contract is finalised as an OpenAPI description in Code Generation. Technology choices (component library, styling approach, state library) are made in NFR Requirements; this document stays technology-neutral.

## 1. Component hierarchy

```text
App
  ThemeProvider                     light, dark, system; remembered choice
  AnnouncerRegion                   screen-reader announcements for toasts and errors
  Router
    PublicLayout
      SetupPage                     only while no user exists
      SignInPage
        MfaChallengeForm            code or recovery code
      AcceptInvitationPage
      ResetPasswordPage
    RequireAuth (any scope)
      MfaEnrolmentPage              also the only page for enrolment-only sessions
    RequireAuth (full scope)
      AppShell
        Header (InstanceName, ThemeToggle, UserMenu)
        Navigation (collapses to a menu on narrow screens)
        Outlet
          HomePage                  recovery-code reminder, session list
          ChatSessionListPage       skeleton
          ChatSessionPage           skeleton: message list and composer
          AccountPage               ChangePasswordForm, MfaSection
        RequireAdmin
          UsersPage                 UserTable, InviteDialog, RoleDialog, ConfirmDialog, ResetLinkDialog
          SettingsPage              SecuritySettingsForm
          AuditLogPage              AuditFilters, AuditTable
  NotFoundPage                      also shown for admin routes to non-admins (BR-9.2)
```

## 2. Shared components

| Component | Purpose | Props and state | Behaviour |
|---|---|---|---|
| `FormField` | Label, input, hint and error | `label`, `error`, `hint`, `required`; value owned by the form | Error linked to the input for assistive technology; never colour-only |
| `PasswordField` | Password with requirement checklist | `value`, `onChange`, `showRules`; local `revealed` | Live checklist: 10+ characters, upper, lower, digit, symbol; show/hide toggle; breached result appears after submit |
| `CopyOnceSecret` | Show a one-time secret (invite link, reset link, recovery codes) | `secret`, `label`, `onDismiss`; local `copied`, `acknowledged` | "Shown once" warning, copy button with confirmation, dismiss disabled until the user ticks "I have saved this" |
| `ConfirmDialog` | Explicit confirmation | `title`, `body`, `confirmLabel`, `requireTyped?` | Focus trapped; destructive actions can require typing the user's email |
| `ResponsiveTable` | Lists that become cards on narrow screens | `columns`, `rows`, `emptyState` | Keyboard navigable; no horizontal page scroll |
| `Toast` | Outcome messages | `kind`, `text` | Announced politely; auto-dismiss never hides an error |
| `ErrorBoundary` | Unexpected failures | | Generic message, no technical detail |
| `RequireAuth`, `RequireAdmin` | Route guards | `scope` | `RequireAdmin` renders `NotFoundPage` for non-admins; enrolment-only sessions are redirected to `MfaEnrolmentPage` |

## 3. Screens

### SetupPage (US-1.1)
- **State**: `status` (loading, available, unavailable), form fields (display name, email, password, confirm), `submitting`, field errors.
- **Flow**: load setup status; if unavailable show `NotFoundPage`; submit; on success go to `MfaEnrolmentPage`.
- **API**: `GET /api/v1/setup/status`, `POST /api/v1/setup`.

### SignInPage and MfaChallengeForm (US-1.2, US-13.3)
- **State**: email, password, `step` (credentials, mfa), `useRecoveryCode`, `retryAt`, `submitting`, one generic form error.
- **Flow**: submit credentials; if the server asks for MFA show the code form; "Use a recovery code" swaps the input; on success go to home (or `MfaEnrolmentPage` for an enrolment-only session).
- **Throttling UI**: when the server returns a retry time, show a countdown, disable submit until it ends, use the same wording for delay, lock and rate limit.
- **API**: `POST /api/v1/auth/sign-in` (with optional `code` or `recovery_code`), `GET /api/v1/auth/me`.

### AcceptInvitationPage (US-1.3)
- **State**: token validity (checking, valid, invalid), display name, password, confirm.
- **Flow**: validate the link; invalid shows "This link is not valid. Ask an admin for a new one"; valid shows the form; success signs the user in.
- **API**: `GET /api/v1/invitations/{token}`, `POST /api/v1/invitations/{token}/accept`.

### ResetPasswordPage (US-1.7)
- **State**: as above with new password and confirm.
- **Flow**: validate link; set password; success sends the user to `SignInPage` with a notice (and, if MFA was reset for an admin, enrolment follows sign-in).
- **API**: `GET /api/v1/password-reset/{token}`, `POST /api/v1/password-reset/{token}/complete`.

### MfaEnrolmentPage (US-1.5)
- **State**: `stage` (intro, scan, verify, codes), secret and QR from the server, entered code, recovery codes (held in memory only).
- **Flow**: start enrolment; show QR and secret text; verify a code; show recovery codes in `CopyOnceSecret` with a download option; finish returns to home. For admins the page cannot be skipped, and the rest of the interface is not reachable until it completes.
- **API**: `POST /api/v1/auth/mfa/enrol/start`, `POST /api/v1/auth/mfa/enrol/confirm`.

### AccountPage (US-1.5, US-1.6)
- **ChangePasswordForm**: current, new (with `PasswordField` rules), confirm; success shows a toast that other devices were signed out. **API**: `POST /api/v1/auth/password/change`.
- **MfaSection**: shows MFA state; members can enrol, regenerate recovery codes (confirmation required), or disable (password and code required); admins see "required" with no disable control. **API**: `POST /api/v1/auth/mfa/enrol/start`, `POST /api/v1/auth/mfa/recovery-codes/regenerate`, `POST /api/v1/auth/mfa/disable`.

### UsersPage (US-1.3, US-1.4, US-1.7, US-1.8)
- **State**: users and pending invitations (lists), selected user, dialog state, loading and error.
- **Table columns**: name, email, role, status (active, deactivated, pending), last sign-in, actions.
- **Actions**: Invite; Revoke invitation; Change role; Issue reset link (with "Also reset MFA" tick); Deactivate; Reactivate. Actions that would remove the last admin are disabled with an explanation, and the server enforces the rule too.
- **InviteDialog**: email and role; on success shows the link in `CopyOnceSecret`.
- **ResetLinkDialog**: confirms scope ("signs the user out everywhere"), then shows the link in `CopyOnceSecret`.
- **API**: `GET/POST /api/v1/admin/users` (list), `GET/POST /api/v1/admin/invitations`, `DELETE /api/v1/admin/invitations/{id}`, `POST /api/v1/admin/users/{id}/role`, `/deactivate`, `/reactivate`, `/reset-link`.

### SettingsPage (US-1.9)
- **State**: current values, edited values, dirty flag, validation errors.
- **Fields**: sign-in idle timeout, absolute duration, online breach check toggle; each shows its default, range and the effect ("applies to new sign-ins").
- **API**: `GET/PUT /api/v1/admin/settings`.

### AuditLogPage (US-2.7)
- **State**: filters (user, event type, date range), page cursor, rows, loading.
- **Behaviour**: newest first, load more, filter changes reset the cursor; each row shows time, actor, event, object and outcome; no edit or delete control; shows a note that entries are append-only.
- **API**: `GET /api/v1/admin/audit?user=&type=&from=&to=&cursor=`.

### Walking skeleton screens (no story completion)
- **ChatSessionListPage**: list own sessions, create one with a title. **API**: `GET/POST /api/v1/chat-sessions`.
- **ChatSessionPage**: message list, composer (1 to 4000 characters, plain text), live updates, agent replies labelled "mock-agent". **API**: `GET/POST /api/v1/chat-sessions/{id}/messages`, WebSocket `/api/v1/events` (subscribes to the session; falls back to reload on disconnect).

## 4. Form validation rules

| Field | Client-side rule (server always re-validates) | Message pattern |
|---|---|---|
| Email | Valid shape, at most 254 characters | "Enter a valid email address" |
| Display name | 1 to 80 characters | "Enter a name up to 80 characters" |
| Password | BR-2.1 and BR-2.2 live; breached check on submit | The unmet requirement, never the password |
| Confirm password | Equals password | "Passwords do not match" |
| TOTP code | 6 digits | "Enter the 6-digit code" |
| Recovery code | Non-empty | "Enter one of your recovery codes" |
| Settings | Ranges in BR-10.1; absolute at least idle | The range and the unit |
| Skeleton title, message | BR-13.1, BR-13.2 | The length limit |

Passwords are never kept in state after a failed submit and are never written to logs or the URL.

## 5. Interaction flows

```text
First run:       Setup -> MFA enrolment -> Home
Invitation:      Link -> Accept (name, password) -> [admin: MFA enrolment] -> Home
Sign-in:         Email and password -> [MFA code] -> Home
Admin invite:    Users -> Invite -> copy link (shown once) -> send out of band
Forced reset:    Users -> Issue reset link -> copy link -> user opens link -> new password -> Sign-in
Skeleton:        Home -> New session -> post message -> mock agent reply appears live
```

## 6. Responsive, theme and accessibility baseline (US-13.7, BR-12)

- **Breakpoints**: designed from 360 px up. Under about 720 px the navigation collapses to a menu and tables become stacked cards; above about 1100 px content is capped to a readable width.
- **Themes**: light, dark and system; a toggle in the header; the choice is remembered; both themes meet contrast requirements.
- **Accessibility checklist** (applied to every screen in every unit): all functions keyboard-operable; visible focus; logical focus order and focus return after dialogs; every control labelled; errors and toasts announced; headings form an outline; contrast at least 4.5:1 for text and 3:1 for controls; touch targets at least 24 by 24 CSS pixels, 44 for primary actions; no information by colour alone; respects reduced-motion; no time limits except sign-in timeouts, with a clear message when one expires.
- **Verification**: each screen has an automated accessibility check and a keyboard walk-through in its tests (NFR-7.1); the checklist is the acceptance criterion for US-13.7 and is re-run by every later unit.

## 7. State and data handling
- **Session state** comes only from the server (`/auth/me`): user, role, scope. The client never decides authority; hiding an admin menu item is convenience, the server enforces access (BR-9).
- **Secrets in the UI** (invite links, reset links, recovery codes) live only in component memory while displayed and are cleared on dismissal.
- **Errors** are mapped to the messages in `business-rules.md` (Error handling); unexpected errors show a generic message and a request reference for support.
