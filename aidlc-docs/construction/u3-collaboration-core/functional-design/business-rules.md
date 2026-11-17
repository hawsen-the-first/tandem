# U3 Collaboration Core: Business Rules

Rules continue the numbering of U1 (BR-1 to BR-13) and U2 (BR-14 to BR-21). Each rule names its stories and the
decisions it comes from (Q for the plan, C for the clarifications). Each rule must map to at least one test (MT-11).

**Terms**:
- **Reader**: anyone allowed to see a session. That means every active user for an open session, and members and
  admins only for a private one.
- **Member**: a current, not removed, membership.
- **Writable**: the session is `active`.

## BR-22 Sessions and membership (US-3.1, US-3.4, US-3.8; Q2, Q3, C3)
1. Any active user can create a session. A title, a prompt and a workspace kind are required. The creator becomes
   the owner and first member.
2. Creating a session also creates its chat channel and its managed-folder workspace. If any part fails, nothing is
   kept.
3. **Adding members.** Any member, the owner or an admin can add active instance users as members. A user already a
   member is not added twice. A removed user can be added again.
4. **Ownership.** There is always exactly one owner, and the owner is always a member.
   - The owner can transfer ownership to another current member, and then becomes an ordinary member.
   - The owner cannot leave or be removed while still the owner.
5. **Removing and leaving.** Only the owner or an admin can remove a member. Any member except the owner can leave.
   Removal keeps the person's past messages, approvals and uploads, attributed to them (US-3.4).
6. **Admins.** An admin can add themselves to any session, open or private, and can do anything the owner can:
   - add and remove members;
   - transfer ownership, including from a deactivated owner;
   - change visibility and the reminder period;
   - end, archive and unarchive.

   Every such action is audited, marked as taken by an admin, and posted as a system message in the session.
   An admin who is not a member reads an open session like anyone else (BR-23.2). To read a private one, they must
   join first.
7. Membership changes are refused when the session is not writable. The membership of an ended session is frozen.

## BR-23 Visibility and access (US-3.5; C1 B, C2 A; SECURITY-08)
1. **Visibility.** Visibility is `open` by default. The owner can choose `private` at creation. Afterwards, only the
   owner or an admin can change it, and the change is audited and posted as a system message.
2. **Open sessions.** In an open session, every active user is a reader. Readers see everything members see: the
   overview, threads, chat, messages, uploads, approvals and, later, artifacts. Only members can write (BR-23.5).
3. **Private sessions.** In a private session, only members are readers. Everyone else, including admins who have
   not joined, gets "not found" on every read, write, link and subscription, with nothing confirming that the session
   exists (BR-9.2). Admins can see private sessions' titles, owners, states and member counts on the admin sessions
   page only, so that they can manage them (BR-22.6). They cannot see the contents.
4. **Session lists.** The list shows "My sessions" (where I am a member) and "Other sessions" (open sessions where
   I am not). Private sessions never appear in another user's "Other sessions".
5. **Writing.** Posting, replying, editing, deleting, uploading, opening threads, approving, revoking, adding members
   and updating read positions all need membership. A reader who is not a member is told "Join or ask a member to
   add you". "Request to join" is post-MVP.
6. **Isolation.** Every id in a request must belong to the session in the path. A message, upload, thread or
   reference from another session is "not found" (US-3.5, FR-11.7).
7. **Checks on every action.** Access is checked on the server for every request, every real-time subscription and
   every event before delivery. Losing access ends delivery at once: when a member is removed from a private
   session, or an open session becomes private, the server drops their subscription.
8. **Non-member readers.** Non-member readers of an open session get no notifications, no unread tracking and no
   presence (C2 A). Their reading is not audited.

## BR-24 Threads (US-5.1, US-5.6; Q4)
1. Any member can open a thread in a writable session. It needs a title (1 to 200 characters) and a first message.
   The thread starts `open` with `origin = member`.
2. Opening a thread posts nothing to the chat. It appears in the thread list and the overview through a real-time
   event.
3. A thread's title can be edited by the member who opened it or by the owner, while the thread is not `approved`.

## BR-25 Messages, replies, references and mentions (US-5.1 to US-5.4, US-12.1; Q7, Q8, C4)
1. Members post into the chat or a thread of a writable session.
   - The body is Markdown of 1 to 20,000 characters after trimming (Q9 A).
   - An empty body is allowed only with at least one attachment.
   - Messages over the limit are refused, the error states the limit, and nothing is stored (US-13.1).
2. Messages are shown in sequence order. The time shown is server time.
3. **Replies.** Replies are one level deep. A reply's parent is a top-level message in the same channel. Replying
   to a reply attaches the new message to the same top-level parent. The parent shows its reply count and the time
   of its last reply.
4. **Editing.** Only the author edits, and only in a writable session. An edited message shows "edited" and the
   time; earlier text is not shown. A message cannot be edited if an approval in force on its thread was given
   after it was posted. Agreed context does not change underneath an approval (Q7 A).
5. **Deleting.** Only the author deletes, under the same conditions as editing.
   - Deleting removes the body, attachments, references and mentions.
   - A "message deleted" placeholder keeps its place and its replies.
   - Its uploads are discarded unless attached elsewhere.
6. **References.** A reference must point at a thread, message or upload in the same session, checked on save
   (US-5.4). When shown, a reference whose target is gone, deleted, or no longer visible to the viewer renders as
   "unavailable" (A13).
7. **Mentions.** `@name` mentions resolve to people (C4 C).
   - In an open session, the picker offers members first, then other active users.
   - In a private session, it offers members only. Naming a non-member there has no effect and sends nothing.
8. **Mentioning a non-member.** When a member mentions a non-member in an open session, the message is posted and
   the person is notified. The author is offered "Add them as a member?", which adds them (BR-22.3) if accepted.
   Nothing is added without that confirmation.
9. A person is never notified twice for one message. Mentioning yourself sends nothing.
10. **Agents.** In U3, `@` names that match no person stay plain text. Agent names arrive in U4 and take precedence
    over people with the same name.

## BR-26 Uploads and limits (US-5.5, US-13.1; Q9, Q10)
1. **Size limits.**
   - Each upload is at most 25 MiB.
   - A message carries at most 10 attachments.
   - A session holds at most 2 GiB of attached uploads.
   - Each refusal states the limit concerned.
2. **File types.** Any file type is accepted (Q9 A). Safety comes from how files are served (BR-28.4), not from
   what is accepted.
3. **No partial uploads.** A failed or refused upload leaves nothing stored (US-13.1). The content hash is
   calculated as the file is received.
4. **Pending uploads.** An upload is `pending` until a message using it is posted. Pending uploads older than 24
   hours are discarded, and their bytes count toward no quota.
5. **Not workspace files.** Uploads are attachments, not workspace files (Q10 A). Members and readers can download
   them, and agents in that channel will be able to read them (U4). Copying one into the workspace is a later,
   explicit action.
6. **Instance settings.** The limits are held in code with these values. Making them instance settings is a later
   option, so they are kept in one place.
7. **Retention (added in NFR Requirements, Q4).** Uploads are deleted a set time after the session ends: 180 days
   by default, an instance setting from 1 day to "keep forever". The upload becomes `expired`, and its messages show
   "File removed after the retention period". Messages, approvals and summaries are kept.

## BR-27 Thread status and approvals (US-5.6, US-7.1 to US-7.3; Q5, Q6, A3 to A5)
1. **Status changes.**
   - `open` or `reopened` becomes `approved` when someone approves.
   - `approved` becomes `reopened` when someone revokes.
   - No other change is possible.
2. **Approving.** Any member can approve a thread in a writable session. Only people approve: an agent or the
   orchestrator never does (FR-3.5). One approval closes the thread (FR-6.1). Approving a thread that is already
   approved is refused with "Already approved by {name}".
3. **Outcome summary.** An outcome summary is required, 1 to 5,000 characters of Markdown, and it is recorded with
   the approval (Q5 B). The approve dialog shows exactly what will be recorded.
4. **Revoking.** Any member can revoke an approval in force (Q6 A), with an optional reason. The thread becomes
   `reopened`. The approval stays in the history, marked revoked, with who and when.
5. **Dependents.** Revoking is refused, with the reason shown, once something depends on the thread. U3 has nothing
   that can depend on a thread, so this is a hook that U6 fills (plan and task dependencies). In U3, revoking is
   always allowed in a writable session.
6. **Records.** Every approval and revoke posts a system message in the thread ("Ana approved this thread", "Sam
   revoked the approval: {reason}"). It is audited with the actor and time, without the summary text (US-7.2).
7. **New messages.** A thread stays open for messages while it is `approved`. Posting does not change its status.
8. **The summary is the agreed decision (Q5 note, FR-6.4).** When an approved thread is given to any model as
   context (U4 and U6), the context must:
   - present the approval summary first, labelled as the agreed decision;
   - state that it overrides anything in the thread's messages that contradicts it.

   Only the approval in force counts. This rule binds the U4 prompt builder, and is tested there.

## BR-28 Safe display of untrusted content (US-13.2, US-5.1; Q15, A7, A12; SECURITY-04, SECURITY-05)
1. **Supported Markdown.** CommonMark with tables, strikethrough, task lists (display only, not clickable) and
   fenced code with syntax highlighting. There is no raw HTML, no maths and no embedded media other than BR-28.3.
2. **Sanitising.** Rendering produces elements from a fixed allow-list only.
   - Raw HTML in the source shows as text.
   - Links allow only `http`, `https`, `mailto` and in-app relative links. Others render as plain text.
   - External links open in a new tab with `rel="noopener noreferrer"` and show their address on hover and focus.
3. **Images.** An image is displayed only when its source is an upload in the same session. Any other image
   renders as a link with its alt text, so no external fetches happen and the CSP stays strict.
4. **Serving uploads.**
   - Uploads are served from the API, never from a path the browser could treat as part of the application.
   - Uploads are always served with `Content-Disposition: attachment` and `X-Content-Type-Options: nosniff`. The only
     exceptions are PNG, JPEG, GIF and WebP whose sniffed type matches, which are served inline as images.
   - SVG, HTML and anything else that could run code is always a download.
5. **Hostile input.** The same rendering applies to every author kind, including future agents, and to prompts,
   summaries, titles and file names. Titles and file names render as text only. Hostile input tests are part of the
   exit criteria (unit-of-work U3).

## BR-29 Real-time delivery and ordering (US-5.1, US-5.6; A9; NFR-3.1)
1. **Event stream.** Every change a reader can see emits a session event with the next per-session sequence number,
   in the same transaction as the change. Delivery follows the outbox pattern from U1, so no change is lost and none
   is announced before it is committed.
2. **Ordering and catching up.** Delivery is at least once and in order within a session.
   - A client tracks the last sequence it applied and ignores duplicates.
   - On reconnecting, the client asks for events after that sequence.
   - If the gap is older than the replay window (24 hours, or 10,000 events), it is told to reload in full.
3. **Content in events.** Events carry ids, kinds and safe metadata (author id, thread status, counts), never
   message bodies or file names. The client then reads content through the normal endpoints, so access is checked
   once, in one place. An exception for latency, carrying the body to readers who are already authorised, is an NFR
   design choice. It must keep BR-23.7.
4. **Latency target.** A message posted by one member appears for other readers within about 500 ms on a LAN
   (NFR-3.1, the unit exit criterion).

## BR-30 Session lifecycle (US-11.4, US-11.6; Q14, C6 B, A6)
1. **States.** `active` becomes `ended` (end). `ended` becomes `archived` (archive), and `archived` returns to
   `ended` (unarchive). Nothing returns to `active`: an ended session is never reopened.
2. **Ending.** Only the owner or an admin ends a session. U3 asks for a simple confirmation.
   - The uncommitted-changes warning is a hook that U5 fills for repositories.
   - The closing summary and the follow-up session are U4 (C6 B, US-11.7, US-11.8).
3. **Read-only after ending.** An ended or archived session is entirely read-only. Every write in BR-22 to BR-27 is
   refused with "This session has ended", except archive and unarchive. Readers can still read and download.
4. **Workspace removal.** Ending removes the managed-folder workspace. Removal runs as a background job; a failure
   is recorded and retried, never ignored. The session counts as ended at once.
5. **Archiving.** Archiving removes the session from the default lists. It stays reachable by link and through an
   "Archived" filter.
6. **Notifications.** Ending stops quiet-thread reminders for the session. Unread notifications that point into it
   remain, and still open it read-only.

## BR-31 Quiet-thread reminders (US-12.2, US-12.4; Q12 C, C5)
1. **When a reminder is sent.** A thread that is `open` or `reopened` in a writable session, with no activity for
   the reminder period, sends one `thread_quiet` notification to each current member. These people are left out:
   - the author of the last message;
   - anyone who approved it while it was last approved.

   Activity means a new message or a status change.
2. **The period resolves in three levels (C5).**
   1. The session's own value, if set. The owner or an admin sets it.
   2. Otherwise, the instance value, if set. An admin sets it.
   3. Otherwise, the built-in default: **one business day**.

   `off` at the session or instance level turns reminders off at that level. A session value of `off` wins over an
   instance period, and a session period wins over an instance `off`.
3. **Business day.** Assumption, to be confirmed at approval. "One business day" means 24 hours counted on working
   days only, Monday to Friday, in the instance time zone. That is a new instance setting, which defaults to UTC.
   Public holidays are not considered.
   - A thread that goes quiet on Friday at 15:00 is reminded on Monday at 15:00.
   - One that goes quiet on Saturday is reminded on Tuesday at 00:00 (one full working day after the weekend ends).

   An explicit period set by an admin or owner is a plain duration from 1 hour to 30 days, counted in calendar
   time.
4. **One reminder per quiet period.** New activity starts a new period. Reopening starts the clock again. Changing
   the period takes effect from the next check.
5. **Timing.** Reminders are checked by a background job at least every 5 minutes. Exact timing is not promised to
   the minute.

## BR-32 Unread and presence (US-12.3; Q13 A, C2 A)
1. Each member has a read position per channel: the chat and each thread. A channel is unread when its newest
   message sequence is greater than the position. The member's own messages advance their position.
2. Read positions only move forward. Opening a channel and scrolling to the newest message marks it read. "Mark all
   read" exists for a session.
3. **Presence.** Presence is per session. A member is `online` while at least one of their tabs has the session open
   and has had input in the last 5 minutes, and `away` otherwise while a tab is open. With no tab open, they are not
   shown. Presence is never shown outside the session, and non-member readers do not appear (C2 A).
4. Presence changes are delivered as events. They are not stored or audited.

## BR-33 Notifications (US-12.1, US-12.2)
1. **Kinds in U3.** `mention` and `thread_quiet`. `plan_ready` and `task_output_ready` are added in U6, under the
   same rules.
2. A notification is created for the recipient only when they could open its target at that moment.
3. **Reading.** Notifications can be marked read one by one or all together. Opening the target marks the related
   notification read.
4. **Delivery.** Notifications are in-app only (no email in the MVP). They are delivered in real time to the
   recipient's open tabs, and listed with the newest first.
5. **Content.** A notification stores no message text. When it is shown, its title is built from current data,
   such as "Ana mentioned you in {thread title}". If the viewer can no longer see the target, it shows "unavailable".
6. Notifications are kept for 90 days, then removed.

## BR-34 Audit (all stories; A11, C3 B)
1. The events in domain-entities §7 are audited with the actor, the time, the session id and the relevant ids.
2. Admin actions on a session they do not own are marked `as_admin`.
3. Message bodies, summaries, file contents and file names are never written to the audit log. Approval entries
   record that a summary exists, and its length.
4. Reading is never audited (C2 A), including an admin reading an open session.
