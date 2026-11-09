//! Repositories for users, invitations, resets, sign-ins and MFA.

use crate::convert;
use crate::error::map_sqlx;
use crate::store::PgTx;
use async_trait::async_trait;
use tandem_core::email::Email;
use tandem_core::ids::{AuthSessionId, InvitationId, PasswordResetId, RecoveryCodeId, UserId};
use tandem_core::invitation::Invitation;
use tandem_core::mfa::{MfaCredential, RecoveryCode};
use tandem_core::reset::PasswordReset;
use tandem_core::session::{AuthSession, EndReason};
use tandem_core::token::TokenHash;
use tandem_core::user::User;
use tandem_ports::{InvitationRepo, MfaRepo, PasswordResetRepo, SessionRepo, StoreError, UserRepo};
use time::OffsetDateTime;
use uuid::Uuid;

struct UserRow {
    id: Uuid,
    email: String,
    display_name: String,
    role: String,
    status: String,
    password_hash: String,
    password_changed_at: OffsetDateTime,
    created_at: OffsetDateTime,
    created_via: String,
    deactivated_at: Option<OffsetDateTime>,
    deactivated_by: Option<Uuid>,
    last_sign_in_at: Option<OffsetDateTime>,
}

impl UserRow {
    fn into_user(self) -> Result<User, StoreError> {
        Ok(User {
            id: UserId::from_uuid(self.id),
            email: convert::email(&self.email)?,
            display_name: convert::display_name(&self.display_name)?,
            role: convert::role(&self.role)?,
            status: convert::user_status(&self.status)?,
            password_hash: self.password_hash,
            password_changed_at: self.password_changed_at,
            created_at: self.created_at,
            created_via: convert::created_via(&self.created_via)?,
            deactivated_at: self.deactivated_at,
            deactivated_by: self.deactivated_by.map(UserId::from_uuid),
            last_sign_in_at: self.last_sign_in_at,
        })
    }
}

#[async_trait]
impl UserRepo for PgTx {
    async fn insert(&mut self, user: &User) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO users (id, email, display_name, role, status, password_hash, password_changed_at,
                                created_at, created_via, deactivated_at, deactivated_by, last_sign_in_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            user.id.as_uuid(),
            user.email.as_str(),
            user.display_name.as_str(),
            user.role.as_str(),
            user.status.as_str(),
            user.password_hash,
            user.password_changed_at,
            user.created_at,
            user.created_via.as_str(),
            user.deactivated_at,
            user.deactivated_by.map(|id| id.as_uuid()),
            user.last_sign_in_at,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get(&mut self, id: UserId) -> Result<Option<User>, StoreError> {
        sqlx::query_as!(
            UserRow,
            "SELECT id, email, display_name, role, status, password_hash, password_changed_at, created_at,
                    created_via, deactivated_at, deactivated_by, last_sign_in_at
             FROM users WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(UserRow::into_user)
        .transpose()
    }

    async fn find_by_email(&mut self, email: &Email) -> Result<Option<User>, StoreError> {
        sqlx::query_as!(
            UserRow,
            "SELECT id, email, display_name, role, status, password_hash, password_changed_at, created_at,
                    created_via, deactivated_at, deactivated_by, last_sign_in_at
             FROM users WHERE email = $1",
            email.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(UserRow::into_user)
        .transpose()
    }

    async fn list(&mut self) -> Result<Vec<User>, StoreError> {
        sqlx::query_as!(
            UserRow,
            "SELECT id, email, display_name, role, status, password_hash, password_changed_at, created_at,
                    created_via, deactivated_at, deactivated_by, last_sign_in_at
             FROM users ORDER BY created_at, id"
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .into_iter()
        .map(UserRow::into_user)
        .collect()
    }

    async fn update(&mut self, user: &User) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE users SET display_name = $2, role = $3, status = $4, password_hash = $5,
                    password_changed_at = $6, deactivated_at = $7, deactivated_by = $8, last_sign_in_at = $9
             WHERE id = $1",
            user.id.as_uuid(),
            user.display_name.as_str(),
            user.role.as_str(),
            user.status.as_str(),
            user.password_hash,
            user.password_changed_at,
            user.deactivated_at,
            user.deactivated_by.map(|id| id.as_uuid()),
            user.last_sign_in_at,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn count_all(&mut self) -> Result<u64, StoreError> {
        let row = sqlx::query!(r#"SELECT count(*) AS "count!" FROM users"#)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        Ok(u64::try_from(row.count).unwrap_or(0))
    }

    async fn count_active_admins(&mut self) -> Result<u64, StoreError> {
        let row = sqlx::query!(
            r#"SELECT count(*) AS "count!" FROM users WHERE role = 'admin' AND status = 'active'"#
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(u64::try_from(row.count).unwrap_or(0))
    }
}

struct InvitationRow {
    id: Uuid,
    email: String,
    role: String,
    token_hash: String,
    created_by: Uuid,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    status: String,
    accepted_user_id: Option<Uuid>,
}

impl InvitationRow {
    fn into_invitation(self) -> Result<Invitation, StoreError> {
        Ok(Invitation {
            id: InvitationId::from_uuid(self.id),
            email: convert::email(&self.email)?,
            role: convert::role(&self.role)?,
            token_hash: TokenHash::from_stored(self.token_hash),
            created_by: UserId::from_uuid(self.created_by),
            created_at: self.created_at,
            expires_at: self.expires_at,
            status: convert::invitation_status(&self.status)?,
            accepted_user_id: self.accepted_user_id.map(UserId::from_uuid),
        })
    }
}

#[async_trait]
impl InvitationRepo for PgTx {
    async fn insert(&mut self, invitation: &Invitation) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO invitations (id, email, role, token_hash, created_by, created_at, expires_at, status,
                                      accepted_user_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            invitation.id.as_uuid(),
            invitation.email.as_str(),
            invitation.role.as_str(),
            invitation.token_hash.as_str(),
            invitation.created_by.as_uuid(),
            invitation.created_at,
            invitation.expires_at,
            invitation.status.as_str(),
            invitation.accepted_user_id.map(|id| id.as_uuid()),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get(&mut self, id: InvitationId) -> Result<Option<Invitation>, StoreError> {
        sqlx::query_as!(
            InvitationRow,
            "SELECT id, email, role, token_hash, created_by, created_at, expires_at, status, accepted_user_id
             FROM invitations WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(InvitationRow::into_invitation)
        .transpose()
    }

    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<Invitation>, StoreError> {
        sqlx::query_as!(
            InvitationRow,
            "SELECT id, email, role, token_hash, created_by, created_at, expires_at, status, accepted_user_id
             FROM invitations WHERE token_hash = $1",
            hash.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(InvitationRow::into_invitation)
        .transpose()
    }

    async fn find_pending_by_email(
        &mut self,
        email: &Email,
        now: OffsetDateTime,
    ) -> Result<Option<Invitation>, StoreError> {
        sqlx::query_as!(
            InvitationRow,
            "SELECT id, email, role, token_hash, created_by, created_at, expires_at, status, accepted_user_id
             FROM invitations WHERE email = $1 AND status = 'pending' AND expires_at > $2
             ORDER BY created_at DESC LIMIT 1",
            email.as_str(),
            now
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(InvitationRow::into_invitation)
        .transpose()
    }

    async fn list(&mut self) -> Result<Vec<Invitation>, StoreError> {
        sqlx::query_as!(
            InvitationRow,
            "SELECT id, email, role, token_hash, created_by, created_at, expires_at, status, accepted_user_id
             FROM invitations ORDER BY created_at DESC, id DESC"
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .into_iter()
        .map(InvitationRow::into_invitation)
        .collect()
    }

    async fn update(&mut self, invitation: &Invitation) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE invitations SET status = $2, accepted_user_id = $3 WHERE id = $1",
            invitation.id.as_uuid(),
            invitation.status.as_str(),
            invitation.accepted_user_id.map(|id| id.as_uuid()),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }
}

struct ResetRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: String,
    created_by: Uuid,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    reset_mfa: bool,
    used_at: Option<OffsetDateTime>,
}

impl ResetRow {
    fn into_reset(self) -> PasswordReset {
        PasswordReset {
            id: PasswordResetId::from_uuid(self.id),
            user_id: UserId::from_uuid(self.user_id),
            token_hash: TokenHash::from_stored(self.token_hash),
            created_by: UserId::from_uuid(self.created_by),
            created_at: self.created_at,
            expires_at: self.expires_at,
            reset_mfa: self.reset_mfa,
            used_at: self.used_at,
        }
    }
}

#[async_trait]
impl PasswordResetRepo for PgTx {
    async fn insert(&mut self, reset: &PasswordReset) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO password_resets (id, user_id, token_hash, created_by, created_at, expires_at, reset_mfa,
                                          used_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            reset.id.as_uuid(),
            reset.user_id.as_uuid(),
            reset.token_hash.as_str(),
            reset.created_by.as_uuid(),
            reset.created_at,
            reset.expires_at,
            reset.reset_mfa,
            reset.used_at,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<PasswordReset>, StoreError> {
        Ok(sqlx::query_as!(
            ResetRow,
            "SELECT id, user_id, token_hash, created_by, created_at, expires_at, reset_mfa, used_at
             FROM password_resets WHERE token_hash = $1",
            hash.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(ResetRow::into_reset))
    }

    async fn invalidate_unused_for_user(
        &mut self,
        user_id: UserId,
        now: OffsetDateTime,
    ) -> Result<u64, StoreError> {
        let result = sqlx::query!(
            "UPDATE password_resets SET used_at = $2 WHERE user_id = $1 AND used_at IS NULL",
            user_id.as_uuid(),
            now
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }

    async fn update(&mut self, reset: &PasswordReset) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE password_resets SET used_at = $2 WHERE id = $1",
            reset.id.as_uuid(),
            reset.used_at
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }
}

struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: String,
    created_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    idle_timeout_seconds: i64,
    absolute_duration_seconds: i64,
    scope: String,
    ended_at: Option<OffsetDateTime>,
    ended_reason: Option<String>,
    source_summary: Option<String>,
}

impl SessionRow {
    fn into_session(self) -> Result<AuthSession, StoreError> {
        Ok(AuthSession {
            id: AuthSessionId::from_uuid(self.id),
            user_id: UserId::from_uuid(self.user_id),
            token_hash: TokenHash::from_stored(self.token_hash),
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
            idle_timeout: convert::duration(self.idle_timeout_seconds),
            absolute_duration: convert::duration(self.absolute_duration_seconds),
            scope: convert::scope(&self.scope)?,
            ended_at: self.ended_at,
            ended_reason: convert::end_reason(self.ended_reason.as_deref())?,
            source_summary: self.source_summary,
        })
    }
}

#[async_trait]
impl SessionRepo for PgTx {
    async fn insert(&mut self, session: &AuthSession) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO auth_sessions (id, user_id, token_hash, created_at, last_seen_at, idle_timeout_seconds,
                                        absolute_duration_seconds, scope, ended_at, ended_reason, source_summary)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            session.id.as_uuid(),
            session.user_id.as_uuid(),
            session.token_hash.as_str(),
            session.created_at,
            session.last_seen_at,
            session.idle_timeout.whole_seconds(),
            session.absolute_duration.whole_seconds(),
            session.scope.as_str(),
            session.ended_at,
            session.ended_reason.map(EndReason::as_str),
            session.source_summary,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn get(&mut self, id: AuthSessionId) -> Result<Option<AuthSession>, StoreError> {
        sqlx::query_as!(
            SessionRow,
            "SELECT id, user_id, token_hash, created_at, last_seen_at, idle_timeout_seconds,
                    absolute_duration_seconds, scope, ended_at, ended_reason, source_summary
             FROM auth_sessions WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(SessionRow::into_session)
        .transpose()
    }

    async fn find_by_token_hash(
        &mut self,
        hash: &TokenHash,
    ) -> Result<Option<AuthSession>, StoreError> {
        sqlx::query_as!(
            SessionRow,
            "SELECT id, user_id, token_hash, created_at, last_seen_at, idle_timeout_seconds,
                    absolute_duration_seconds, scope, ended_at, ended_reason, source_summary
             FROM auth_sessions WHERE token_hash = $1",
            hash.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?
        .map(SessionRow::into_session)
        .transpose()
    }

    async fn update(&mut self, session: &AuthSession) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE auth_sessions SET last_seen_at = $2, scope = $3, ended_at = $4, ended_reason = $5
             WHERE id = $1",
            session.id.as_uuid(),
            session.last_seen_at,
            session.scope.as_str(),
            session.ended_at,
            session.ended_reason.map(EndReason::as_str),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn end_all_for_user(
        &mut self,
        user_id: UserId,
        reason: EndReason,
        now: OffsetDateTime,
        except: Option<AuthSessionId>,
    ) -> Result<u64, StoreError> {
        let result = sqlx::query!(
            "UPDATE auth_sessions SET ended_at = $3, ended_reason = $2
             WHERE user_id = $1 AND ended_at IS NULL AND ($4::uuid IS NULL OR id <> $4)",
            user_id.as_uuid(),
            reason.as_str(),
            now,
            except.map(|id| id.as_uuid()),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl MfaRepo for PgTx {
    async fn get_credential(
        &mut self,
        user_id: UserId,
    ) -> Result<Option<MfaCredential>, StoreError> {
        let row = sqlx::query!(
            "SELECT user_id, secret_sealed, confirmed_at, last_used_step FROM mfa_credentials WHERE user_id = $1",
            user_id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| {
            Ok(MfaCredential {
                user_id: UserId::from_uuid(row.user_id),
                secret_sealed: row.secret_sealed,
                confirmed_at: row.confirmed_at,
                last_used_step: convert::step(row.last_used_step)?,
            })
        })
        .transpose()
    }

    async fn upsert_credential(&mut self, credential: &MfaCredential) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO mfa_credentials (user_id, secret_sealed, confirmed_at, last_used_step)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id) DO UPDATE
             SET secret_sealed = EXCLUDED.secret_sealed, confirmed_at = EXCLUDED.confirmed_at,
                 last_used_step = EXCLUDED.last_used_step",
            credential.user_id.as_uuid(),
            credential.secret_sealed,
            credential.confirmed_at,
            convert::step_to_stored(credential.last_used_step)?,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn delete_credential_and_codes(&mut self, user_id: UserId) -> Result<(), StoreError> {
        sqlx::query!(
            "DELETE FROM recovery_codes WHERE user_id = $1",
            user_id.as_uuid()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query!(
            "DELETE FROM mfa_credentials WHERE user_id = $1",
            user_id.as_uuid()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn replace_recovery_codes(
        &mut self,
        user_id: UserId,
        codes: &[RecoveryCode],
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "DELETE FROM recovery_codes WHERE user_id = $1",
            user_id.as_uuid()
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        for code in codes {
            sqlx::query!(
                "INSERT INTO recovery_codes (id, user_id, code_hash, used_at) VALUES ($1, $2, $3, $4)",
                code.id.as_uuid(),
                code.user_id.as_uuid(),
                code.code_hash.as_str(),
                code.used_at,
            )
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        }
        Ok(())
    }

    async fn find_recovery_code(
        &mut self,
        user_id: UserId,
        hash: &TokenHash,
    ) -> Result<Option<RecoveryCode>, StoreError> {
        let row = sqlx::query!(
            "SELECT id, user_id, code_hash, used_at FROM recovery_codes WHERE user_id = $1 AND code_hash = $2",
            user_id.as_uuid(),
            hash.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(|row| RecoveryCode {
            id: RecoveryCodeId::from_uuid(row.id),
            user_id: UserId::from_uuid(row.user_id),
            code_hash: TokenHash::from_stored(row.code_hash),
            used_at: row.used_at,
        }))
    }

    async fn update_recovery_code(&mut self, code: &RecoveryCode) -> Result<(), StoreError> {
        let result = sqlx::query!(
            "UPDATE recovery_codes SET used_at = $2 WHERE id = $1",
            code.id.as_uuid(),
            code.used_at
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn count_unused_recovery_codes(&mut self, user_id: UserId) -> Result<usize, StoreError> {
        let row = sqlx::query!(
            r#"SELECT count(*) AS "count!" FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL"#,
            user_id.as_uuid()
        )
        .fetch_one(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(usize::try_from(row.count).unwrap_or(0))
    }
}
