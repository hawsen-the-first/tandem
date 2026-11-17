# U3 Collaboration Core: Frontend Components

Screens, components, state and the API each one uses. Paths are **provisional** until the OpenAPI description is
generated in Code Generation.

Follows the U1 conventions:
- strict CSP, CSS Modules and design tokens, native dialogs;
- the shared components `FormField`, `ConfirmDialog`, `ResponsiveTable` and `Toast`;
- WCAG 2.2 AA, keyboard operation, both themes and 360 px (BR-12).

The U1 walking-skeleton chat screens are replaced.

## 1. Hierarchy

```text
AppShell (U1)                               header gains NotificationBell
  SessionListPage                           US-3.1, US-3.5, US-11.6: "My sessions", "Other sessions", Archived filter
    NewSessionDialog                        title, prompt, visibility (open | private)
  SessionLayout                             one session; owns the real-time subscription
    SessionHeader                           title, state badge, visibility badge, Join / Leave, owner menu
    SessionSidebar                          Overview, Chat, thread list with status and unread, MemberPanel
      ThreadList                            US-5.6, US-12.3
      MemberPanel                           US-3.4, US-12.3: members, presence, Add member
    OverviewPage                            US-3.7: prompt, threads by status, awaiting approval first
    ChannelView                             US-5.1, US-5.2: chat or a thread
      ThreadHeader                          title, StatusBadge, ApprovalRecord, Approve / Revoke
      MessageList                           virtualised, grouped by day, "new messages" divider
        MessageItem                         author, time, edited mark, body, attachments, reply count, actions
          MarkdownView                      the sanitised renderer (BR-28)
          AttachmentList                    file chips, inline images (BR-28.3)
          ReferenceChip                     link, or "unavailable"
        ReplyPane                           one-level replies for a message (US-5.3)
      Composer                              Markdown text, @ picker, # reference picker, attach, limits
        MentionPicker                       members first, then others (open sessions)
        ReferencePicker                     threads, messages, uploads in this session
        UploadQueue                         progress, cancel, errors with the limit
      AddMentionedDialog                    "Add them as a member?" (BR-25.8)
    ApproveDialog                           US-7.1: required outcome summary, preview
    RevokeDialog                            US-7.3: optional reason
    SessionSettingsDialog                   owner or admin: visibility, reminder period, transfer ownership
    EndSessionDialog                        US-11.4: simple confirmation (U5 adds the changes warning, U4 the summary)
  NotificationsPage / NotificationBell      US-12.1, US-12.2
RequireAdmin (U1)
  AdminSessionsPage                         US-3.8: every session's metadata, join, manage
  SettingsPage (U1)                         gains the reminder period and instance time zone (US-12.4)
```

## 2. Screens

### SessionListPage
- **Lists.** Two lists, "My sessions" and "Other sessions" (open sessions only), each with:
  - the title, owner and state;
  - a visibility badge, shown for private sessions;
  - the last activity time;
  - an unread dot (members only).
- **Filter.** An "Archived" toggle shows archived sessions.
- **Creating.** "New session" opens `NewSessionDialog`:
  - title and prompt, with live character counts against the limits;
  - visibility as two radio options: "Open to everyone on this instance" (default) and "Private: members only";
  - the workspace kind is fixed to "Managed folder" in U3, shown as text.

  On success, the new session opens.
- **API.** `GET /api/sessions?scope=mine|others&archived=`, `POST /api/sessions`.

### SessionLayout and SessionHeader
- **Data.** Loads `GET /api/sessions/{id}`. A "not found" shows the U1 not-found page (BR-23.3).
- **Real-time.** Opens the real-time subscription with the last sequence it has (BR-29.2), and handles "reload" by
  refetching.
- **Read-only banners.**
  - A non-member reader sees "You can read this session. Join or ask a member to add you." Admins get "Join"
    (`POST /api/sessions/{id}/members` with themselves). Members get "Leave", except the owner.
  - An ended or archived session shows "This session has ended. It is read-only." Every write control is removed,
    not just disabled, and nothing is announced as available.
- **Owner and admin menu.** Settings, End session, Archive or Unarchive.

### OverviewPage (US-3.7)
- **Content.** The prompt (owner can edit while active), then threads grouped as "Awaiting approval" (open and
  reopened), then "Approved".
- **Threads.** Each thread shows its status, last activity and approver. Each links to its thread.
- **Later units.** Plan, tasks and artifacts sections are placeholders hidden until U4 and U6 supply data.
- **API.** `GET /api/sessions/{id}/overview`.

### ChannelView, MessageList and MessageItem (US-5.1 to US-5.4)
- **Loading.** Messages load by sequence: newest page first, older on scroll, using
  `GET /api/sessions/{id}/channels/{cid}/messages?before=`. A "New messages" divider sits at the read position.
- **MessageItem.**
  - The author name. System messages are styled distinctly, and agent messages get an "Agent" tag from U4.
  - The time, with the full date in a tooltip and in `datetime`.
  - "edited" and its time, when edited.
  - The body through `MarkdownView`, plus attachments, references and the reply count.
- **Actions.** Actions sit in a menu that is reachable by keyboard: Reply, Copy link, and Edit and Delete for the
  author. Edit and Delete are hidden when BR-25.4 blocks them, with the reason in the menu.
- **Deleted messages.** A deleted message renders "Message deleted" in place, with its replies still reachable.
- **Accessibility.** The message list is a `log` region. New messages are announced politely, at most once per few
  seconds as a count ("3 new messages"), never by reading every message.

### Composer
- **Input.** Multi-line text with a character count that appears near the limit (from 18,000 of 20,000).
  - Enter sends and Shift+Enter adds a new line, with the alternative documented in a visible hint.
  - Over the limit, Send is disabled and the field shows "Messages can be up to 20,000 characters" (BR-25.1,
    US-13.1).
- **`@` mentions.** `@` opens `MentionPicker`, an ARIA combobox.
  - In an open session it lists members, then "Not in this session" users.
  - In a private session it lists members only.
- **`#` references.** `#` opens `ReferencePicker` with threads, messages and uploads in this session (US-5.4).
- **Attachments.** Attach opens a file chooser, and files can also be dropped or pasted.
  - Each file appears in `UploadQueue` with progress and cancel.
  - Files over 25 MiB are refused before sending with "Files can be up to 25 MiB". The server's refusal for the
    session quota shows "This session has reached its 2 GiB upload limit".
  - At most 10 per message.
- **After sending.** If the response lists mentioned non-members (BR-25.8), `AddMentionedDialog` asks "Add {names}
  as members?" with Add and Not now.
- **Posting.** `POST .../messages`, with replies sent as `parent_id`. Uploads use `POST /api/sessions/{id}/uploads`.
- **Hidden for non-members.** For a non-member reader, the composer is replaced by the join banner.

### MarkdownView (BR-28)
- **Rendering.** It renders Markdown to React elements from an allow-list. It never uses `dangerouslySetInnerHTML`.
- **Links.** External links get `target="_blank" rel="noopener noreferrer"` and an "opens in a new tab" note for
  screen readers.
- **Images.** Images render only for upload URLs of this session. Others render as links.
- **Code.** Code blocks are highlighted with classes only, which is CSP-safe, and have a Copy button.
- **Reuse.** The same component renders prompts, outcome summaries and, later, agent output.
- **Tests.** It has a hostile-input test corpus (scripts, event attributes, `javascript:` and `data:` links,
  malformed HTML, SVG, very long lines, nesting bombs).

### ThreadHeader, ApproveDialog and RevokeDialog (US-5.6, US-7.1 to US-7.3)
- **ThreadHeader.** It shows `StatusBadge` (open, approved, reopened) as text plus an icon.
  - When approved, `ApprovalRecord` adds "Approved by {name}, {time}" and the summary, collapsible.
  - The approval history (including revoked approvals with who, when and reason) is behind "History".
- **ApproveDialog.** It has a required summary field (1 to 5,000 characters) with a live preview through
  `MarkdownView`.
  - Its helper text reads "This summary is recorded as the agreed decision. It takes precedence over the discussion
    when used as context."
  - It confirms with "Approve thread".
  - A conflict ("Already approved by {name}") closes the dialog and refreshes the thread.
- **RevokeDialog.** It has an optional reason. A refusal because of dependents shows the reason and the links it
  names, which come from U6.
- **API.** `POST .../threads/{tid}/approvals`, `POST .../threads/{tid}/approvals/current/revoke`.

### MemberPanel and member dialogs (US-3.4, US-3.8, US-12.3)
- **Members.** Each member shows a presence dot (online or away) as text plus colour, with the owner marked. The
  online members are listed first.
- **Actions.**
  - "Add member" opens a picker of active instance users, for any member.
  - "Remove" is offered to the owner and admins, never on the owner.
  - "Leave" is offered to members other than the owner.
- **Transfer.** `SessionSettingsDialog` lets the owner or an admin transfer ownership to a current member, with a
  confirmation naming them.
- **API.** `GET/POST/DELETE /api/sessions/{id}/members`, `POST /api/sessions/{id}/owner`.

### SessionSettingsDialog (US-3.5, US-3.8, US-12.4)
- **Visibility.** Open or private. Switching to private warns "People who are not members will lose access
  immediately".
- **Quiet-thread reminders.** One of three choices:
  - "Use the instance setting ({value})";
  - a custom period (hours or days, 1 hour to 30 days);
  - "Off".
- **API.** `PATCH /api/sessions/{id}`.

### EndSessionDialog (US-11.4)
- **Confirmation.** It names the session and says it becomes read-only, the managed folder is deleted, and the
  session cannot be reopened. It confirms with "End session".
- **API.** `POST /api/sessions/{id}/end`, plus `/archive` and `/unarchive`.

### NotificationBell and NotificationsPage (US-12.1, US-12.2)
- **Bell.** The header bell shows the unread count. Its accessible name includes the count.
- **List.** It opens a list, newest first: "Ana mentioned you in {thread}" and "{thread} has been waiting for
  approval for {period}". Each links to its target, and an inaccessible target shows "unavailable".
- **Reading.** "Mark all read" is available. Opening a target marks its notification read.
- **Real-time.** Notifications arrive through the user's own real-time channel.
- **API.** `GET /api/notifications`, `POST /api/notifications/read`.

### AdminSessionsPage (US-3.8)
- **Table.** A `ResponsiveTable` of every session: title, owner (flagged if deactivated), visibility, state,
  member count and created time. It shows no contents.
- **Actions.** Join, Transfer ownership, Change visibility, End, and Archive or Unarchive, each with a confirmation
  stating that it is recorded as an admin action.
- **API.** `GET /api/admin/sessions` plus the session actions above.

### SettingsPage additions (US-12.4)
- **Reminder default.** "Quiet-thread reminder default": built-in (one business day), a custom period, or off.
- **Time zone.** "Instance time zone", an IANA zone picker defaulting to UTC.
- **API.** Existing U1 settings endpoints.

## 3. Form validation (client mirrors; the server decides)
| Field | Rule | Message |
|---|---|---|
| Session title | 1 to 120 characters | "Enter a title (up to 120 characters)" |
| Prompt | 1 to 20,000 | "Enter a prompt (up to 20,000 characters)" |
| Thread title | 1 to 200 | "Enter a title (up to 200 characters)" |
| Message | 1 to 20,000, or empty with an attachment | "Messages can be up to 20,000 characters" |
| Outcome summary | 1 to 5,000 | "Write the agreed outcome (up to 5,000 characters)" |
| Revoke reason | 0 to 500 | "Up to 500 characters" |
| Upload | 25 MiB each, 10 per message | "Files can be up to 25 MiB" / "Up to 10 files per message" |
| Reminder period | 1 hour to 30 days | "Choose between 1 hour and 30 days" |

Server field errors are shown beside the field they name, as in U2.

## 4. Interaction flows
1. **Create and invite.** New session, then open or private, then post in the chat, then Add member. The new member
   sees the session at once (US-3.4) through a `member.added` event to their notification channel.
2. **Discuss and approve.** Open a thread, discuss with replies and mentions, then Approve. Write the summary and
   confirm. Everyone sees the status change without refreshing (US-5.6).
3. **Revoke.** Revoke with an optional reason. The thread shows reopened, and the history keeps both records.
4. **Reading without joining.** A non-member opens an open session from "Other sessions" and reads it. An admin
   presses Join, and a member adds a colleague after a mention.
5. **End and archive.** The owner ends the session, which turns it read-only everywhere at once. The owner can then
   archive it, and unarchive it from the Archived filter.

## 5. State and data handling
- **Data fetching.** Server state uses TanStack Query, as in U1 and U2. Real-time events update the caches:
  - append or patch messages by id and sequence, ignoring duplicates;
  - invalidate the overview and thread list on status events;
  - set presence in a small store owned by `SessionLayout`.
- **Sequence tracking.** The last applied sequence is kept per session in memory only. On reconnect, it is sent
  with the subscription.
- **Drafts.** Composer drafts may be kept in `sessionStorage` per channel, as a convenience. Uploads and secrets are
  never kept in browser storage.
- **Reconnecting.** The real-time connection reconnects with backoff and shows a quiet "Reconnecting…" status.
  Writes made while disconnected are sent normally over HTTP, so they never depend on the socket.
