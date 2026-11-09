REVOKE ALL ON
    users, invitations, sign_in_guards, instance_settings,
    password_resets, auth_sessions, mfa_credentials, recovery_codes, unknown_sign_in_guards,
    outbox_events, jobs, audit_entries, chat_sessions, chat_messages, _sqlx_migrations
FROM tandem_app;
