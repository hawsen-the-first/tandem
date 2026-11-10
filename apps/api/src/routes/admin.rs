//! Administration: users, invitations, settings and the audit log (US-1.3, 1.4, 1.7 to 1.9, 2.7).
//! Every route requires an admin with a full-scope sign-in; others are told the route is absent.

use crate::dto::{
    AdminUserDto, AuditEntryDto, AuditPageDto, InvitationDto, InviteCreatedDto, InviteRequest,
    ResetLinkDto, ResetLinkRequest, RoleRequest, SecuritySettingsDto, timestamp,
};
use crate::error::{ApiError, ErrorEnvelope};
use crate::pipeline::Ctx;
use crate::pipeline::access::AdminUser;
use crate::pipeline::rate_limit::Bucket;
use crate::routes::{ApiJson, ApiQuery, RouteTable, parse_id};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode};
use serde::Deserialize;
use tandem_app::AppError;
use tandem_core::audit::AuditEventType;
use tandem_core::ids::{AuditEntryId, InvitationId, UserId};
use tandem_core::role::Role;
use tandem_ports::{AuditCursor, AuditFilter};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};

const DEFAULT_PAGE: usize = 50;
const MAX_PAGE: usize = 200;

pub(super) fn register(table: RouteTable) -> RouteTable {
    let general = Bucket::General;
    table
        .protected::<AdminUser, _, _>(Method::GET, "/api/admin/users", general, list_users)
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/users/{id}/deactivate",
            general,
            deactivate_user,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/users/{id}/reactivate",
            general,
            reactivate_user,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/users/{id}/role",
            general,
            change_role,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/users/{id}/reset-link",
            general,
            issue_reset_link,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/invitations",
            general,
            list_invitations,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/invitations",
            general,
            create_invitation,
        )
        .protected::<AdminUser, _, _>(
            Method::DELETE,
            "/api/admin/invitations/{id}",
            general,
            revoke_invitation,
        )
        .protected::<AdminUser, _, _>(Method::GET, "/api/admin/settings", general, get_settings)
        .protected::<AdminUser, _, _>(Method::PUT, "/api/admin/settings", general, put_settings)
        .protected::<AdminUser, _, _>(Method::GET, "/api/admin/audit", general, audit)
}

fn parse_role(text: &str) -> Result<Role, ApiError> {
    Role::parse(text)
        .ok_or_else(|| AppError::validation("role", "The role must be `admin` or `member`.").into())
}

/// List users.
#[utoipa::path(get, path = "/api/admin/users", tag = "admin",
    responses((status = 200, body = Vec<AdminUserDto>), (status = 404, body = ErrorEnvelope)))]
pub async fn list_users(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<Vec<AdminUserDto>>, ApiError> {
    let users = state.app.list_users(&ctx).await?;
    Ok(Json(users.iter().map(AdminUserDto::from).collect()))
}

/// Deactivate a user and end their sign-ins.
#[utoipa::path(post, path = "/api/admin/users/{id}/deactivate", tag = "admin",
    params(("id" = String, Path, description = "User ID")),
    responses((status = 204), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn deactivate_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .deactivate_user(&ctx, parse_id::<UserId>(&id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Reactivate a user.
#[utoipa::path(post, path = "/api/admin/users/{id}/reactivate", tag = "admin",
    params(("id" = String, Path, description = "User ID")),
    responses((status = 204), (status = 404, body = ErrorEnvelope)))]
pub async fn reactivate_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .reactivate_user(&ctx, parse_id::<UserId>(&id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Change a user's role.
#[utoipa::path(put, path = "/api/admin/users/{id}/role", tag = "admin", request_body = RoleRequest,
    params(("id" = String, Path, description = "User ID")),
    responses((status = 204), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn change_role(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ApiJson(request): ApiJson<RoleRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .change_user_role(&ctx, parse_id::<UserId>(&id)?, parse_role(&request.role)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Issue a one-time password-reset link for a user.
#[utoipa::path(post, path = "/api/admin/users/{id}/reset-link", tag = "admin", request_body = ResetLinkRequest,
    params(("id" = String, Path, description = "User ID")),
    responses((status = 200, body = ResetLinkDto), (status = 404, body = ErrorEnvelope)))]
pub async fn issue_reset_link(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ApiJson(request): ApiJson<ResetLinkRequest>,
) -> Result<Json<ResetLinkDto>, ApiError> {
    let link = state
        .app
        .issue_reset_link(&ctx, parse_id::<UserId>(&id)?, request.reset_mfa)
        .await?;
    Ok(Json(ResetLinkDto {
        link: format!(
            "{}/reset-password?token={}",
            state.config.public_origin(),
            link.token
        ),
        expires_at: timestamp(link.expires_at),
        user_display_name: link.user_display_name,
    }))
}

/// List invitations.
#[utoipa::path(get, path = "/api/admin/invitations", tag = "admin",
    responses((status = 200, body = Vec<InvitationDto>), (status = 404, body = ErrorEnvelope)))]
pub async fn list_invitations(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<Vec<InvitationDto>>, ApiError> {
    let invitations = state.app.list_invitations(&ctx).await?;
    Ok(Json(invitations.iter().map(InvitationDto::from).collect()))
}

/// Invite someone. Nothing is emailed; the link is returned once.
#[utoipa::path(post, path = "/api/admin/invitations", tag = "admin", request_body = InviteRequest,
    responses((status = 201, body = InviteCreatedDto), (status = 422, body = ErrorEnvelope)))]
pub async fn create_invitation(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<InviteRequest>,
) -> Result<(StatusCode, Json<InviteCreatedDto>), ApiError> {
    let created = state
        .app
        .invite(&ctx, &request.email, parse_role(&request.role)?)
        .await?;
    let view = tandem_app::InvitationView {
        status: created.invitation.status,
        invitation: created.invitation,
    };
    Ok((
        StatusCode::CREATED,
        Json(InviteCreatedDto {
            invitation: InvitationDto::from(&view),
            link: format!(
                "{}/accept-invitation?token={}",
                state.config.public_origin(),
                created.token
            ),
        }),
    ))
}

/// Revoke a pending invitation.
#[utoipa::path(delete, path = "/api/admin/invitations/{id}", tag = "admin",
    params(("id" = String, Path, description = "Invitation ID")),
    responses((status = 204), (status = 404, body = ErrorEnvelope)))]
pub async fn revoke_invitation(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .revoke_invitation(&ctx, parse_id::<InvitationId>(&id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The instance security settings.
#[utoipa::path(get, path = "/api/admin/settings", tag = "admin",
    responses((status = 200, body = SecuritySettingsDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_settings(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<SecuritySettingsDto>, ApiError> {
    Ok(Json(state.app.get_security_settings(&ctx).await?.into()))
}

/// Replace the instance security settings.
#[utoipa::path(put, path = "/api/admin/settings", tag = "admin", request_body = SecuritySettingsDto,
    responses((status = 200, body = SecuritySettingsDto), (status = 422, body = ErrorEnvelope)))]
pub async fn put_settings(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<SecuritySettingsDto>,
) -> Result<Json<SecuritySettingsDto>, ApiError> {
    let saved = state
        .app
        .update_security_settings(&ctx, request.into_settings())
        .await?;
    Ok(Json(saved.into()))
}

/// Audit log filters and paging.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditQueryParams {
    /// Only this actor.
    pub actor_id: Option<String>,
    /// Only this event type, for example `auth.sign_in.failure`.
    pub event_type: Option<String>,
    /// Not before (RFC 3339).
    pub from: Option<String>,
    /// Not after (RFC 3339).
    pub to: Option<String>,
    /// Page cursor: the `next_after_time` of the previous page.
    pub after_time: Option<String>,
    /// Page cursor: the `next_after_id` of the previous page.
    pub after_id: Option<String>,
    /// Page size (default 50, at most 200).
    pub limit: Option<usize>,
}

fn rfc3339(field: &'static str, text: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| {
        AppError::validation(field, "Use a date and time like 2026-01-31T09:30:00Z.").into()
    })
}

/// Query the audit log, newest first.
#[utoipa::path(get, path = "/api/admin/audit", tag = "admin", params(AuditQueryParams),
    responses((status = 200, body = AuditPageDto), (status = 422, body = ErrorEnvelope)))]
pub async fn audit(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiQuery(query): ApiQuery<AuditQueryParams>,
) -> Result<Json<AuditPageDto>, ApiError> {
    let filter = AuditFilter {
        actor_id: query
            .actor_id
            .as_deref()
            .map(|text| {
                text.parse::<UserId>()
                    .map_err(|_| AppError::validation("actor_id", "That is not a user ID."))
            })
            .transpose()?,
        event_type: query
            .event_type
            .as_deref()
            .map(|text| {
                AuditEventType::parse(text).ok_or_else(|| {
                    AppError::validation("event_type", "That event type does not exist.")
                })
            })
            .transpose()?,
        from: query
            .from
            .as_deref()
            .map(|text| rfc3339("from", text))
            .transpose()?,
        to: query
            .to
            .as_deref()
            .map(|text| rfc3339("to", text))
            .transpose()?,
    };
    let after = match (&query.after_time, &query.after_id) {
        (Some(time), Some(id)) => Some(AuditCursor {
            occurred_at: rfc3339("after_time", time)?,
            id: id
                .parse::<AuditEntryId>()
                .map_err(|_| AppError::validation("after_id", "That is not an entry ID."))?,
        }),
        (None, None) => None,
        _ => {
            return Err(
                AppError::validation("after_id", "Send after_time and after_id together.").into(),
            );
        }
    };
    let limit = query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let page = state.app.query_audit(&ctx, filter, after, limit).await?;
    Ok(Json(AuditPageDto {
        entries: page.entries.iter().map(AuditEntryDto::from).collect(),
        next_after_time: page
            .next
            .as_ref()
            .map(|cursor| timestamp(cursor.occurred_at)),
        next_after_id: page.next.as_ref().map(|cursor| cursor.id.to_string()),
    }))
}
