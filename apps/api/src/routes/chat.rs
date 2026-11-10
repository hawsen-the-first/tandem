//! The walking skeleton's chat endpoints.

use crate::dto::{ChatMessageDto, ChatSessionDto, CreateChatSessionRequest, PostMessageRequest};
use crate::error::{ApiError, ErrorEnvelope};
use crate::pipeline::Ctx;
use crate::pipeline::access::FullScopeUser;
use crate::pipeline::rate_limit::Bucket;
use crate::routes::{ApiJson, RouteTable, parse_id};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode};
use tandem_core::ids::ChatSessionId;

pub(super) fn register(table: RouteTable) -> RouteTable {
    let general = Bucket::General;
    table
        .protected::<FullScopeUser, _, _>(Method::GET, "/api/chat/sessions", general, list_sessions)
        .protected::<FullScopeUser, _, _>(
            Method::POST,
            "/api/chat/sessions",
            general,
            create_session,
        )
        .protected::<FullScopeUser, _, _>(
            Method::GET,
            "/api/chat/sessions/{id}/messages",
            general,
            list_messages,
        )
        .protected::<FullScopeUser, _, _>(
            Method::POST,
            "/api/chat/sessions/{id}/messages",
            general,
            post_message,
        )
}

/// The caller's chat sessions, newest first.
#[utoipa::path(get, path = "/api/chat/sessions", tag = "chat",
    responses((status = 200, body = Vec<ChatSessionDto>), (status = 401, body = ErrorEnvelope)))]
pub async fn list_sessions(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
) -> Result<Json<Vec<ChatSessionDto>>, ApiError> {
    let sessions = state.app.list_chat_sessions(&ctx).await?;
    Ok(Json(sessions.iter().map(ChatSessionDto::from).collect()))
}

/// Start a chat session.
#[utoipa::path(post, path = "/api/chat/sessions", tag = "chat", request_body = CreateChatSessionRequest,
    responses((status = 201, body = ChatSessionDto), (status = 422, body = ErrorEnvelope)))]
pub async fn create_session(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
    ApiJson(request): ApiJson<CreateChatSessionRequest>,
) -> Result<(StatusCode, Json<ChatSessionDto>), ApiError> {
    let session = state.app.create_chat_session(&ctx, &request.title).await?;
    Ok((StatusCode::CREATED, Json(ChatSessionDto::from(&session))))
}

/// The messages of one of the caller's sessions, oldest first.
#[utoipa::path(get, path = "/api/chat/sessions/{id}/messages", tag = "chat",
    params(("id" = String, Path, description = "Session ID")),
    responses((status = 200, body = Vec<ChatMessageDto>), (status = 404, body = ErrorEnvelope)))]
pub async fn list_messages(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Json<Vec<ChatMessageDto>>, ApiError> {
    let messages = state
        .app
        .list_messages(&ctx, parse_id::<ChatSessionId>(&id)?)
        .await?;
    Ok(Json(messages.iter().map(ChatMessageDto::from).collect()))
}

/// Post a message; the agent's reply arrives over the event feed.
#[utoipa::path(post, path = "/api/chat/sessions/{id}/messages", tag = "chat", request_body = PostMessageRequest,
    params(("id" = String, Path, description = "Session ID")),
    responses((status = 201, body = ChatMessageDto), (status = 404, body = ErrorEnvelope),
              (status = 422, body = ErrorEnvelope)))]
pub async fn post_message(
    State(state): State<AppState>,
    _user: FullScopeUser,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ApiJson(request): ApiJson<PostMessageRequest>,
) -> Result<(StatusCode, Json<ChatMessageDto>), ApiError> {
    let message = state
        .app
        .post_message(&ctx, parse_id::<ChatSessionId>(&id)?, &request.body)
        .await?;
    Ok((StatusCode::CREATED, Json(ChatMessageDto::from(&message))))
}
