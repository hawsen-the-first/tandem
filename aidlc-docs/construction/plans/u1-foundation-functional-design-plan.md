# Functional Design Plan: U1 Foundation and walking skeleton

**Stage**: CONSTRUCTION - Functional Design (unit `u1-foundation`)
**Inputs**: `unit-of-work.md` (U1), `unit-of-work-story-map.md` (10 stories), `application-design.md`, `stories.md`, `requirements.md`
**Stories in scope**: US-1.1 to US-1.7 (accounts and access), US-2.7 (audit log viewer), US-13.3 (sign-in lockout), US-13.7 (mobile, themes, WCAG baseline), plus the walking-skeleton stubs (session, message, queue, outbox, event bus, scripted mock provider)
**Scope**: business logic, domain model and rules, technology-agnostic. Libraries, hashing parameters and framework choices belong to NFR Requirements and NFR Design.

## Working assumptions (not questions)
- **A1**: Email address, compared case-insensitively and unique, is the sign-in identifier.
- **A2**: Invitation, password-reset and session tokens are random, high-entropy, stored only as hashes, and single-use where they are links.
- **A3**: TOTP uses 6-digit codes, 30-second steps and a one-step tolerance either side. Recovery codes are single-use and stored hashed.
- **A4**: Errors at sign-in, invitation and reset never reveal whether an email exists (US-1.2).
- **A5**: There is exactly one team per instance; there is no sign-up path other than first-run setup and invitations (FR-1.2, FR-1.3).
- **A6**: The skeleton's session is the thinnest possible: title, owner, managed-folder workspace type; the skeleton's message is session chat only (author, body, time). Both are replaced by the full U3 model rather than extended in place.

## Questions

## Question 1
What password policy should apply (US-1.2, US-1.6)?

A) Length-based: at least 12 characters, no composition rules, rejected if found in the breached-password list (bundled offline list; an optional online k-anonymity check the admin can switch on)

B) Length plus composition: at least 10 characters with upper, lower, digit and symbol

C) Longer passphrases only: at least 16 characters, no composition rules, breached-password check as in A

D) Other (please describe after [Answer]: tag below)

[Answer]: B (plus breached-password check, see Clarification 1)

## Question 2
How long should a sign-in last (US-1.2)?

A) Same for everyone: signed out after 12 hours idle or 14 days in total

B) Stricter for admins: admins 2 hours idle or 24 hours in total; members 12 hours idle or 14 days in total

C) Same defaults as A, but the admin can change them in instance settings

D) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 3
How should repeated failed sign-ins be handled (US-13.3)?

A) Temporary lockout: 5 failed attempts for one account within 15 minutes locks that account for 15 minutes, plus a per-source rate limit

B) Progressive delay instead of lockout: each failure adds a growing delay, plus a per-source rate limit; no account is ever fully locked (avoids lockout being used to block a colleague)

C) Both: progressive delay first, then a temporary lockout after 10 failures in an hour

D) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 4
Email sending is out of scope for the MVP (FR-14.1), but US-1.3 says "invite people by email". How should invitations and password resets reach people?

A) No email in the MVP: the admin gets a one-time link to copy and send through their own channel; forced password resets work the same way

B) Optional SMTP settings in U2 so Tandem can email invitations and reset links; the copyable link remains as the fallback

C) The admin creates the account with a one-time temporary password the person must change at first sign-in

D) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 5
When must an admin enrol in MFA (FR-1.5, US-1.1)?

A) Immediately: right after setting a password at first-run or after an invitation as admin, nothing else is reachable until MFA is enrolled

B) Within a 7-day grace period, with a persistent reminder, after which admin access is blocked until enrolled

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 6
Can an admin change another user's role after invitation (FR-1.4)? The stories only set the role at invite time.

A) Yes: admins can promote a member to admin or demote an admin to member, never leaving zero admins (adds a small story, US-1.8)

B) No: the role is fixed at invitation; changing it means removing and re-inviting

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 7
What is the difference between deactivating and removing a user (US-1.4)?

A) Deactivate: blocks sign-in, reversible. Remove: permanent; the login and personal details are erased and past content stays attributed to "Former member"

B) Deactivate: blocks sign-in, reversible. Remove: only allowed for users who have never taken any action; otherwise deactivate is the only option

C) There is only deactivate (reversible); no permanent removal in the MVP

D) Other (please describe after [Answer]: tag below)

[Answer]: C

## Question 8
How long should audit log entries be kept (US-2.7, SECURITY-14)?

A) Indefinitely: nothing deletes entries from the product; operators can prune with a documented CLI command later if they want to

B) A configurable retention period (default 12 months) after which old entries are pruned automatically

C) Other (please describe after [Answer]: tag below)

[Answer]: B

## Question 9
What should the walking skeleton's worker do to prove the round trip (message to API, database, queue, worker, event, WebSocket)?

A) A scripted mock agent in the worker replies in the session chat ("Received: ..."), which also exercises the mock model provider, queue, outbox and event bus

B) The worker only records a "message processed" event shown in the UI, with no agent reply

C) Other (please describe after [Answer]: tag below)

[Answer]: A

## Question 10
What profile information does a user have?

A) Display name and email only

B) Display name, email and an optional avatar image

C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Execution Checklist (generation, after answers are approved)
- [x] 1. Model the domain entities and relationships for identity, audit and the skeleton stubs; write `domain-entities.md`
- [x] 2. Define the business logic and workflows (first-run setup, invite, accept, sign-in with and without MFA, MFA enrolment, password change and reset, deactivate and remove, lockout, audit recording, skeleton round trip); write `business-logic-model.md`
- [x] 3. Define business rules, validation and error handling per workflow, including audit events emitted; write `business-rules.md`
- [x] 4. Define screens, components, states, form validation, and the API each uses, including the responsive, theme and accessibility baseline (US-13.7); write `frontend-components.md`
- [x] 5. Trace every U1 story to the workflows and rules that satisfy it (traceability check); amend `stories.md` if answers add or change a story
- [x] 6. Check the design against Security Baseline rules that apply at this stage (SECURITY-08, SECURITY-12, SECURITY-14, SECURITY-15) and note the rest for NFR stages
- [x] 7. Update state, log the approval prompt, present the completion message

## Extension compliance (planning)
- Security Baseline: applied at generation (checklist step 6), implementation detail deferred to NFR Requirements and Design
- Resiliency Baseline: N/A for functional design; U1 NFR stages cover health, metrics and migrations
- Property-Based Testing: disabled

---

## Clarification 1 (Q1): breached-password check
Q1 B (length plus composition) did not mention the breached-password check that US-1.2, US-1.6 and SECURITY-12 require. Resolved: the check applies **on top**. Policy: at least 10 characters with upper, lower, digit and symbol, and rejected if found in the breached-password list (bundled offline list; optional online k-anonymity check the admin can enable).

## Consequences of the answers
| Answer | Consequence |
|---|---|
| Q2 C | Sign-in lasts 12 hours idle or 14 days total by default, adjustable by the admin. U1 therefore needs a small **instance settings** store and screen (new story US-1.9) |
| Q3 C | Progressive delay on each failure, temporary lockout after 10 failures in an hour, plus a per-source rate limit |
| Q4 A | No email in the MVP. Invitations and forced resets produce a **one-time link the admin copies**. A user who forgets their password asks an admin for a reset link; there is no self-service reset in the MVP |
| Q5 A | Admins must enrol MFA immediately; nothing else is reachable until they do |
| Q6 A | **New story US-1.8** (change a user's role; never zero admins); new FR-1.7 |
| Q7 C | **Deactivate only (reversible); no permanent removal in the MVP.** This amends FR-1.3 and US-1.4, which said "remove" |
| Q8 B | Audit retention is configurable (default 12 months) with automatic pruning; configured in the same instance settings (US-1.9). US-2.7 gains a retention criterion |
| Q9 A | A scripted mock agent in the worker replies in the session chat |
| Q10 A | A user has a display name and an email; no avatar |

## Assumptions added (confirm at approval)
- **A7**: An invitation link expires after 7 days; a password-reset link expires after 24 hours.
- **A8**: A deactivated user can be reactivated by an admin; reactivation requires the user to sign in again (old sessions stay ended) and re-uses their existing password and MFA.
- **A9**: The progressive delay doubles after each failure from 1 second, capped at 60 seconds. The per-source limit is 20 sign-in attempts per minute.
