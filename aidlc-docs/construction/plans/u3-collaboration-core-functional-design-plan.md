# Functional Design Plan: U3 Collaboration core

**Stage**: CONSTRUCTION - Functional Design, **Standard** (unit `u3-collaboration-core`)
**Inputs**: `unit-of-work.md` (U3), `unit-of-work-story-map.md`, `stories.md`, `requirements.md` (FR-2, FR-4, FR-6, FR-11.1, FR-14, NFR-3, NFR-4), U1 and U2 artifacts
**Stories in scope (23, after decisions)**:
- **Sessions**: US-3.1 create, US-3.3 managed folder, US-3.4 members, US-3.5 members-only access, US-3.7 overview.
- **Messaging**: US-5.1 thread messages, US-5.2 session chat, US-5.3 replies, US-5.4 references, US-5.5 uploads, US-5.6 thread status.
- **Approvals**: US-7.1 approve, US-7.2 who and when, US-7.3 revoke.
- **Lifecycle**: US-11.4 end, US-11.6 read-only ended and archived.
- **Notifications and presence**: US-12.1 mentions, US-12.2 needs-me, US-12.3 presence and unread, US-12.4 reminder settings.
- **Administration**: US-3.8 ownership transfer and admin session management.
- **Safety**: US-13.1 limits, US-13.2 safe display.

**Components**: C3 Realtime Gateway, C5 Session Service, C6, C7 (security-critical: full NFR stages), C11, C23; C19 deepened from the U1 stub.
**Scope**: business logic, domain model and rules, technology-agnostic. Libraries, schema and wire formats belong to Code Generation.
**Boundary with U4**: there are no AI agents yet. The orchestrator, `@agent` calls, outcome summaries written by a model, the plan and tasks arrive in U4 and U6. U3 must leave hooks for them, not build them.

## Steps
- [x] 1. Analyse the unit context (stories, requirements, components, the U1 skeleton tables `chat_sessions`, `chat_messages`, `outbox_events`, `jobs`)
- [x] 2. Answer the questions below (fill each `[Answer]:` tag)
- [x] 3. Review the answers for ambiguity; ask follow-ups if needed
- [x] 4. Record decisions and any changes they cause to earlier artifacts
- [x] 5. Domain entities (`domain-entities.md`)
- [x] 6. Business rules (`business-rules.md`)
- [x] 7. Business logic model (`business-logic-model.md`)
- [x] 8. Frontend components (`frontend-components.md`)
- [x] 9. Approval of the functional design (approved; pull request)

## Decisions
From the answers below (Q) and `u3-collaboration-core-functional-design-clarification-questions.md` (C).

| # | Decision |
|---|---|
| Q1, C1 | Sessions are **open** by default (every active user reads, only members write). The owner can make one **private** at creation (members only, others get "not found"). Request to join and fuller RBAC are post-MVP |
| C2 | Non-member readers read quietly: no notifications, unread or presence. Lists split into "My sessions" and "Other sessions" |
| Q2 | Any member adds members; only the owner (or an admin) removes them |
| Q3, C3 | Ownership transfers; the owner cannot leave without transferring. Admins can fully manage any session (join, add, remove, transfer, visibility, end, archive), always audited |
| Q4 | Any member opens threads (title and first message); orchestrator threads arrive in U4 |
| Q5 | An outcome summary is **required** to approve. It is the agreed decision and overrides contradicting thread content when used as model context (FR-6.4, BR-27.8, binding on U4) |
| Q6 | Any member revokes, recorded and audited |
| Q7 | Authors edit (marked edited) and delete (placeholder kept); blocked in ended sessions and under an approval given after the message |
| Q8 | One level of replies |
| Q9 | 20,000 characters per message; 25 MiB per file, 10 per message, 2 GiB per session; any type, served safely. Snippets are post-MVP |
| Q10 | Uploads are attachments, not workspace files |
| C4 | Mentions: members first, then other active users in open sessions; a mentioned non-member is notified and the author is offered "Add them as a member?". Private sessions: members only |
| Q12, C5 | Quiet-thread reminders, three levels: built-in one business day, instance default (admin), session value (owner or admin); "off" allowed at the instance and session levels |
| Q13 | Presence per session, online or away (5 minutes idle). "Actively viewing" is post-MVP |
| Q14, C6 | U3 ends a session with a simple confirmation; archive and unarchive; never reopened. The closing summary and follow-up session are new stories in U4 |
| Q15 | CommonMark, tables, strikethrough, display-only task lists, highlighted code; images only from this session's uploads; no raw HTML |

**Assumptions added during design (confirm at approval)**:
- **A14**: "One business day" means 24 hours counted on Monday to Friday in a new instance time-zone setting (default UTC), ignoring public holidays (BR-31.3).
- **A15**: Admins see private sessions' metadata (title, owner, state, member count) on an admin sessions page in order to manage them, but not their contents until they join (BR-23.3).
- **A16**: Events carry ids and metadata, not message bodies; whether bodies ride along for authorised receivers, to meet 500 ms, is an NFR Design choice (BR-29.3).

## Changes to earlier artifacts caused by these decisions
- **`requirements.md`**:
  - FR-2.2, FR-2.3 and FR-2.5 rewritten.
  - New requirements: **FR-2.8** (closing summary and follow-up, Should), **FR-6.4** (summary required and takes precedence), **FR-14.2** (quiet-thread reminders) and **FR-15.4** (admin session management).
  - Out of Scope gains request to join and RBAC visibility, snippets and "actively viewing".
- **`stories.md`**:
  - Amended: US-3.4, US-3.5 (renamed "Session visibility and workspace isolation"), US-7.1, US-11.4, US-11.6, US-12.1 and US-12.2.
  - New stories: **US-3.8** transfer ownership and admin session management (Should, U3), **US-11.7** closing summary (Should, U4), **US-11.8** follow-up session (Should, U4) and **US-12.4** quiet-thread reminders (Should, U3).
  - Coverage table, persona matrix and priority summary updated: 95 stories (Must 53, Should 38, Could 4).
- **`unit-of-work-story-map.md`**: U3 has 23 stories (12 Must, 11 Should) and U4 has 17. `unit-of-work-dependency.md`: 95 of 95.

## Working assumptions (not questions; confirm or correct at approval)
- **A1**: Only active instance users can be members. A deactivated user keeps their membership record, but cannot sign in, so they see nothing. Their messages and approvals stay attributed to them.
- **A2**: Every session read and write, and every real-time subscription and event, is checked against membership on the server. A non-member gets "not found", as in BR-9.2, never "forbidden".
- **A3**: Thread status moves `open` to `approved` (approve), and `approved` to `reopened` (revoke). A reopened thread can be approved again. The data model keeps a list of approvals per thread, so that rules needing N approvals (FR-6.1, post-MVP) fit without reshaping.
- **A4**: Only humans approve. An approval and a revoke each record the person, the time and the outcome summary text at that moment. Both are written to the audit log.
- **A5**: Revoke is blocked once something depends on the thread. U3 has no plan or tasks yet, so the "depends on" check is a hook that U6 fills in. In U3, revoke is always allowed on an approved thread in an active session.
- **A6**: Session states are `active`, `ended` and `archived`. Ending removes the managed folder's files. An ended or archived session is entirely read-only, including messages, uploads, approvals and membership changes. The uncommitted-changes check is a hook that U5 fills in for repositories, so U3 always shows the simple confirmation.
- **A7**: Messages are Markdown, rendered on the client from a sanitised subset: no raw HTML, no scripts, no event handlers and no `javascript:` or `data:` links. External links open with `rel="noopener noreferrer"`. The CSP from U1 stays in force.
- **A8**: Each user has a last-read position per thread and for the session chat. The unread indicator compares it with the newest message.
- **A9**: Real-time delivery is at least once and ordered within a session. A client that reconnects asks for what it missed since its last event, and falls back to a full reload if the gap is too old. Messages carry a server timestamp and a per-session sequence number.
- **A10**: Each message has one author kind: `person`, `agent` or `system`. Only `person` exists in U3; the other kinds are kept so that U4 needs no reshaping. System messages (for example "Ana approved this thread") are generated by the server.
- **A11**: Audit events added: `session.created`, `session.member_added`, `session.member_removed`, `session.ended`, `session.archived`, `thread.created`, `thread.approved`, `thread.approval_revoked`, `upload.stored`. Message posting is not audited (it is content, not a security event).
- **A12**: Uploads are stored with a SHA-256 content hash, a size, a declared and a sniffed media type, and the uploader. They are served only to members, with `Content-Disposition: attachment` unless they are a safe image type, and never run as HTML.
- **A13**: References (US-5.4) resolve on display. A reference to a thread, upload or artifact that is gone, or that the viewer can no longer see, shows "unavailable". Artifacts and workspace files arrive in later units, so U3 references threads, messages and uploads, and the picker gains the other kinds later.

---

## Questions

Fill in the letter after each `[Answer]:` tag. Add notes after the letter if you like.

## Question 1
Can instance admins see sessions they are not members of?

A) No. Admins are treated like everyone else. Only members see a session, which keeps confidential work confidential (FR-2.3). Admins see that sessions exist only through counts in the audit log or the admin pages.

B) Admins can list every session (title, owner, state, member count) but cannot open its contents unless they are added.

C) Admins can open every session read-only, and each such view is audited.

X) Other (please describe after [Answer]: tag below)

[Answer]: X similar to how slack treats conversations. Read only for all members, admins can add themselves, regular members can request to join (that can be added to a later slice). Eventually we also may want to add the ability to set visibility per session, so that some are public and some are private with RBAC.

## Question 2
Who can add and remove session members?

A) Only the session owner (FR-2.2).

B) The owner, plus any members the owner promotes to co-owner.

C) Any member can add people; only the owner can remove them.

X) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 3
What happens to ownership when the owner leaves, or is deactivated?

A) The owner can transfer ownership to another member. The owner cannot leave without transferring it. If an owner is deactivated, an admin can assign a new owner from the existing members (audited), without seeing the contents.

B) Ownership can be transferred, but nothing happens on deactivation. The session simply has no active owner until the user is reactivated.

C) No transfer in the MVP.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 4
How do people create threads in U3, before the orchestrator exists (U4)?

A) Any member can open a thread with a title and a first message. The orchestrator will later open question threads the same way, marked as orchestrator-proposed.

B) Only the owner can open threads by hand.

C) No manual threads. U3 tests use seeded threads, and real threads come only from the orchestrator in U4.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
Without a model to write it, what is the outcome summary when someone approves a thread (US-7.1, FR-4.8)?

A) The approver writes or edits it in the approve dialog. It is optional in U3. From U4, the orchestrator pre-fills a draft that the approver can edit.

B) It is required: the approver must write at least one sentence.

C) No summary in U3. Approving records only who and when.

X) Other (please describe after [Answer]: tag below)

[Answer]: B, and we should ensure that the resulting prompt in the build phase knows to default to the summary over any contradicting info in the attached thread.

## Question 6
Who can revoke an approval (US-7.3)?

A) Any session member, as with approving. It is recorded and audited.

B) Only the person who approved, or the owner.

C) Only the owner.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
Can people edit or delete their own messages?

A) Authors can edit their messages, which then show "edited" with the time; the previous text is not shown to others. Authors can delete them, leaving a "message deleted" placeholder so replies keep their parent. Neither is possible in an ended session, or on a message that an approval summary quotes.

B) Edit only, with no delete.

C) Neither. Messages are permanent, as a decision record.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 8
How deep do replies go (US-5.3)?

A) One level, as in Slack. A reply belongs to a top-level message, and replying to a reply adds to the same group.

B) Unlimited nesting, as on a forum.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 9
What limits apply to messages and uploads (US-13.1)? Admins could change these later in instance settings.

A) A message holds up to 20,000 characters. An upload is up to 25 MiB, with up to 10 files per message and 2 GiB of uploads per session. Any file type is accepted and served safely as a download (A12).

B) Same sizes as A, but only an allow-list of types is accepted: text, Markdown, PDF, images, office documents, CSV, JSON and common source code.

C) Smaller limits: 10,000 characters, 10 MiB per file, 500 MiB per session, any type.

X) Other (please describe after [Answer]: tag below)

[Answer]: A sounds fine. We should have a facility for longer context to be pasted in at some point, but this isn't required for mvp (like snippets in Slack)

## Question 10
Should uploads become part of the managed-folder workspace?

A) No. Uploads are session attachments, readable by members and by agents in that thread or chat. Copying one into the workspace is an explicit later action (U5 or U6).

B) Yes. Every upload is also written into an `uploads/` folder in the workspace.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 11
Who can be @mentioned, and what happens when someone mentions a non-member?

A) The picker offers session members only. Typing the name of a non-member does nothing special and sends no notification; nobody outside the session learns of it.

B) The picker offers members only. If the author is the owner, it offers to add the non-member as a member.

X) Other (please describe after [Answer]: tag below)

[Answer]:

## Question 12
Who is notified that a thread is awaiting approval (US-12.2), and when?

A) Every member except the author, once, when a thread is opened or reopened. Nobody receives a reminder or a second notice.

B) Only the owner, when a thread is opened or reopened.

C) Every member, once a thread has had no activity for a set time (for example 24 hours).

X) Other (please describe after [Answer]: tag below)

[Answer]: C, configurable by the session/platform admin

## Question 13
What should presence show (US-12.3)?

A) Per session: a member shows as online while they have the session open in a tab. When they have no tab open, or have been idle for 5 minutes, they show as away. Nothing is shown outside the session.

B) Instance-wide: online whenever they are signed in anywhere in Tandem.

C) Online or offline only, with no away state.

X) Other (please describe after [Answer]: tag below)

[Answer]: A, maybe with an 'Actively viewing this thread/session' indicator like Zendesk has but again, not required for MVP.

## Question 14
After a session is ended, what happens next?

A) The owner can archive an ended session, and unarchive it back to ended. Nobody can reopen an ended session. Deleting a session is post-MVP.

B) Same as A, but the owner can also reopen an ended session to active (managed folders only, because the files are gone).

C) Archiving is one way, with no unarchive.

X) Other (please describe after [Answer]: tag below)

[Answer]: When a session ends, a summary should be generated. This can then be the basis of a 'create a follow up session' flow, so new information and context is added with some memory of the previous convo but not a rehash of the same info.

## Question 15
Which Markdown features do messages support (A7)?

A) CommonMark plus tables, strikethrough, task lists (display only) and fenced code with syntax highlighting. Images display only when they are uploads from the same session; external images show as links, which also keeps the strict CSP. There is no raw HTML and no maths.

B) Same as A, but external images are allowed too, fetched through a server-side image proxy.

C) A minimal set: paragraphs, emphasis, links, lists and code, with no tables or images.

X) Other (please describe after [Answer]: tag below)

[Answer]: A
