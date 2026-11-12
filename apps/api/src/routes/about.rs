//! `GET /api/about`: the legal notices the interface must show (AGPL section 0, "Appropriate Legal
//! Notices", and the attribution terms in `LICENSE-TERMS.md`). Public, so the sign-in page can show them.

use crate::dto::AboutDto;
use crate::pipeline::rate_limit::Bucket;
use crate::routes::RouteTable;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::Method;

/// The copyright line shown in the footer. Forks must keep it (`LICENSE-TERMS.md`).
pub const COPYRIGHT: &str = "Copyright (C) 2026 Reuben Hawthorne-Jensen and Tandem contributors";
/// The licence, by SPDX identifier.
pub const LICENSE_NAME: &str = "AGPL-3.0-only";
/// Where to read the licence text.
pub const LICENSE_URL: &str = "https://www.gnu.org/licenses/agpl-3.0.html";
/// The original project, credited as "Based on Tandem".
pub const UPSTREAM_URL: &str = "https://github.com/hawsen-the-first/tandem";

pub(super) fn register(table: RouteTable) -> RouteTable {
    table.public(Method::GET, "/api/about", Bucket::General, about)
}

/// The version, licence and source location of this running copy.
#[utoipa::path(get, path = "/api/about", tag = "about",
    responses((status = 200, body = AboutDto)))]
pub async fn about(State(state): State<AppState>) -> Json<AboutDto> {
    Json(AboutDto {
        name: "Tandem".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        copyright: COPYRIGHT.to_owned(),
        license: LICENSE_NAME.to_owned(),
        license_url: LICENSE_URL.to_owned(),
        source_url: state.config.source_url.as_str().to_owned(),
        upstream_url: UPSTREAM_URL.to_owned(),
    })
}
