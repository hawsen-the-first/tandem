//! The OpenAPI description (MT-3), generated from the handlers' annotations and served at
//! `/api/openapi.json`. A copy is committed as `openapi.json` and a test keeps it current.

use crate::pipeline::rate_limit::Bucket;
use crate::routes::{RouteTable, about, admin, auth, chat, integrations};
use axum::Json;
use axum::http::Method;
use utoipa::OpenApi;

/// The API description.
#[derive(Debug, OpenApi)]
#[openapi(
    info(
        title = "Tandem API",
        description = "Authentication, administration, instance integrations and the chat skeleton. Cookie-authenticated; \
            state-changing calls need the X-CSRF-Token header returned by GET /api/auth/me.",
        license(name = "AGPL-3.0-only")
    ),
    paths(
        about::about,
        auth::setup_status,
        auth::setup,
        auth::sign_in,
        auth::sign_out,
        auth::me,
        auth::mfa_enrol,
        auth::mfa_confirm,
        auth::mfa_recovery_codes,
        auth::mfa_disable,
        auth::change_password,
        auth::validate_invitation,
        auth::accept_invitation,
        auth::validate_reset,
        auth::complete_reset,
        admin::list_users,
        admin::deactivate_user,
        admin::reactivate_user,
        admin::change_role,
        admin::issue_reset_link,
        admin::list_invitations,
        admin::create_invitation,
        admin::revoke_invitation,
        admin::get_settings,
        admin::put_settings,
        admin::audit,
        integrations::list_providers,
        integrations::create_provider,
        integrations::get_provider,
        integrations::update_provider,
        integrations::delete_provider,
        integrations::removal_impact,
        integrations::test_provider,
        integrations::get_defaults,
        integrations::put_defaults,
        integrations::get_search,
        integrations::put_search_kind,
        integrations::test_search,
        integrations::put_active_search,
        integrations::get_connector,
        integrations::put_connector,
        integrations::delete_connector,
        integrations::test_connector,
        integrations::get_policy,
        integrations::put_policy,
        integrations::check_runtime,
        integrations::get_test_run,
        chat::list_sessions,
        chat::create_session,
        chat::list_messages,
        chat::post_message,
    )
)]
pub struct ApiDoc;

/// The description as a document.
pub fn document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// The description as pretty JSON (what is committed).
///
/// # Errors
/// [`serde_json::Error`] if serialisation fails (it cannot for this document).
pub fn document_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&document())
}

pub(super) fn register(table: RouteTable) -> RouteTable {
    table.public(Method::GET, "/api/openapi.json", Bucket::General, serve)
}

async fn serve() -> Json<utoipa::openapi::OpenApi> {
    Json(document())
}
