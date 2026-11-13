-- U2 instance administration: model providers, default models, search, the GitHub connector, the
-- sandbox policy and configuration test runs. Secrets are sealed by the vault before they reach
-- these columns (purposes provider_api_key, search_api_key and connector_secret, bound to the row).

CREATE TABLE providers (
    id             uuid PRIMARY KEY,
    kind           text        NOT NULL CHECK (kind IN ('openai_compatible')),
    name           text        NOT NULL,
    base_url       text        NOT NULL,
    api_key_sealed bytea,
    key_saved_at   timestamptz,
    created_at     timestamptz NOT NULL,
    created_by     uuid        NOT NULL REFERENCES users (id),
    updated_at     timestamptz NOT NULL,
    updated_by     uuid        NOT NULL REFERENCES users (id),
    CONSTRAINT providers_key_consistent CHECK ((api_key_sealed IS NULL) = (key_saved_at IS NULL))
);
-- Names are unique ignoring case (BR-14.2).
CREATE UNIQUE INDEX providers_name_key ON providers (lower(name));

CREATE TABLE provider_models (
    id                      uuid PRIMARY KEY,
    provider_id             uuid    NOT NULL REFERENCES providers (id) ON DELETE CASCADE,
    model_name              text    NOT NULL,
    label                   text,
    position                integer NOT NULL,
    detected_tool_calling   text    NOT NULL DEFAULT 'unknown'
        CHECK (detected_tool_calling IN ('supported', 'unsupported', 'unknown')),
    declared_tool_calling   text
        CHECK (declared_tool_calling IN ('supported', 'unsupported', 'unknown')),
    detected_context_tokens integer CHECK (detected_context_tokens > 0),
    declared_context_tokens integer CHECK (declared_context_tokens > 0),
    CONSTRAINT provider_models_name_key UNIQUE (provider_id, model_name)
);
CREATE INDEX provider_models_provider ON provider_models (provider_id, position);

-- One row at most; a default is cleared when its model goes (BR-15.3).
CREATE TABLE model_defaults (
    singleton             boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    orchestrator_model_id uuid REFERENCES provider_models (id) ON DELETE SET NULL,
    agent_model_id        uuid REFERENCES provider_models (id) ON DELETE SET NULL,
    updated_at            timestamptz NOT NULL,
    updated_by            uuid        NOT NULL REFERENCES users (id)
);

CREATE TABLE search_configs (
    kind           text PRIMARY KEY CHECK (kind IN ('searxng', 'brave', 'tavily')),
    base_url       text,
    api_key_sealed bytea,
    key_saved_at   timestamptz,
    updated_at     timestamptz NOT NULL,
    updated_by     uuid        NOT NULL REFERENCES users (id),
    CONSTRAINT search_configs_key_consistent CHECK ((api_key_sealed IS NULL) = (key_saved_at IS NULL))
);

-- At most one provider is active (BR-16.1); none is the absence of a value.
CREATE TABLE search_active (
    singleton  boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    kind       text REFERENCES search_configs (kind) ON DELETE SET NULL,
    updated_at timestamptz NOT NULL,
    updated_by uuid        NOT NULL REFERENCES users (id)
);

CREATE TABLE connectors (
    kind             text PRIMARY KEY CHECK (kind IN ('github')),
    auth_kind        text        NOT NULL CHECK (auth_kind IN ('token')),
    secret_sealed    bytea       NOT NULL,
    secret_saved_at  timestamptz NOT NULL,
    account_login    text,
    token_expires_at timestamptz,
    updated_at       timestamptz NOT NULL,
    updated_by       uuid        NOT NULL REFERENCES users (id)
);

-- One policy per scope; only 'instance' is written in the MVP (BR-18.1). The version rises with
-- every saved change so a run can record the policy it captured.
CREATE TABLE sandbox_policies (
    scope               text PRIMARY KEY CHECK (scope IN ('instance', 'session')),
    runner_tier         text    NOT NULL CHECK (runner_tier IN ('in_process', 'container')),
    tool_allow_list     text[]  NOT NULL,
    network_mode        text    NOT NULL CHECK (network_mode IN ('none', 'allow_hosts')),
    network_hosts       text[]  NOT NULL DEFAULT '{}',
    wall_clock_seconds  bigint  NOT NULL CHECK (wall_clock_seconds > 0),
    max_output_bytes    bigint  NOT NULL CHECK (max_output_bytes > 0),
    max_file_bytes      bigint  NOT NULL CHECK (max_file_bytes > 0),
    max_files_written   bigint  NOT NULL CHECK (max_files_written > 0),
    memory_mebibytes    bigint  NOT NULL CHECK (memory_mebibytes > 0),
    cpu_millicores      bigint  NOT NULL CHECK (cpu_millicores > 0),
    version             integer NOT NULL CHECK (version > 0),
    updated_at          timestamptz NOT NULL,
    updated_by          uuid        NOT NULL REFERENCES users (id),
    CONSTRAINT sandbox_policies_hosts_match_mode CHECK (network_mode = 'allow_hosts' OR network_hosts = '{}')
);

CREATE TABLE config_test_runs (
    id           uuid PRIMARY KEY,
    target       text        NOT NULL,
    status       text        NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    requested_by uuid        NOT NULL REFERENCES users (id),
    requested_at timestamptz NOT NULL,
    started_at   timestamptz,
    finished_at  timestamptz,
    results      jsonb       NOT NULL DEFAULT '[]',
    failure      text        CHECK (failure IN ('could_not_complete', 'no_worker_available'))
);
-- One active run per target (NFR design pattern 2): a second start violates this index.
CREATE UNIQUE INDEX config_test_runs_one_active ON config_test_runs (target)
    WHERE status IN ('queued', 'running');
CREATE INDEX config_test_runs_latest ON config_test_runs (target, requested_at DESC);
CREATE INDEX config_test_runs_stuck ON config_test_runs (requested_at) WHERE status = 'queued';

-- Least privilege: configuration tables are ordinary mutable tables for the application role
-- (SE-26); the audit table is untouched.
GRANT SELECT, INSERT, UPDATE, DELETE ON
    providers, provider_models, model_defaults, search_configs, search_active, connectors,
    sandbox_policies, config_test_runs TO tandem_app;
