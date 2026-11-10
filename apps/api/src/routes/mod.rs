//! Route registration. Every route goes through [`RouteTable`], which records who may call it and
//! wraps it in the matching guard, so a route that is neither public nor guarded cannot exist
//! (SE-9; the start-up check).

pub mod admin;
pub mod assets;
pub mod auth;
pub mod chat;
pub mod health;
pub mod openapi;
pub mod ws;

use crate::error::ApiError;
use crate::pipeline::access::{Access, AccessKind, guard};
use crate::pipeline::rate_limit::Bucket;
use crate::state::{AppState, RouteInfo, RouteInfoMap};
use axum::Router;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts};
use axum::handler::Handler;
use axum::http::{Method, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::routing::{MethodFilter, on};
use std::str::FromStr;

/// JSON body extractor whose failures use the error envelope.
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection {
            JsonRejection::MissingJsonContentType(_) => ApiError::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "Send the request as JSON.",
            ),
            JsonRejection::BytesRejection(_) => ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "That request is too large.",
            ),
            _ => ApiError::bad_request("The request body is not valid."),
        }
    }
}

/// Query-string extractor whose failures use the error envelope.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct ApiQuery<T>(pub T);

impl From<QueryRejection> for ApiError {
    fn from(_: QueryRejection) -> Self {
        ApiError::bad_request("The query is not valid.")
    }
}

/// Parse an ID from a path segment; anything malformed is simply not found.
///
/// # Errors
/// [`ApiError::not_found`] for a malformed ID.
pub fn parse_id<T: FromStr>(text: &str) -> Result<T, ApiError> {
    text.parse().map_err(|_| ApiError::not_found())
}

/// Why the route table is not acceptable.
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    /// The same method and path were registered twice.
    #[error("route registered twice: {0} {1}")]
    Duplicate(Method, String),
    /// A method that routes cannot express.
    #[error("unsupported method {0}")]
    UnsupportedMethod(Method),
}

/// Collects routes and their access classes.
#[derive(Debug)]
pub struct RouteTable {
    router: Router<AppState>,
    info: RouteInfoMap,
    error: Option<RouteError>,
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteTable {
    /// An empty table.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            info: RouteInfoMap::new(),
            error: None,
        }
    }

    fn add<H, T>(
        mut self,
        method: Method,
        path: &'static str,
        access: AccessKind,
        bucket: Bucket,
        handler: H,
    ) -> Self
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        if self.error.is_some() {
            return self;
        }
        let Ok(filter) = MethodFilter::try_from(method.clone()) else {
            self.error = Some(RouteError::UnsupportedMethod(method));
            return self;
        };
        if self
            .info
            .insert(
                (method.clone(), path.to_owned()),
                RouteInfo { access, bucket },
            )
            .is_some()
        {
            self.error = Some(RouteError::Duplicate(method, path.to_owned()));
            return self;
        }
        let guarded = on(filter, handler).route_layer(from_fn_with_state(access, guard));
        self.router = self.router.route(path, guarded);
        self
    }

    /// A route anyone may call.
    pub fn public<H, T>(
        self,
        method: Method,
        path: &'static str,
        bucket: Bucket,
        handler: H,
    ) -> Self
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.add(method, path, AccessKind::Public, bucket, handler)
    }

    /// A route for the class `A`; the handler takes `A` in its signature.
    pub fn protected<A: Access, H, T>(
        self,
        method: Method,
        path: &'static str,
        bucket: Bucket,
        handler: H,
    ) -> Self
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.add(method, path, A::KIND, bucket, handler)
    }

    /// Finish: the router and the facts the pipeline needs.
    ///
    /// # Errors
    /// [`RouteError`] when a route was registered twice or with an unsupported method.
    pub fn finish(self) -> Result<(Router<AppState>, RouteInfoMap), RouteError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok((self.router, self.info)),
        }
    }
}

/// Every route of the service.
///
/// # Errors
/// [`RouteError`] if the table is inconsistent (a programming error caught at start-up).
pub fn all_routes() -> Result<(Router<AppState>, RouteInfoMap), RouteError> {
    let table = RouteTable::new();
    let table = health::register(table);
    let table = auth::register(table);
    let table = admin::register(table);
    let table = chat::register(table);
    let table = ws::register(table);
    let table = openapi::register(table);
    table.finish()
}
