# U3 Functional Design: clarification questions

Follow-ups to the answers in `u3-collaboration-core-functional-design-plan.md`. Six questions. Fill in the letter
after each `[Answer]:` tag, and add notes if you like.

## What is already settled
These answers are clear and will be used as given:
- **Membership:** any member can add people and only the owner removes them (Q2). Ownership transfers, and an admin
  can reassign the owner of a deactivated owner's session (Q3).
- **Threads and approvals:** any member opens threads (Q4). An outcome summary is required to approve (Q5). Any
  member can revoke (Q6).
- **Messages:** edit and delete with placeholders (Q7), one level of replies (Q8), limits as in Q9 A, and uploads
  are attachments, not workspace files (Q10).
- **Presence and Markdown:** presence per session (Q13 A) and the Markdown subset (Q15 A).

## Notes recorded for later (not MVP)
- **Q5:** the summary takes precedence. When an approved thread is passed to a model as context (U4 and U6), the
  prompt must say that its outcome summary is the agreed decision, and that the summary overrides anything in the
  thread's messages that contradicts it. This becomes a business rule now and a requirement on the U4 prompt
  builder.
- **Q9:** long pasted text kept as a "snippet" attachment, as in Slack. Post-MVP backlog.
- **Q13:** an "actively viewing this thread" indicator, as in Zendesk. Post-MVP backlog.
- **Q1:** "request to join" and per-session visibility with RBAC. Post-MVP backlog, unless C1 says otherwise.

---

## Question C1 (follows Q1)
Your answer makes sessions readable by everyone on the instance, as public channels in Slack are. This reverses
FR-2.3 ("only session members can see a session") and US-3.5 ("a non-member cannot view, list or fetch anything").
Which version should the MVP build?

A) **Open by default, no private option yet.** Every active user can find and read every session: the overview,
threads, messages, chat, uploads and, later, artifacts and workspace files. Only members can post, reply, upload,
approve, revoke or call agents. Private sessions with RBAC are post-MVP. FR-2.3 and US-3.5 are rewritten to match.

B) **Open by default, plus a private option at creation.** As A, but the owner can mark a session private when
creating it. A private session is members only, exactly as FR-2.3 reads today: non-members get "not found". Only
the owner can change visibility later. Fuller RBAC is post-MVP.

C) **As B, but private by default.** The owner chooses "open to everyone" when creating the session.

X) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question C2 (follows Q1)
What do people who read an open session without being members get?

A) **Read only, quietly.** They get no notifications, have no unread tracking and do not appear in presence. The
session list is split into "My sessions" and "Other sessions". Reading is not audited, as it is content they are
allowed to see.

B) **As A, plus they can follow a session** to get unread indicators, without becoming a member or being able to
post.

X) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question C3 (follows Q1 and Q3)
"Admins can add themselves." What can an admin do in a session they are not a member of?

A) **Join, then be an ordinary member.** An admin can add themselves to any session, which is audited. They also
have the owner reassignment from Q3. They have no other special rights: once joined, they cannot remove people or
end the session unless they are its owner. (For a private session under C1 B or C, joining is how an admin gets
in, and it is audited.)

B) **Full management of any session.** Admins can add and remove anyone, end, archive and transfer ownership of
any session, with every action audited.

X) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question C4 (Q11 was left blank)
Who can be @mentioned, and what happens when someone mentions a non-member? Sessions are now readable by others
(C1), so a non-member could follow a mention link and read the message.

A) **Members only.** The picker offers session members. Typing a non-member's name does nothing special, and no
notification is sent.

B) **Anyone who can read the session.** The picker offers members first, then other active users. A mentioned
non-member gets a notification and can read the session, but is not added as a member. In a private session (if
C1 is B or C), only members can be mentioned.

C) **As B, plus an offer to add.** When the author mentions a non-member, a prompt offers "Add them as a member?",
which any member can accept (Q2).

X) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question C5 (follows Q12)
A thread that has had no activity for a set time notifies every member. Who sets that time, and what is the
default?

A) **The instance admin sets a default, and the owner can override it per session.** The default is 24 hours, and
"off" is allowed at either level. Each member gets one notice per quiet period: new activity in the thread resets
the clock. Members who have already approved, and the author of the last message, are left out. A reopened thread
starts its clock again.

B) **The instance admin only.** One setting for all sessions, defaulting to 24 hours. Otherwise as A.

C) **The owner only, per session.** The default is 24 hours. Otherwise as A.

X) Other (please describe after [Answer]: tag below)

[Answer]: Three levels. Global default should be one business day with no activity, then admins can define a default for their instance and then session owners/admins can edit the notifications for a specific session.

## Question C6 (follows Q14)
A summary generated when a session ends needs a model, which arrives in U4. A "follow-up session" flow is new
scope that no current story covers. How should this be split?

A) **U3 asks the owner for a closing summary; U4 drafts it; the follow-up flow is a new story.**
- In U3, the end dialog asks the owner for a closing summary, which is required, matching Q5. It is stored, and
  shown at the top of the ended session.
- In U4, the orchestrator drafts the summary from the approved thread outcomes, and the owner edits it before
  ending.
- A new Should story, "Start a follow-up session", is assigned to U4. The new session links back to the old one,
  starts with the closing summary and the approved outcomes as context (not the full transcript), and offers the
  old members.
- Archive and unarchive work as in Q14 A, and an ended session is never reopened.

B) **Everything in U4.** U3 ends a session with a simple confirmation and no summary. U4 adds the generated summary
(the owner edits it before ending) and the follow-up flow. Archive and unarchive work as in A.

C) **As A, but the follow-up flow waits until after the MVP.**

X) Other (please describe after [Answer]: tag below)

[Answer]: B
