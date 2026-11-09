-- Least privilege for the application role (SE-7, SECURITY-06). No wildcard grants: every table is
-- listed. The audit table gets INSERT and SELECT only (SECURITY-14).

GRANT SELECT, INSERT, UPDATE ON users, sign_in_guards, instance_settings TO tandem_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON
    invitations, password_resets, auth_sessions, mfa_credentials, recovery_codes, unknown_sign_in_guards,
    outbox_events, jobs TO tandem_app;
GRANT SELECT, INSERT ON audit_entries, chat_sessions, chat_messages TO tandem_app;
-- The schema guard reads the applied migration versions.
GRANT SELECT ON _sqlx_migrations TO tandem_app;
