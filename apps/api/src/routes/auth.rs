//! Setup, sign-in, MFA, password and link endpoints (US-1.1 to 1.7, 13.3).

use crate::cookie;
use crate::dto::{
    AcceptInvitationRequest, ChangePasswordRequest, CodeRequest, CompleteResetRequest,
    DisableMfaRequest, EnrolmentResponse, InvitationInfoDto, MeResponse, RecoveryCodesResponse,
    ResetInfoDto, SecondFactorDto, SessionStarted, SetupRequest, SetupStatus, SignInRequestDto,
    SignInResponse, TokenRequest, UserDto,
};
use crate::error::{ApiError, ErrorEnvelope};
use crate::pipeline::access::{AnyUser, FullScopeUser};
use crate::pipeline::csrf::csrf_token_for;
use crate::pipeline::rate_limit::Bucket;
use crate::pipeline::{Access, Ctx};
use crate::routes::{ApiJson, RouteTable};
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tandem_app::{AppError, NewSession, SecondFactor, SetupInput, SignInInput, SignInOutcome};
use tandem_telemetry::metric_names;

pub(super) fn register(table: RouteTable) -> RouteTable {
    table
        .public(Method::GET, "/api/setup", Bucket::Setup, setup_status)
        .public(Method::POST, "/api/setup", Bucket::Setup, setup)
        .public(Method::POST, "/api/auth/sign-in", Bucket::Auth, sign_in)
        .protected::<AnyUser, _, _>(
            Method::POST,
            "/api/auth/sign-out",
            Bucket::General,
            sign_out,
        )
        .protected::<AnyUser, _, _>(Method::GET, "/api/auth/me", Bucket::General, me)
        .protected::<AnyUser, _, _>(
            Method::POST,
            "/api/auth/mfa/enrol",
            Bucket::General,
            mfa_enrol,
        )
        .protected::<AnyUser, _, _>(
            Method::POST,
            "/api/auth/mfa/confirm",
            Bucket::Auth,
            mfa_confirm,
        )
        .protected::<FullScopeUser, _, _>(
            Method::POST,
            "/api/auth/mfa/recovery-codes",
            Bucket::General,
            mfa_recovery_codes,
        )
        .protected::<FullScopeUser, _, _>(
            Method::POST,
            "/api/auth/mfa/disable",
            Bucket::Auth,
            mfa_disable,
        )
        .protected::<FullScopeUser, _, _>(
            Method::POST,
            "/api/auth/password",
            Bucket::Auth,
            change_password,
        )
        .public(
            Method::POST,
            "/api/invitations/validate",
            Bucket::Auth,
            validate_invitation,
        )
        .public(
            Method::POST,
            "/api/invitations/accept",
            Bucket::Auth,
            accept_invitation,
        )
        .public(
            Method::POST,
            "/api/password-resets/validate",
            Bucket::Auth,
            validate_reset,
        )
        .public(
            Method::POST,
            "/api/password-resets/complete",
            Bucket::Auth,
            complete_reset,
        )
}

fn started(session: &NewSession) -> Result<Response, ApiError> {
    let cookie = cookie::set_session(&session.token).ok_or_else(ApiError::internal)?;
    let body = SessionStarted {
        user: UserDto::from(&session.user),
        scope: session.scope.as_str().to_owned(),
    };
    Ok(([(header::SET_COOKIE, cookie)], Json(body)).into_response())
}

fn count_sign_in(outcome: &'static str) {
    tandem_telemetry::metrics::count_outcome(metric_names::AUTH_SIGN_IN_TOTAL, outcome);
}

/// Whether first-run setup is still open.
#[utoipa::path(get, path = "/api/setup", tag = "setup",
    responses((status = 200, body = SetupStatus)))]
pub async fn setup_status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    Ok(Json(SetupStatus {
        available: state.app.setup_available().await?,
    }))
}

/// Create the first admin and sign in with an enrolment-only scope.
#[utoipa::path(post, path = "/api/setup", tag = "setup", request_body = SetupRequest,
    responses((status = 200, body = SessionStarted), (status = 404, body = ErrorEnvelope),
              (status = 422, body = ErrorEnvelope)))]
pub async fn setup(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<SetupRequest>,
) -> Result<Response, ApiError> {
    let session = state
        .app
        .first_run_setup(
            &ctx,
            SetupInput {
                display_name: request.display_name,
                email: request.email,
                password: request.password,
            },
        )
        .await?;
    started(&session)
}

fn second_factor(input: Option<SecondFactorDto>) -> Result<Option<SecondFactor>, ApiError> {
    match input {
        None => Ok(None),
        Some(factor) => match factor.kind.as_str() {
            "totp" => Ok(Some(SecondFactor::Totp(factor.code))),
            "recovery" => Ok(Some(SecondFactor::Recovery(factor.code))),
            _ => Err(ApiError::bad_request(
                "The second factor must be `totp` or `recovery`.",
            )),
        },
    }
}

/// Sign in with email, password and, when enrolled, a second factor.
#[utoipa::path(post, path = "/api/auth/sign-in", tag = "auth", request_body = SignInRequestDto,
    responses((status = 200, body = SignInResponse), (status = 401, body = ErrorEnvelope),
              (status = 429, body = ErrorEnvelope), (status = 503, body = ErrorEnvelope)))]
pub async fn sign_in(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<SignInRequestDto>,
) -> Result<Response, ApiError> {
    let input = SignInInput {
        email: request.email,
        password: request.password,
        second_factor: second_factor(request.second_factor)?,
    };
    match state.app.sign_in(&ctx, input).await {
        Ok(SignInOutcome::Success(session)) => {
            count_sign_in("success");
            let cookie = cookie::set_session(&session.token).ok_or_else(ApiError::internal)?;
            let body = SignInResponse {
                status: "signed_in".to_owned(),
                user: Some(UserDto::from(&session.user)),
                scope: Some(session.scope.as_str().to_owned()),
            };
            Ok(([(header::SET_COOKIE, cookie)], Json(body)).into_response())
        }
        Ok(SignInOutcome::MfaRequired) => {
            count_sign_in("mfa_required");
            Ok(Json(SignInResponse {
                status: "mfa_required".to_owned(),
                user: None,
                scope: None,
            })
            .into_response())
        }
        Err(error) => {
            count_sign_in(match &error {
                AppError::InvalidCredentials => "bad_credentials",
                AppError::Refused { .. } => "locked",
                AppError::Busy => "busy",
                _ => "error",
            });
            Err(error.into())
        }
    }
}

/// End the current sign-in.
#[utoipa::path(post, path = "/api/auth/sign-out", tag = "auth",
    responses((status = 204), (status = 401, body = ErrorEnvelope)))]
pub async fn sign_out(
    State(state): State<AppState>,
    _user: AnyUser,
    Ctx(ctx): Ctx,
) -> Result<Response, ApiError> {
    state.app.sign_out(&ctx).await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookie::clear_session())],
    )
        .into_response())
}

/// The signed-in user, their scope and the CSRF token.
#[utoipa::path(get, path = "/api/auth/me", tag = "auth",
    responses((status = 200, body = MeResponse), (status = 401, body = ErrorEnvelope)))]
pub async fn me(
    State(state): State<AppState>,
    user: AnyUser,
    Ctx(ctx): Ctx,
) -> Result<Json<MeResponse>, ApiError> {
    let me = state.app.current_user(&ctx).await?;
    Ok(Json(MeResponse {
        user: UserDto::from(&me.user),
        scope: me.scope.as_str().to_owned(),
        mfa: me.mfa.into(),
        csrf_token: csrf_token_for(&user.session().token),
    }))
}

/// Begin MFA enrolment: returns the secret to show once.
#[utoipa::path(post, path = "/api/auth/mfa/enrol", tag = "mfa",
    responses((status = 200, body = EnrolmentResponse), (status = 401, body = ErrorEnvelope)))]
pub async fn mfa_enrol(
    State(state): State<AppState>,
    _user: AnyUser,
    Ctx(ctx): Ctx,
) -> Result<Json<EnrolmentResponse>, ApiError> {
    let enrolment = state.app.mfa_start_enrolment(&ctx).await?;
    Ok(Json(EnrolmentResponse {
        secret_base32: enrolment.secret_base32,
        otpauth_uri: enrolment.otpauth_uri,
    }))
}

/// Confirm enrolment with a code; returns the recovery codes once.
#[utoipa::path(post, path = "/api/auth/mfa/confirm", tag = "mfa", request_body = CodeRequest,
    responses((status = 200, body = RecoveryCodesResponse), (status = 422, body = ErrorEnvelope)))]
pub async fn mfa_confirm(
    State(state): State<AppState>,
    _user: AnyUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<CodeRequest>,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    let recovery_codes = state.app.mfa_confirm_enrolment(&ctx, &request.code).await?;
    Ok(Json(RecoveryCodesResponse { recovery_codes }))
}

/// Replace the recovery codes; returns the new set once.
#[utoipa::path(post, path = "/api/auth/mfa/recovery-codes", tag = "mfa",
    responses((status = 200, body = RecoveryCodesResponse), (status = 403, body = ErrorEnvelope)))]
pub async fn mfa_recovery_codes(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
) -> Result<Json<RecoveryCodesResponse>, ApiError> {
    Ok(Json(RecoveryCodesResponse {
        recovery_codes: state.app.mfa_regenerate_recovery_codes(&ctx).await?,
    }))
}

/// Turn MFA off (members only).
#[utoipa::path(post, path = "/api/auth/mfa/disable", tag = "mfa", request_body = DisableMfaRequest,
    responses((status = 204), (status = 403, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn mfa_disable(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<DisableMfaRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .mfa_disable(&ctx, &request.password, &request.code)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Change the signed-in user's password.
#[utoipa::path(post, path = "/api/auth/password", tag = "auth", request_body = ChangePasswordRequest,
    responses((status = 204), (status = 422, body = ErrorEnvelope)))]
pub async fn change_password(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .change_password(&ctx, &request.current_password, &request.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Check an invitation link.
#[utoipa::path(post, path = "/api/invitations/validate", tag = "invitations", request_body = TokenRequest,
    responses((status = 200, body = InvitationInfoDto), (status = 404, body = ErrorEnvelope)))]
pub async fn validate_invitation(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<TokenRequest>,
) -> Result<Json<InvitationInfoDto>, ApiError> {
    let info = state.app.validate_invitation(&request.token).await?;
    Ok(Json(InvitationInfoDto {
        email: info.email,
        role: info.role.as_str().to_owned(),
    }))
}

/// Accept an invitation: creates the account and signs in.
#[utoipa::path(post, path = "/api/invitations/accept", tag = "invitations", request_body = AcceptInvitationRequest,
    responses((status = 200, body = SessionStarted), (status = 404, body = ErrorEnvelope),
              (status = 422, body = ErrorEnvelope)))]
pub async fn accept_invitation(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<AcceptInvitationRequest>,
) -> Result<Response, ApiError> {
    let session = state
        .app
        .accept_invitation(
            &ctx,
            &request.token,
            &request.display_name,
            &request.password,
        )
        .await?;
    started(&session)
}

/// Check a password-reset link.
#[utoipa::path(post, path = "/api/password-resets/validate", tag = "password-resets", request_body = TokenRequest,
    responses((status = 200, body = ResetInfoDto), (status = 404, body = ErrorEnvelope)))]
pub async fn validate_reset(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<TokenRequest>,
) -> Result<Json<ResetInfoDto>, ApiError> {
    let info = state.app.validate_reset_link(&request.token).await?;
    Ok(Json(ResetInfoDto {
        display_name: info.display_name,
    }))
}

/// Finish a password reset.
#[utoipa::path(post, path = "/api/password-resets/complete", tag = "password-resets", request_body = CompleteResetRequest,
    responses((status = 204), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn complete_reset(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<CompleteResetRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .app
        .complete_reset(&ctx, &request.token, &request.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
