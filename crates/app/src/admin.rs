//! User administration: invitations, deactivation, roles (W2, W3, W9, W10; BR-5, BR-7).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::AppError;
use crate::sessions::{NewSession, scope_for};
use crate::validation::{fail_if_any, name_and_email};
use tandem_core::admin_rules::{AdminRuleError, check_can_lose_admin, check_not_self_deactivation};
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::email::Email;
use tandem_core::ids::{InvitationId, UserId};
use tandem_core::invitation::{Invitation, InvitationStatus};
use tandem_core::permissions::Action;
use tandem_core::role::{Role, UserStatus};
use tandem_core::session::EndReason;
use tandem_core::token::{generate_token, hash_token};
use tandem_core::user::{CreatedVia, User};

/// An invitation just issued. The token is shown once to the issuing admin (BR-5.3).
#[derive(Debug, Clone)]
pub struct InviteCreated {
    /// The stored invitation.
    pub invitation: Invitation,
    /// The link token (plaintext; only its hash is stored).
    pub token: String,
}

/// What a valid invitation link shows before the invitee sets a password.
#[derive(Debug, Clone)]
pub struct InvitationInfo {
    /// The invited address.
    pub email: String,
    /// The role they will receive.
    pub role: Role,
}

/// An invitation with its status as of now.
#[derive(Debug, Clone)]
pub struct InvitationView {
    /// The stored invitation.
    pub invitation: Invitation,
    /// Pending, accepted, revoked or expired as of now.
    pub status: InvitationStatus,
}

/// A user as the admin list shows them.
#[derive(Debug, Clone)]
pub struct UserView {
    /// The user.
    pub user: User,
    /// Whether they have confirmed MFA.
    pub mfa_enrolled: bool,
}

fn admin_rule_error(error: AdminRuleError) -> AppError {
    AppError::validation("user", error.to_string())
}

impl App {
    /// Invite someone by email and role (BR-5.1). The link is returned once; nothing is emailed.
    ///
    /// # Errors
    /// [`AppError::Validation`] for a bad or already-used email.
    pub async fn invite(
        &self,
        ctx: &RequestContext,
        email: &str,
        role: Role,
    ) -> Result<InviteCreated, AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let actor = ctx.actor()?;
        let email = Email::parse(email)
            .map_err(|error| AppError::validation("email", error.to_string()))?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        if tx.users().find_by_email(&email).await?.is_some() {
            return Err(AppError::validation(
                "email",
                "A user with this email already exists",
            ));
        }
        if tx
            .invitations()
            .find_pending_by_email(&email, now)
            .await?
            .is_some()
        {
            return Err(AppError::validation(
                "email",
                "An invitation for this email is already pending",
            ));
        }
        let (token, token_hash) = generate_token(&*self.rng)?;
        let invitation = Invitation::new(
            InvitationId::generate(&*self.clock, &*self.rng)?,
            email,
            role,
            token_hash,
            actor.user_id,
            now,
        );
        tx.invitations().insert(&invitation).await?;
        let event = self
            .audit_event(ctx, AuditEventType::UserInvited, AuditOutcome::Success)
            .object("invitation", invitation.id)
            .detail("role", role.as_str());
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(InviteCreated { invitation, token })
    }

    /// All invitations, newest first, with their status as of now.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn list_invitations(
        &self,
        ctx: &RequestContext,
    ) -> Result<Vec<InvitationView>, AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let invitations = tx.invitations().list().await?;
        Ok(invitations
            .into_iter()
            .map(|invitation| InvitationView {
                status: invitation.effective_status(now),
                invitation,
            })
            .collect())
    }

    /// Revoke a pending invitation (BR-5.6).
    ///
    /// # Errors
    /// [`AppError::NotFound`] for unknown invitations; [`AppError::Validation`] if not pending.
    pub async fn revoke_invitation(
        &self,
        ctx: &RequestContext,
        id: InvitationId,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut invitation = tx.invitations().get(id).await?.ok_or(AppError::NotFound)?;
        invitation
            .revoke(now)
            .map_err(|error| AppError::validation("invitation", error.to_string()))?;
        tx.invitations().update(&invitation).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::UserInviteRevoked,
                AuditOutcome::Success,
            )
            .object("invitation", invitation.id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Check an invitation link before showing the sign-up form.
    ///
    /// # Errors
    /// [`AppError::InvalidLink`] for any unusable link (BR-5.4).
    pub async fn validate_invitation(&self, token: &str) -> Result<InvitationInfo, AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let invitation = tx
            .invitations()
            .find_by_token_hash(&hash_token(token))
            .await?
            .ok_or(AppError::InvalidLink)?;
        if !invitation.is_usable(now) {
            return Err(AppError::InvalidLink);
        }
        Ok(InvitationInfo {
            email: invitation.email.as_str().to_owned(),
            role: invitation.role,
        })
    }

    /// Accept an invitation: create the account and sign the new user in (W3). Admins get an
    /// enrolment-only sign-in.
    ///
    /// # Errors
    /// [`AppError::InvalidLink`] for an unusable link; [`AppError::Validation`] for bad input.
    pub async fn accept_invitation(
        &self,
        ctx: &RequestContext,
        token: &str,
        display_name: &str,
        password: &str,
    ) -> Result<NewSession, AppError> {
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let mut invitation = tx
            .invitations()
            .find_by_token_hash(&hash_token(token))
            .await?
            .ok_or(AppError::InvalidLink)?;
        if !invitation.is_usable(now) {
            return Err(AppError::InvalidLink);
        }
        if tx.users().find_by_email(&invitation.email).await?.is_some() {
            return Err(AppError::InvalidLink);
        }
        let settings = Self::load_settings(tx.as_mut()).await?;
        let mut issues = Vec::new();
        let (name, _) = name_and_email(display_name, invitation.email.as_str(), &mut issues);
        issues.extend(
            self.password_issues(password, settings.online_breach_check, None)
                .await?,
        );
        fail_if_any(issues)?;
        let Some(display_name) = name else {
            return Err(AppError::Internal("validated name was missing".into()));
        };
        let password_hash = self.hasher.hash(password).await?;
        let user = User {
            id: UserId::generate(&*self.clock, &*self.rng)?,
            email: invitation.email.clone(),
            display_name,
            role: invitation.role,
            status: UserStatus::Active,
            password_hash,
            password_changed_at: now,
            created_at: now,
            created_via: CreatedVia::Invitation,
            deactivated_at: None,
            deactivated_by: None,
            last_sign_in_at: None,
        };
        tx.users().insert(&user).await?;
        invitation
            .accept(user.id, now)
            .map_err(|_| AppError::InvalidLink)?;
        tx.invitations().update(&invitation).await?;
        let event = self
            .audit_event(
                ctx,
                AuditEventType::UserInviteAccepted,
                AuditOutcome::Success,
            )
            .actor(user.id, user.display_name.as_str())
            .object("invitation", invitation.id)
            .detail("role", user.role.as_str());
        self.append_audit(tx.as_mut(), event).await?;
        let session = self
            .start_session(tx.as_mut(), &user, scope_for(user.role, false), ctx)
            .await?;
        tx.commit().await?;
        Ok(session)
    }

    /// All users, oldest first, with their MFA state.
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn list_users(&self, ctx: &RequestContext) -> Result<Vec<UserView>, AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let mut tx = self.store.begin().await?;
        let users = tx.users().list().await?;
        let mut views = Vec::with_capacity(users.len());
        for user in users {
            let mfa_enrolled = tx
                .mfa()
                .get_credential(user.id)
                .await?
                .is_some_and(|credential| credential.is_confirmed());
            views.push(UserView { user, mfa_enrolled });
        }
        Ok(views)
    }

    /// Deactivate a user: their sign-ins end and pending reset links stop working (W9).
    ///
    /// # Errors
    /// [`AppError::Validation`] for the last admin, for oneself, or if already deactivated.
    pub async fn deactivate_user(
        &self,
        ctx: &RequestContext,
        target: UserId,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let actor = ctx.actor()?;
        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        tx.lock_admin_changes().await?;
        let mut user = tx.users().get(target).await?.ok_or(AppError::NotFound)?;
        check_not_self_deactivation(actor.user_id, user.id).map_err(admin_rule_error)?;
        let admins = tx.users().count_active_admins().await?;
        check_can_lose_admin(&user, admins).map_err(admin_rule_error)?;
        user.deactivate(actor.user_id, now)
            .map_err(|error| AppError::validation("user", error.to_string()))?;
        tx.users().update(&user).await?;
        tx.sessions()
            .end_all_for_user(user.id, EndReason::Deactivated, now, None)
            .await?;
        tx.resets().invalidate_unused_for_user(user.id, now).await?;
        let event = self
            .audit_event(ctx, AuditEventType::UserDeactivated, AuditOutcome::Success)
            .object("user", user.id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Reactivate a user. They sign in again with their existing password and MFA (BR-7.4).
    ///
    /// # Errors
    /// [`AppError::Validation`] if the user is already active.
    pub async fn reactivate_user(
        &self,
        ctx: &RequestContext,
        target: UserId,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let mut tx = self.store.begin().await?;
        let mut user = tx.users().get(target).await?.ok_or(AppError::NotFound)?;
        user.reactivate()
            .map_err(|error| AppError::validation("user", error.to_string()))?;
        tx.users().update(&user).await?;
        let event = self
            .audit_event(ctx, AuditEventType::UserReactivated, AuditOutcome::Success)
            .object("user", user.id);
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Change a user's role. The last admin cannot be demoted (BR-7.2).
    ///
    /// # Errors
    /// [`AppError::Validation`] for the last admin or an unchanged role.
    pub async fn change_user_role(
        &self,
        ctx: &RequestContext,
        target: UserId,
        role: Role,
    ) -> Result<(), AppError> {
        self.authorize(ctx, Action::ManageUsers).await?;
        let mut tx = self.store.begin().await?;
        tx.lock_admin_changes().await?;
        let mut user = tx.users().get(target).await?.ok_or(AppError::NotFound)?;
        let old_role = user.role;
        if role == Role::Member {
            let admins = tx.users().count_active_admins().await?;
            check_can_lose_admin(&user, admins).map_err(admin_rule_error)?;
        }
        user.change_role(role)
            .map_err(|error| AppError::validation("role", error.to_string()))?;
        tx.users().update(&user).await?;
        let event = self
            .audit_event(ctx, AuditEventType::UserRoleChanged, AuditOutcome::Success)
            .object("user", user.id)
            .detail("old_role", old_role.as_str())
            .detail("new_role", role.as_str());
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        Ok(())
    }
}
