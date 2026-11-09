-- Identity: users, invitations, resets, sign-ins, MFA, sign-in guards and instance settings.
-- Runs under the migration role in the `tandem` schema (search_path is set on the role).

CREATE TABLE users (
    id                  uuid PRIMARY KEY,
    email               text        NOT NULL,
    display_name        text        NOT NULL,
    role                text        NOT NULL CHECK (role IN ('admin', 'member')),
    status              text        NOT NULL CHECK (status IN ('active', 'deactivated')),
    password_hash       text        NOT NULL,
    password_changed_at timestamptz NOT NULL,
    created_at          timestamptz NOT NULL,
    created_via         text        NOT NULL CHECK (created_via IN ('first_run_setup', 'invitation')),
    deactivated_at      timestamptz,
    deactivated_by      uuid REFERENCES users (id),
    last_sign_in_at     timestamptz,
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_email_normalised CHECK (email = lower(btrim(email)))
);
CREATE INDEX users_active_admins ON users (role, status) WHERE role = 'admin' AND status = 'active';

CREATE TABLE invitations (
    id               uuid PRIMARY KEY,
    email            text        NOT NULL,
    role             text        NOT NULL CHECK (role IN ('admin', 'member')),
    token_hash       text        NOT NULL,
    created_by       uuid        NOT NULL REFERENCES users (id),
    created_at       timestamptz NOT NULL,
    expires_at       timestamptz NOT NULL,
    status           text        NOT NULL CHECK (status IN ('pending', 'accepted', 'revoked', 'expired')),
    accepted_user_id uuid REFERENCES users (id),
    CONSTRAINT invitations_token_hash_key UNIQUE (token_hash)
);
CREATE INDEX invitations_pending_email ON invitations (email) WHERE status = 'pending';
CREATE INDEX invitations_created_at ON invitations (created_at DESC);

CREATE TABLE password_resets (
    id          uuid PRIMARY KEY,
    user_id     uuid        NOT NULL REFERENCES users (id),
    token_hash  text        NOT NULL,
    created_by  uuid        NOT NULL REFERENCES users (id),
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    reset_mfa   boolean     NOT NULL,
    used_at     timestamptz,
    CONSTRAINT password_resets_token_hash_key UNIQUE (token_hash)
);
CREATE INDEX password_resets_unused_for_user ON password_resets (user_id) WHERE used_at IS NULL;

CREATE TABLE auth_sessions (
    id                        uuid PRIMARY KEY,
    user_id                   uuid        NOT NULL REFERENCES users (id),
    token_hash                text        NOT NULL,
    created_at                timestamptz NOT NULL,
    last_seen_at              timestamptz NOT NULL,
    idle_timeout_seconds      bigint      NOT NULL,
    absolute_duration_seconds bigint      NOT NULL,
    scope                     text        NOT NULL CHECK (scope IN ('full', 'mfa_enrolment_only')),
    ended_at                  timestamptz,
    ended_reason              text CHECK (ended_reason IN (
        'signed_out', 'idle_timeout', 'absolute_timeout', 'password_changed', 'deactivated', 'reset_issued')),
    source_summary            text,
    CONSTRAINT auth_sessions_token_hash_key UNIQUE (token_hash),
    CONSTRAINT auth_sessions_ended_consistent CHECK ((ended_at IS NULL) = (ended_reason IS NULL))
);
CREATE INDEX auth_sessions_active_for_user ON auth_sessions (user_id) WHERE ended_at IS NULL;
CREATE INDEX auth_sessions_ended_at ON auth_sessions (ended_at) WHERE ended_at IS NOT NULL;

CREATE TABLE mfa_credentials (
    user_id        uuid PRIMARY KEY REFERENCES users (id),
    secret_sealed  bytea       NOT NULL,
    confirmed_at   timestamptz,
    last_used_step bigint
);

CREATE TABLE recovery_codes (
    id        uuid PRIMARY KEY,
    user_id   uuid NOT NULL REFERENCES users (id),
    code_hash text NOT NULL,
    used_at   timestamptz,
    CONSTRAINT recovery_codes_user_code_key UNIQUE (user_id, code_hash)
);

CREATE TABLE sign_in_guards (
    user_id           uuid PRIMARY KEY REFERENCES users (id),
    failure_count     integer NOT NULL DEFAULT 0,
    window_started_at timestamptz,
    delay_until       timestamptz,
    locked_until      timestamptz
);

-- Guards for emails that match no account (BR-3.5). Keyed by a hash so nothing readable is stored.
CREATE TABLE unknown_sign_in_guards (
    key_hash          text PRIMARY KEY,
    failure_count     integer     NOT NULL DEFAULT 0,
    window_started_at timestamptz,
    delay_until       timestamptz,
    locked_until      timestamptz,
    updated_at        timestamptz NOT NULL
);
CREATE INDEX unknown_sign_in_guards_updated_at ON unknown_sign_in_guards (updated_at);

CREATE TABLE instance_settings (
    key        text PRIMARY KEY,
    value      text        NOT NULL,
    updated_by uuid REFERENCES users (id),
    updated_at timestamptz NOT NULL
);
