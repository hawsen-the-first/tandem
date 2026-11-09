-- Walking skeleton (replaced by the full model in later units): chat, outbox and jobs.

CREATE TABLE chat_sessions (
    id         uuid PRIMARY KEY,
    title      text        NOT NULL,
    owner_id   uuid        NOT NULL REFERENCES users (id),
    created_at timestamptz NOT NULL
);
CREATE INDEX chat_sessions_owner ON chat_sessions (owner_id, created_at DESC);

CREATE TABLE chat_messages (
    -- seq keeps insertion order for messages with the same timestamp.
    seq             bigint GENERATED ALWAYS AS IDENTITY,
    id              uuid PRIMARY KEY,
    chat_session_id uuid        NOT NULL REFERENCES chat_sessions (id),
    author_kind     text        NOT NULL CHECK (author_kind IN ('user', 'agent')),
    author_user_id  uuid REFERENCES users (id),
    author_agent    text,
    body            text        NOT NULL,
    created_at      timestamptz NOT NULL,
    in_reply_to     uuid REFERENCES chat_messages (id),
    CONSTRAINT chat_messages_seq_key UNIQUE (seq),
    CONSTRAINT chat_messages_author_consistent CHECK (
        (author_kind = 'user' AND author_user_id IS NOT NULL AND author_agent IS NULL)
        OR (author_kind = 'agent' AND author_agent IS NOT NULL AND author_user_id IS NULL))
);
CREATE INDEX chat_messages_session ON chat_messages (chat_session_id, seq);
-- At most one reply per message (BR-13.4).
CREATE UNIQUE INDEX chat_messages_in_reply_to_key ON chat_messages (in_reply_to) WHERE in_reply_to IS NOT NULL;

CREATE TABLE outbox_events (
    id            bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    kind          text        NOT NULL,
    payload       jsonb       NOT NULL,
    trace_context text,
    created_at    timestamptz NOT NULL,
    published_at  timestamptz
);
CREATE INDEX outbox_events_unpublished ON outbox_events (id) WHERE published_at IS NULL;
CREATE INDEX outbox_events_published_at ON outbox_events (published_at) WHERE published_at IS NOT NULL;

CREATE TABLE jobs (
    id                uuid PRIMARY KEY,
    kind              text        NOT NULL,
    payload           jsonb       NOT NULL,
    status            text        NOT NULL CHECK (status IN ('queued', 'leased', 'done', 'failed')),
    attempts          integer     NOT NULL DEFAULT 0,
    run_at            timestamptz NOT NULL,
    last_error        text,
    lease_expires_at  timestamptz,
    trace_context     text,
    created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX jobs_due ON jobs (run_at, id) WHERE status = 'queued';
CREATE INDEX jobs_leased ON jobs (lease_expires_at) WHERE status = 'leased';
CREATE INDEX jobs_finished ON jobs (created_at) WHERE status IN ('done', 'failed');
