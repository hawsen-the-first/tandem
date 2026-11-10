//! The OpenAPI description matches the routes and the committed copy is current (MT-3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use tandem_api::routes::{all_routes, openapi};

/// Routes that are intentionally not described: operations endpoints and the event WebSocket.
const UNDOCUMENTED: &[&str] = &[
    "GET /healthz",
    "GET /readyz",
    "GET /metrics",
    "GET /api/openapi.json",
    "GET /api/events",
];

fn documented() -> BTreeSet<String> {
    let document = serde_json::to_value(openapi::document()).unwrap();
    let mut operations = BTreeSet::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        for method in item.as_object().unwrap().keys() {
            operations.insert(format!("{} {path}", method.to_uppercase()));
        }
    }
    operations
}

#[test]
fn every_route_is_documented_and_every_documented_operation_is_routed() {
    let (_, info) = all_routes().unwrap();
    let routed: BTreeSet<String> = info
        .keys()
        .map(|(method, path)| format!("{method} {path}"))
        .filter(|route| !UNDOCUMENTED.contains(&route.as_str()))
        .collect();
    assert_eq!(documented(), routed);
}

#[test]
fn every_operation_documents_its_error_envelope_or_has_none_to_document() {
    let document = serde_json::to_value(openapi::document()).unwrap();
    assert!(document["components"]["schemas"]["ErrorEnvelope"].is_object());
    assert_eq!(document["info"]["title"], "Tandem API");
}

#[test]
fn the_committed_description_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
    let current = openapi::document_json().unwrap();
    if std::env::var_os("UPDATE_OPENAPI").is_some() {
        std::fs::write(&path, format!("{current}\n")).unwrap();
    }
    let committed = std::fs::read_to_string(&path)
        .expect("apps/api/openapi.json is committed (run with UPDATE_OPENAPI=1 to write it)");
    assert_eq!(
        committed.trim_end(),
        current.trim_end(),
        "regenerate with: UPDATE_OPENAPI=1 cargo test -p tandem-api --test contract"
    );
}
