-- The audit log (SECURITY-14). Append-only: the application role is granted INSERT and SELECT
-- only (see 0004_grants), and no application code updates or deletes rows.
CREATE TABLE audit_entries (
    id          uuid PRIMARY KEY,
    occurred_at timestamptz NOT NULL,
    actor_id    uuid,
    actor_label text        NOT NULL,
    event_type  text        NOT NULL,
    object_type text,
    object_id   text,
    outcome     text        NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
    details     jsonb       NOT NULL DEFAULT '{}'::jsonb,
    request_id  text,
    source      text
);
CREATE INDEX audit_entries_time ON audit_entries (occurred_at DESC, id DESC);
CREATE INDEX audit_entries_actor_time ON audit_entries (actor_id, occurred_at DESC, id DESC);
CREATE INDEX audit_entries_type_time ON audit_entries (event_type, occurred_at DESC, id DESC);
