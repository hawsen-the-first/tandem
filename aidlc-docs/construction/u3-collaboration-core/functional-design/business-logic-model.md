# U3 Collaboration Core: Business Logic Model

How the session service, the real-time gateway and the background jobs carry out the rules in `business-rules.md`.
Technology-agnostic: "transaction" means one atomic change to the store, and "outbox" is the U1 mechanism that
publishes events after commit.

## 1. Access decision (BR-23)

Every operation starts with one function. It runs on the server for each request, subscription and event delivery.

```text
access(user, session, need) -> allowed | not_found | must_join | ended
  if session missing                                  -> not_found
  member   = user has a current membership
  is_admin = user is an admin (MFA confirmed, U1 BR-9)
  reader   = member or (session.visibility == open and user is active)
  if not reader and not is_admin                      -> not_found
  if need == read:
      if session.visibility == private and not member -> not_found   (admins join first)
      -> allowed
  if need == manage (BR-22.6: remove, transfer, visibility, reminder, end, archive):
      if not (user is owner or is_admin)              -> not_found if not reader, else must_join
      if need changes content and session.state != active -> ended
      -> allowed
  if need == write (post, reply, edit, delete, upload, thread, approve, revoke, add member, read position):
      if not member                                   -> must_join
      if session.state != active                      -> ended
      -> allowed
```

- `not_found` gives the same response as an unknown id (BR-9.2).
- `must_join` is shown as "Join or ask a member to add you". It is a forbidden response, but only to users who can
  already read the session, so it reveals nothing they could not see.
- Archive and unarchive are the only `manage` actions allowed on ended sessions.
- Admin join is a separate action: `add_member(self)`, allowed for an admin on any session.
- Every id inside the request (thread, message, upload, reference target) is then looked up within this session
  only (BR-23.6).

## 2. Create a session (BR-22.1, BR-22.2)

```text
create_session(user, title, prompt, visibility = open)
  validate title, prompt (BR-26.1 limits), visibility
  transaction:
    session  = new (state active, owner user, next_sequence 1)
    add membership(user, owner, added_by user)
    create chat channel
    create workspace(managed_folder, status ready)
    emit event session.created (to the creator)
    audit session.created
  return session
```

The folder itself is created by the workspace port. If that fails, the transaction is rolled back and the user sees
a clear error, so a session is never half-built.

## 3. Membership (BR-22.3 to BR-22.7)

```text
add_member(actor, session, target_user)          need: write, or admin on any session
  target must be active; skip if already a current member
  upsert membership (clear removed_*), added_by actor
  system message member_added; event member.added; audit session.member_added
     (session.admin_joined, as_admin, when an admin adds themselves)

remove_member(actor, session, target)             need: manage
  target != owner
  set removed_at, removed_by
  system message member_removed; event member.removed; audit session.member_removed
  if session is private: gateway drops target's subscriptions (BR-23.7)

leave(actor, session)                             actor is a member and not the owner -> as remove_member

transfer_ownership(actor, session, new_owner)     need: manage; new_owner is a current member
  swap roles in one transaction; system message owner_changed; audit session.owner_changed

change_visibility(actor, session, value)          need: manage
  set; system message visibility_changed; audit session.visibility_changed
  if open -> private: gateway drops every subscriber who is not a member
```

## 4. Threads and messages (BR-24, BR-25)

```text
open_thread(member, session, title, first_body, attachments)
  transaction:
    thread channel + thread (status open, origin member, last_activity now)
    post_message(... into the new channel) without its own transaction
    emit thread.created; audit thread.created

post_message(member, channel, body, parent_id?, upload_ids[], references[], mentions[])
  validate body length (BR-25.1) and attachment count (BR-26.1)
  parent: must be top-level in this channel; if parent has a parent, use that instead (BR-25.3)
  uploads: each pending, same session, uploaded by this member; session quota not exceeded
  references: each target in the same session (BR-25.6)
  mentions: resolve names -> users (BR-25.7); keep only those allowed by visibility
  transaction:
    seq = session.next_sequence++
    insert message(seq); mark uploads attached; insert references and mentions
    update channel.last_message_sequence; thread.last_activity_at and clear reminded_at
    parent.reply_count++ and last_reply_at
    advance author's read position to seq
    for each mentioned user, except the author, once: create notification(mention) if they can read (BR-33.2)
    emit message.created(seq)
  return message, plus non_member_mentions for the "Add them as a member?" prompt (BR-25.8)

edit_message(author, message, body)  / delete_message(author, message)
  author only; session writable
  refuse if the thread has an approval in force given after message.created_at (BR-25.4)
  edit: replace body, set edited_at, recompute references and mentions (new mentions notify; removed ones do not un-notify)
  delete: clear body, attachments, references, mentions; set deleted_at; discard orphaned uploads
  emit message.edited or message.deleted
```

Message ordering uses the per-session sequence taken inside the transaction. Two posts in the same session are
therefore strictly ordered, and the order clients see equals commit order.

## 5. Uploads (BR-26, BR-28.4)

```text
upload(member, session, file stream, declared name and type)
  need: write
  refuse early if declared size > 25 MiB, or the quota would be exceeded
  stream to temporary storage while hashing (SHA-256) and counting bytes; abort at 25 MiB + 1
  sniff type from the first bytes
  transaction: insert upload(pending, sha256, size, sniffed type) ; audit upload.stored
  move bytes into permanent storage; on any failure delete temporary bytes, insert nothing
  return upload id

download(reader, upload)
  need: read; upload in this session and attached (or pending and requested by its uploader)
  serve with attachment disposition and nosniff, except inline for matching PNG, JPEG, GIF and WebP

discard_pending (job, hourly): pending older than 24 h -> delete bytes and row
```

## 6. Approvals (BR-27)

```text
approve(member, thread, summary)
  need: write; thread.status in (open, reopened); summary 1..5,000 characters
  transaction:
    approval = insert(approved_by, now, summary, source person)
    thread.status = approved; current_approval = approval; last_activity_at = now
    system message thread_approved; emit thread.status_changed
    audit thread.approved (summary present, length)
  a second concurrent approve sees status approved and is refused "Already approved by {name}"

revoke(member, thread, reason?)
  need: write; thread.status == approved
  dependents = dependency_check(thread)            hook: returns [] in U3; U6 returns plan or task links
  if dependents not empty -> refuse with the reason naming them
  transaction:
    mark current approval revoked (by, at, reason); thread.status = reopened; clear current approval
    thread.last_activity_at = now; reminded_at = null
    system message approval_revoked; emit thread.status_changed; audit thread.approval_revoked

agreed_context(thread)                             used by U4 and U6 (BR-27.8)
  if no approval in force -> the thread is not agreed context
  -> [ "Agreed decision (overrides anything below that contradicts it):", approval.summary,
       "Discussion:", messages in order ]
```

The approve and revoke transitions lock the thread row, so concurrent actions resolve to exactly one outcome.

## 7. Real-time gateway (BR-29, BR-32)

```text
subscribe(connection, session, after_sequence?)
  access(user, session, read) must be allowed
  if after_sequence given:
     if within the replay window -> send stored events after it, then live
     else -> send "reload"
  register the connection for the session; if member, mark presence

on outbox event(e) for session s:
  for each connection subscribed to s:
     re-check access(user, s, read)          cheap: membership cache invalidated on member and visibility events
     if denied -> close that subscription
     else if e is read.updated and e.user != connection.user -> skip
     else send e

presence:
  per (session, user): set of connections with last input time
  online if any connection had input in 5 min; away if connections but none recent; absent if none
  send presence.changed to members on transitions only
```

Events carry ids and metadata. Clients fetch bodies through the normal reads (BR-29.3). The NFR design decides
whether to include bodies in events for authorised receivers, to meet the 500 ms target.

## 8. Lifecycle (BR-30)

```text
end_session(actor, session)                     need: manage; state active
  uncommitted = workspace_change_check(session)  hook: always "none" in U3; U5 fills it for repositories
  (U4 adds: closing summary draft, edited by the owner, before confirm)
  transaction:
    state = ended; ended_at, ended_by
    workspace.status = removing; enqueue job remove_workspace
    system message session_ended; emit session.ended; audit session.ended
remove_workspace job: delete folder; status removed; on error status removal_failed, retry with backoff, alert after N
archive / unarchive (actor; need manage; ended <-> archived): set state and time; emit; audit
```

## 9. Quiet-thread reminders (BR-31)

```text
reminder job, every 5 minutes:
  for each active session s:
    period = resolve(s.reminder_period, instance.thread_reminder_period, ONE_BUSINESS_DAY)
    if period is off -> continue
    for each thread t in s with status open or reopened and reminded_at is null:
      due = add(period, t.last_activity_at, instance.time_zone)
      if now >= due:
        recipients = current members
                     - author of the last message
                     - approvers of the last revoked approval      (they agreed before; BR-31.1)
        create notification(thread_quiet) for each; set t.reminded_at = now

add(ONE_BUSINESS_DAY, start, zone): count 24 hours forward, skipping Saturday and Sunday in that zone
add(duration, start, zone): start + duration
```

Activity (a message or a status change) clears `reminded_at`, so the next quiet period can remind again.

## 10. Notifications and unread (BR-32, BR-33)

```text
mark_read(member, channel, sequence)   position = max(position, sequence); emit read.updated to this user only
mark_all_read(member, session)         every channel's position = its last_message_sequence
notifications(user)                    newest first; title built at display; unavailable if access lost
mark_notification_read(user, ids | all)
prune job, daily: delete notifications older than 90 days
```

## 11. Hooks left for later units

| Hook | U3 behaviour | Filled by |
|---|---|---|
| `dependency_check(thread)` | Returns none | U6: plan steps and tasks that rely on the thread |
| `workspace_change_check(session)` | Returns none | U5: uncommitted changes in a repository |
| `agreed_context(thread)` | Implemented and tested | U4 prompt builder must use it (BR-27.8) |
| Thread `origin = orchestrator`, `round_id` | Never set | U4 question rounds |
| Author kind `agent`; agent `@` names | Never written | U4 |
| Closing summary, follow-up session | Fields reserved | U4 (US-11.7, US-11.8) |
| Notification kinds `plan_ready`, `task_output_ready` | Not produced | U6 |
| Reference kinds `artifact`, `workspace_file` | Refused | U5, U6 |
