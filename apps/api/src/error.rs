//! Mapping application errors to HTTP answers. Internal details never reach the client (SE-16).

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tandem_app::AppError;
use time::OffsetDateTime;
use utoipa::ToSchema;

/// One field problem in a validation answer.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldProblem {
    /// The field as the client knows it.
    pub field: String,
    /// What to fix.
    pub message: String,
}

/// The body of every error answer.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorBody {
    /// A stable machine-readable code.
    pub code: String,
    /// A plain-words message safe to show to the user.
    pub message: String,
    /// Field problems, for validation errors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldProblem>,
}

/// The error envelope: `{"error": {...}}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorEnvelope {
    /// The error.
    pub error: ErrorBody,
}

/// An error ready to become a response.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
    retry_after_seconds: Option<u64>,
}

impl ApiError {
    /// An error with a code and message.
    pub fn new(status: StatusCode, code: &str, message: &str) -> Self {
        Self {
            status,
            body: ErrorBody {
                code: code.to_owned(),
                message: message.to_owned(),
                fields: Vec::new(),
            },
            retry_after_seconds: None,
        }
    }

    /// 404.
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", "Not found.")
    }

    /// 401.
    pub fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "You need to sign in.",
        )
    }

    /// 403.
    pub fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "You are not allowed to do that.",
        )
    }

    /// 403 for a sign-in that must finish MFA enrolment first.
    pub fn enrolment_required() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "mfa_enrolment_required",
            "Set up two-step sign-in to continue.",
        )
    }

    /// 400 for a request that cannot be understood.
    pub fn bad_request(message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    /// 429 with a `Retry-After`.
    pub fn rate_limited(retry_after_seconds: u64) -> Self {
        let mut error = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many attempts. Try again in a little while.",
        );
        error.retry_after_seconds = Some(retry_after_seconds.max(1));
        error
    }

    /// 500 with no detail.
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Something went wrong. Try again.",
        )
    }

    /// The status.
    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::NotFound => Self::not_found(),
            AppError::Forbidden => Self::forbidden(),
            AppError::Unauthenticated => Self::unauthenticated(),
            AppError::Validation(issues) => {
                let mut api = Self::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation",
                    "Check the highlighted fields.",
                );
                api.body.fields = issues
                    .into_iter()
                    .map(|issue| FieldProblem {
                        field: issue.field.to_owned(),
                        message: issue.message,
                    })
                    .collect();
                api
            }
            AppError::InvalidCredentials => Self::new(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Those details do not match.",
            ),
            AppError::Refused { retry_at } => {
                let seconds = (retry_at - OffsetDateTime::now_utc()).whole_seconds();
                Self::rate_limited(u64::try_from(seconds).unwrap_or(1))
            }
            AppError::Busy => {
                let mut api = Self::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "busy",
                    "Tandem is busy right now. Try again in a moment.",
                );
                api.retry_after_seconds = Some(2);
                api
            }
            AppError::InvalidLink => Self::new(
                StatusCode::NOT_FOUND,
                "invalid_link",
                "This link is not valid.",
            ),
            AppError::AlreadyRunning { .. } => Self::new(
                StatusCode::CONFLICT,
                "already_running",
                "A test is already running.",
            ),
            AppError::Internal(detail) => {
                tracing::error!(detail = %detail, "request failed with an internal error");
                Self::internal()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(ErrorEnvelope { error: self.body })).into_response();
        if let Some(seconds) = self.retry_after_seconds
            && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        response
    }
}
