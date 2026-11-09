# Tandem: Component Methods

Method signatures are Rust-flavoured pseudocode: they fix names, inputs and outputs, not implementation. **Business rules are detailed later in Functional Design (per unit).**

Conventions: `Ctx` carries the authenticated actor (user id, role) and a request id. Every application service method begins by calling `Access::authorize` and returns `Result<T, AppError>`. Ids are typed (`SessionId`, `ThreadId`, and so on). Pagination is cursor-based.

## C4 Identity and Access
| Method | Purpose |
|---|---|
| `bootstrap_admin(email, password) -> User` | First-run admin; fails once any admin exists |
| `invite(ctx, email, role) -> Invitation` | Single-use, expiring invitation |
| `accept_invite(token, password) -> User` | Sets password after breached-password check |
| `sign_in(email, password, totp: Option<Code>) -> AuthSession` | Rate-limited; generic errors |
| `sign_out(ctx)` / `expire_sessions(user)` | End logins |
| `enrol_mfa(ctx) -> MfaEnrolment`, `confirm_mfa(ctx, code) -> RecoveryCodes` | TOTP |
| `change_password(ctx, old, new)`, `force_reset(ctx, user)` | Credential changes |
| `deactivate(ctx, user)`, `remove(ctx, user)` | Never the last admin |
| `authorize(ctx, Permission, Resource) -> Result<()>` | Central check for admin, session membership and ownership |

## C5 Session Service
| Method | Purpose |
|---|---|
| `create_session(ctx, NewSession{title, prompt, workspace, models}) -> Session` | Creates session and enqueues workspace preparation |
| `add_member(ctx, session, user)`, `remove_member(...)` | Owner only |
| `get_overview(ctx, session) -> SessionOverview` | Prompt, rounds, threads, plan, tasks, artifacts |
| `end_session(ctx, session, confirm_discard: bool)` | Requires confirmation when uncommitted changes exist |
| `archive_session(ctx, session)` | Ended sessions only |
| `set_models(ctx, session, ModelChoice)` | Within configured providers |

## C6 Thread and Messaging
| Method | Purpose |
|---|---|
| `open_round(session, questions: Vec<ProposedQuestion>) -> Round` | Called by C14 with validated proposals |
| `edit_question(ctx, thread, patch)`, `add_question(ctx, round, q)`, `remove_question(ctx, thread)` | Only before the first message |
| `post_message(ctx, thread_or_chat, NewMessage) -> Message` | Markdown, references, replies; size-limited |
| `list_messages(ctx, target, cursor) -> Page<Message>` | |
| `attach_file(ctx, target, upload) -> FileRef` | Stored via C23 with content hash |
| `request_round(ctx, session, note)` | Before plan approval only |

## C7 Approval Service
| Method | Purpose |
|---|---|
| `approve(ctx, Approvable) -> Approval` | `Approvable` = `Thread`, `Plan`, `TaskOutput`; humans only; single approval closes (rule behind a `ApprovalPolicy` trait) |
| `revoke(ctx, approval) -> Result<()>` | Refused when dependants exist |
| `edit_outcome_summary(ctx, thread, text)` | Before approval |
| `list_approvals(ctx, target) -> Vec<Approval>` | Who and when |

## C8 Plan Service
| Method | Purpose |
|---|---|
| `store_draft(session, PlanProposal) -> PlanVersion` | Validates steps, dependencies (acyclic) and thread links; called by C14 |
| `request_rework(ctx, plan, feedback)` | Enqueues redraft |
| `open_step_thread(ctx, plan, step) -> Thread` | Humans only |
| `get_version(ctx, plan, n)`, `compare(ctx, plan, a, b)` | History |
| `lock_on_approval(plan_version)` | Called through C7 |

## C9 Task Service
| Method | Purpose |
|---|---|
| `spawn_from_plan(plan_version) -> Vec<Task>` | Only for an approved plan; C14 only |
| `next_runnable(session) -> Vec<Task>` | Dependencies approved |
| `record_output(task, OutputSnapshot)` | `Diff` or `ArtifactVersion` snapshot stored in the database |
| `reject(ctx, task, feedback)`, `discard(ctx, task)`, `retry(ctx, task)` | Review actions |
| `set_status(task, TaskStatus)` | `queued`, `running`, `needs_review`, `approved`, `failed` |

## C10 Artifact Service
| Method | Purpose |
|---|---|
| `create(actor, session, ArtifactDraft) -> Artifact` | Actor = person, agent or task |
| `new_version(actor, artifact, body) -> ArtifactVersion` | Content hash recorded |
| `diff(ctx, artifact, a, b) -> Diff`, `download(ctx, artifact, version)` | |
| `save_to_workspace(ctx, artifact, path)` | Enqueues workspace write via C17 |

## C11 Notification and Presence
| Method | Purpose |
|---|---|
| `notify(users, Notification)` | Mention, awaiting approval, plan ready, task output ready |
| `mark_read(ctx, ids)`, `unread_counts(ctx) -> Counts` | |
| `heartbeat(ctx, session)`, `online_members(ctx, session)` | Presence |

## C12 Admin and Settings
| Method | Purpose |
|---|---|
| `upsert_provider(ctx, ProviderConfig)`, `test_provider(ctx, id) -> ProviderReport` | Keys stored via C24 |
| `set_defaults(ctx, ModelDefaults)` | Orchestrator and agents |
| `configure_search(ctx, SearchConfig)`, `test_search(ctx)` | |
| `configure_connector(ctx, ConnectorConfig)`, `test_connector(ctx)` | GitHub |
| `set_sandbox_policy(ctx, SandboxPolicy)`, `get_sandbox_policy(ctx)` | **New.** Instance-wide |
| `usage_by_session(ctx, session) -> Usage` | Visibility only |

## C13 Audit Service
| Method | Purpose |
|---|---|
| `record(event: AuditEvent)` | Append-only; failure is surfaced, not swallowed |
| `query(ctx, filter) -> Page<AuditEntry>` | Admin only |

## C14 Orchestration Engine
| Method | Purpose |
|---|---|
| `on_event(DomainEvent)` | Entry point; looks up allowed transitions and enqueues jobs |
| `propose_questions(session) -> ProposedQuestions` | Job; model output parsed and validated |
| `summarise_thread(thread) -> OutcomeSummary` | Job on approval |
| `decide_next_round(session) -> RoundDecision` | Job after a round closes |
| `draft_plan(session) -> PlanProposal` | Job |
| `run_ready_tasks(session)` | Enqueues task jobs in dependency order |
| `transition(entity, Event) -> Result<NewState>` | Pure state machine in `core` |

## C15 Agent Runtime
| Method | Purpose |
|---|---|
| `run_thread_agent(call: AgentCall) -> AgentRun` | `@ask`, `@code`, `@search`, `@draft`; streams events; cancellable |
| `run_task_agent(task) -> OutputSnapshot` | Writes only through the tool layer |
| `cancel(run_id)` | Caller-initiated |
| `ToolLayer::invoke(run, ToolCall) -> ToolResult` | Policy check, then runner; logs to the activity log |
| `build_context(call) -> Context` | Thread outcomes, session prompt, selected files; untrusted content is marked |

## C16 Sandbox Manager
| Method | Purpose |
|---|---|
| `resolve_policy(run: RunKind, grants: SessionGrants) -> RunPolicy` | Combines the instance policy with per-run path bindings |
| `runner_for(policy) -> Box<dyn SandboxRunner>` | `in-process` or `container` |
| `SandboxRunner::execute(policy, ToolCall) -> ToolResult` | Enforces confinement; denies by default |
| `SandboxRunner::teardown(run)` | Removes containers and scratch space |
| `check_path(policy, path, Access) -> Result<ResolvedPath>` | Canonicalises, rejects traversal and symlink escape |

## C17 Workspace Manager
| Method | Purpose |
|---|---|
| `prepare(session, WorkspaceSpec) -> Workspace` | Clone and session branch, or an empty managed folder |
| `add_readonly_repo(session, repo) -> Workspace` | After approval |
| `snapshot_diff(workspace) -> Diff` | Stored with the task output |
| `commit_and_push(ctx, workspace, message) -> CommitRef` | Explicit member action only |
| `open_pull_request(ctx, workspace, base, body) -> PrRef` | Via C22 |
| `cleanup(session)` | Idempotent and retried on failure |

## C18 Job Queue, C19 Event Bus
| Method | Purpose |
|---|---|
| `enqueue(Job, run_at) -> JobId`, `lease(kinds, ttl) -> Option<Lease>` | `SKIP LOCKED` |
| `heartbeat(lease)`, `complete(lease)`, `fail(lease, error, retry_policy)` | Dead-letter after retries |
| `publish(Event)`, `subscribe(Filter) -> Stream<Event>` | In-process plus `NOTIFY` |

## C20 to C24 Ports
| Port | Key methods |
|---|---|
| `ModelProvider` | `capabilities(model) -> Capabilities`, `chat_stream(request) -> Stream<Chunk>`, `health() -> Health` |
| `SearchProvider` | `search(query, limit) -> Vec<CitedResult>` |
| `Connector` | `list_repos() -> Vec<Repo>`, `clone_auth(repo) -> ShortLivedCredential`, `create_pull_request(...)`, `permissions_report()` |
| `BlobStore` | `put(bytes) -> ContentHash`, `get(hash) -> Bytes`, `delete(hash)` |
| `SecretVault` | `seal(plaintext) -> Sealed`, `open(sealed) -> Plaintext` |

## C25 to C27
| Component | Key methods |
|---|---|
| Persistence | `with_transaction(f)`, per-aggregate repositories, `migrate_up()`, `migrate_down(n)` |
| Observability | `healthz()`, `readyz()` (database, queue, vault checks), `metrics()` |
| Operations CLI | `tandem migrate`, `tandem backup <dir>`, `tandem restore <dir>`, `tandem admin reset-mfa <user>` |
