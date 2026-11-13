//! Instance administration (U2): providers, default models, search, the GitHub connector, the
//! sandbox policy and configuration tests (US-2.1, 2.3 to 2.6, 2.9). Every route needs an admin
//! with a full-scope sign-in; others are told the route does not exist (BR-21.3). No response
//! carries a secret (BR-20.2).

use crate::dto_integrations::{
    ActivationDto, ActiveSearchRequest, ConnectorDto, ConnectorRequest, DefaultsDto,
    DefaultsRequest, PolicyRequest, PolicyViewDto, ProviderDto, ProviderRequest, RemovalImpactDto,
    SearchOverviewDto, SearchRequest, TestRunDto,
};
use crate::error::{ApiError, ErrorEnvelope};
use crate::pipeline::Ctx;
use crate::pipeline::access::AdminUser;
use crate::pipeline::rate_limit::Bucket;
use crate::routes::{ApiJson, RouteTable, parse_id};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode};
use tandem_app::AppError;
use tandem_core::config::{ConnectorKind, SearchKind, TestTarget};
use tandem_core::ids::{ProviderId, TestRunId};

pub(super) fn register(table: RouteTable) -> RouteTable {
    let (general, test) = (Bucket::General, Bucket::Test);
    table
        .protected::<AdminUser, _, _>(Method::GET, "/api/admin/providers", general, list_providers)
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/providers",
            general,
            create_provider,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/providers/{id}",
            general,
            get_provider,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/providers/{id}",
            general,
            update_provider,
        )
        .protected::<AdminUser, _, _>(
            Method::DELETE,
            "/api/admin/providers/{id}",
            general,
            delete_provider,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/providers/{id}/removal-impact",
            general,
            removal_impact,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/providers/{id}/test",
            test,
            test_provider,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/model-defaults",
            general,
            get_defaults,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/model-defaults",
            general,
            put_defaults,
        )
        .protected::<AdminUser, _, _>(Method::GET, "/api/admin/search", general, get_search)
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/search/kinds/{kind}",
            general,
            put_search_kind,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/search/kinds/{kind}/test",
            test,
            test_search,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/search/active",
            general,
            put_active_search,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/connectors/github",
            general,
            get_connector,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/connectors/github",
            general,
            put_connector,
        )
        .protected::<AdminUser, _, _>(
            Method::DELETE,
            "/api/admin/connectors/github",
            general,
            delete_connector,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/connectors/github/test",
            test,
            test_connector,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/sandbox-policy",
            general,
            get_policy,
        )
        .protected::<AdminUser, _, _>(
            Method::PUT,
            "/api/admin/sandbox-policy",
            general,
            put_policy,
        )
        .protected::<AdminUser, _, _>(
            Method::POST,
            "/api/admin/runtime/check",
            test,
            check_runtime,
        )
        .protected::<AdminUser, _, _>(
            Method::GET,
            "/api/admin/test-runs/{id}",
            general,
            get_test_run,
        )
}

fn parse_search_kind(text: &str) -> Result<SearchKind, ApiError> {
    SearchKind::parse(text).ok_or_else(ApiError::not_found)
}

// ---- providers ----------------------------------------------------------------------------

/// List model providers with their models and last test.
#[utoipa::path(get, path = "/api/admin/providers", tag = "integrations",
    responses((status = 200, body = Vec<ProviderDto>), (status = 404, body = ErrorEnvelope)))]
pub async fn list_providers(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<Vec<ProviderDto>>, ApiError> {
    let views = state.app.list_providers(&ctx).await?;
    Ok(Json(views.iter().map(ProviderDto::from).collect()))
}

/// Add a provider.
#[utoipa::path(post, path = "/api/admin/providers", tag = "integrations", request_body = ProviderRequest,
    responses((status = 201, body = ProviderDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn create_provider(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(body): ApiJson<ProviderRequest>,
) -> Result<(StatusCode, Json<ProviderDto>), ApiError> {
    let view = state.app.create_provider(&ctx, body.into_input()?).await?;
    Ok((StatusCode::CREATED, Json(ProviderDto::from(&view))))
}

/// One provider.
#[utoipa::path(get, path = "/api/admin/providers/{id}", tag = "integrations",
    params(("id" = String, Path, description = "Provider ID")),
    responses((status = 200, body = ProviderDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_provider(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Json<ProviderDto>, ApiError> {
    let view = state
        .app
        .get_provider(&ctx, parse_id::<ProviderId>(&id)?)
        .await?;
    Ok(Json(ProviderDto::from(&view)))
}

/// Change a provider. An empty `api_key` keeps the saved key; `remove_api_key` deletes it.
#[utoipa::path(put, path = "/api/admin/providers/{id}", tag = "integrations", request_body = ProviderRequest,
    params(("id" = String, Path, description = "Provider ID")),
    responses((status = 200, body = ProviderDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn update_provider(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ProviderRequest>,
) -> Result<Json<ProviderDto>, ApiError> {
    let view = state
        .app
        .update_provider(&ctx, parse_id::<ProviderId>(&id)?, body.into_input()?)
        .await?;
    Ok(Json(ProviderDto::from(&view)))
}

/// What removing a provider would do, for the confirmation.
#[utoipa::path(get, path = "/api/admin/providers/{id}/removal-impact", tag = "integrations",
    params(("id" = String, Path, description = "Provider ID")),
    responses((status = 200, body = RemovalImpactDto), (status = 404, body = ErrorEnvelope)))]
pub async fn removal_impact(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Json<RemovalImpactDto>, ApiError> {
    let impact = state
        .app
        .provider_removal_impact(&ctx, parse_id::<ProviderId>(&id)?)
        .await?;
    Ok(Json(RemovalImpactDto::from(&impact)))
}

/// Remove a provider, its models and key, clearing the defaults that used them.
#[utoipa::path(delete, path = "/api/admin/providers/{id}", tag = "integrations",
    params(("id" = String, Path, description = "Provider ID")),
    responses((status = 200, body = RemovalImpactDto), (status = 404, body = ErrorEnvelope)))]
pub async fn delete_provider(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Json<RemovalImpactDto>, ApiError> {
    let impact = state
        .app
        .delete_provider(&ctx, parse_id::<ProviderId>(&id)?)
        .await?;
    Ok(Json(RemovalImpactDto::from(&impact)))
}

/// Test a provider and each of its models. Runs in the background: poll the run.
#[utoipa::path(post, path = "/api/admin/providers/{id}/test", tag = "integrations",
    params(("id" = String, Path, description = "Provider ID")),
    responses((status = 202, body = TestRunDto), (status = 404, body = ErrorEnvelope),
        (status = 409, description = "A test is already running; the body carries its run_id", body = ErrorEnvelope),
        (status = 429, body = ErrorEnvelope)))]
pub async fn test_provider(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<TestRunDto>), ApiError> {
    let run = state
        .app
        .start_test(&ctx, TestTarget::Provider(parse_id::<ProviderId>(&id)?))
        .await?;
    Ok((StatusCode::ACCEPTED, Json(TestRunDto::from(&run))))
}

// ---- default models -----------------------------------------------------------------------

/// The default models, with where each comes from and any warnings.
#[utoipa::path(get, path = "/api/admin/model-defaults", tag = "integrations",
    responses((status = 200, body = DefaultsDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_defaults(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<DefaultsDto>, ApiError> {
    Ok(Json(DefaultsDto::from(
        &state.app.get_defaults(&ctx).await?,
    )))
}

/// Choose the default models. A model with warnings is allowed; the warnings come back.
#[utoipa::path(put, path = "/api/admin/model-defaults", tag = "integrations", request_body = DefaultsRequest,
    responses((status = 200, body = DefaultsDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn put_defaults(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(body): ApiJson<DefaultsRequest>,
) -> Result<Json<DefaultsDto>, ApiError> {
    let view = state.app.set_defaults(&ctx, body.into_input()?).await?;
    Ok(Json(DefaultsDto::from(&view)))
}

// ---- search -------------------------------------------------------------------------------

/// All search providers and which one is active.
#[utoipa::path(get, path = "/api/admin/search", tag = "integrations",
    responses((status = 200, body = SearchOverviewDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_search(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<SearchOverviewDto>, ApiError> {
    Ok(Json(SearchOverviewDto::from(
        &state.app.get_search(&ctx).await?,
    )))
}

/// Save one search provider's settings; the overview comes back.
#[utoipa::path(put, path = "/api/admin/search/kinds/{kind}", tag = "integrations", request_body = SearchRequest,
    params(("kind" = String, Path, description = "searxng, brave or tavily")),
    responses((status = 200, body = SearchOverviewDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn put_search_kind(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(kind): Path<String>,
    ApiJson(body): ApiJson<SearchRequest>,
) -> Result<Json<SearchOverviewDto>, ApiError> {
    let kind = parse_search_kind(&kind)?;
    state.app.save_search(&ctx, body.into_input(kind)).await?;
    Ok(Json(SearchOverviewDto::from(
        &state.app.get_search(&ctx).await?,
    )))
}

/// Test one search provider with a small query.
#[utoipa::path(post, path = "/api/admin/search/kinds/{kind}/test", tag = "integrations",
    params(("kind" = String, Path, description = "searxng, brave or tavily")),
    responses((status = 202, body = TestRunDto), (status = 404, body = ErrorEnvelope),
        (status = 409, description = "A test is already running; the body carries its run_id", body = ErrorEnvelope),
        (status = 429, body = ErrorEnvelope)))]
pub async fn test_search(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(kind): Path<String>,
) -> Result<(StatusCode, Json<TestRunDto>), ApiError> {
    let run = state
        .app
        .start_test(&ctx, TestTarget::Search(parse_search_kind(&kind)?))
        .await?;
    Ok((StatusCode::ACCEPTED, Json(TestRunDto::from(&run))))
}

/// Turn one search provider on (the others go off), or search off with no `kind`.
#[utoipa::path(put, path = "/api/admin/search/active", tag = "integrations", request_body = ActiveSearchRequest,
    responses((status = 200, body = ActivationDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn put_active_search(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(body): ApiJson<ActiveSearchRequest>,
) -> Result<Json<ActivationDto>, ApiError> {
    let kind = body
        .kind
        .as_deref()
        .map(|text| {
            SearchKind::parse(text).ok_or_else(|| {
                AppError::validation("kind", "The kind must be `searxng`, `brave` or `tavily`.")
            })
        })
        .transpose()?;
    let activation = state.app.set_active_search(&ctx, kind).await?;
    Ok(Json(ActivationDto::from(activation)))
}

// ---- connector ----------------------------------------------------------------------------

/// The GitHub connector's status.
#[utoipa::path(get, path = "/api/admin/connectors/github", tag = "integrations",
    responses((status = 200, body = ConnectorDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_connector(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<ConnectorDto>, ApiError> {
    Ok(Json(match state.app.get_connector(&ctx).await? {
        Some(view) => ConnectorDto::from(&view),
        None => ConnectorDto::not_configured(),
    }))
}

/// Save or replace the GitHub token. The token is never returned.
#[utoipa::path(put, path = "/api/admin/connectors/github", tag = "integrations", request_body = ConnectorRequest,
    responses((status = 200, body = ConnectorDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn put_connector(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(body): ApiJson<ConnectorRequest>,
) -> Result<Json<ConnectorDto>, ApiError> {
    let view = state
        .app
        .save_connector_token(&ctx, tandem_support::Secret::new(body.token))
        .await?;
    Ok(Json(ConnectorDto::from(&view)))
}

/// Remove the GitHub token; the connector is then off.
#[utoipa::path(delete, path = "/api/admin/connectors/github", tag = "integrations",
    responses((status = 204), (status = 404, body = ErrorEnvelope)))]
pub async fn delete_connector(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<StatusCode, ApiError> {
    state.app.remove_connector(&ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Test the GitHub token: the account, its expiry and the repositories it reaches.
#[utoipa::path(post, path = "/api/admin/connectors/github/test", tag = "integrations",
    responses((status = 202, body = TestRunDto), (status = 404, body = ErrorEnvelope),
        (status = 409, description = "A test is already running; the body carries its run_id", body = ErrorEnvelope),
        (status = 429, body = ErrorEnvelope)))]
pub async fn test_connector(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<(StatusCode, Json<TestRunDto>), ApiError> {
    let run = state
        .app
        .start_test(&ctx, TestTarget::Connector(ConnectorKind::Github))
        .await?;
    Ok((StatusCode::ACCEPTED, Json(TestRunDto::from(&run))))
}

// ---- sandbox policy -----------------------------------------------------------------------

/// The sandbox policy with the factory values, allowed ranges and the tool registry.
#[utoipa::path(get, path = "/api/admin/sandbox-policy", tag = "integrations",
    responses((status = 200, body = PolicyViewDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_policy(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<Json<PolicyViewDto>, ApiError> {
    let policy = state.app.get_sandbox_policy(&ctx).await?;
    Ok(Json(PolicyViewDto::of(&policy)))
}

/// Save the sandbox policy. Choosing the container tier needs a runtime that answers right now.
#[utoipa::path(put, path = "/api/admin/sandbox-policy", tag = "integrations", request_body = PolicyRequest,
    responses((status = 200, body = PolicyViewDto), (status = 404, body = ErrorEnvelope), (status = 422, body = ErrorEnvelope)))]
pub async fn put_policy(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    ApiJson(body): ApiJson<PolicyRequest>,
) -> Result<Json<PolicyViewDto>, ApiError> {
    let saved = state
        .app
        .save_sandbox_policy(&ctx, body.into_candidate()?)
        .await?;
    Ok(Json(PolicyViewDto::of(&saved)))
}

/// Check whether the container runtime answers, without starting anything.
#[utoipa::path(post, path = "/api/admin/runtime/check", tag = "integrations",
    responses((status = 202, body = TestRunDto), (status = 404, body = ErrorEnvelope),
        (status = 409, description = "A check is already running; the body carries its run_id", body = ErrorEnvelope),
        (status = 429, body = ErrorEnvelope)))]
pub async fn check_runtime(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
) -> Result<(StatusCode, Json<TestRunDto>), ApiError> {
    let run = state.app.start_test(&ctx, TestTarget::Runtime).await?;
    Ok((StatusCode::ACCEPTED, Json(TestRunDto::from(&run))))
}

// ---- test runs ----------------------------------------------------------------------------

/// A test run, for the screen that polls it.
#[utoipa::path(get, path = "/api/admin/test-runs/{id}", tag = "integrations",
    params(("id" = String, Path, description = "Run ID")),
    responses((status = 200, body = TestRunDto), (status = 404, body = ErrorEnvelope)))]
pub async fn get_test_run(
    State(state): State<AppState>,
    _admin: AdminUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Json<TestRunDto>, ApiError> {
    let run = state
        .app
        .get_test_run(&ctx, parse_id::<TestRunId>(&id)?)
        .await?;
    Ok(Json(TestRunDto::from(&run)))
}
