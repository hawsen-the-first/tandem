//! The built web client: caching, SPA fallback, no listings, no traversal.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Harness, Options, get};

fn harness_with_client() -> (Harness, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("index.html"), "<html>entry</html>").unwrap();
    std::fs::create_dir(directory.path().join("assets")).unwrap();
    std::fs::write(
        directory.path().join("assets/app-3f9c.js"),
        "console.log(1)",
    )
    .unwrap();
    std::fs::write(directory.path().join("secret.txt"), "not for the web").unwrap_or(());
    let harness = Harness::with(Options {
        web_dir: Some(directory.path().to_path_buf()),
        ..Options::default()
    });
    (harness, directory)
}

#[tokio::test]
async fn the_entry_page_is_never_cached_and_client_routes_get_it() {
    let (harness, _directory) = harness_with_client();
    for path in ["/", "/index.html", "/sign-in", "/admin/users"] {
        let reply = harness.send(get(path)).await;
        assert_eq!(reply.status, StatusCode::OK, "{path}");
        assert!(reply.text.contains("entry"), "{path}");
        assert_eq!(reply.header("cache-control"), "no-cache", "{path}");
        assert!(
            reply
                .header("content-security-policy")
                .contains("default-src 'self'")
        );
    }
}

#[tokio::test]
async fn hashed_assets_are_immutable() {
    let (harness, _directory) = harness_with_client();
    let reply = harness.send(get("/assets/app-3f9c.js")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.text, "console.log(1)");
    assert_eq!(
        reply.header("cache-control"),
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn directories_are_not_listed() {
    let (harness, _directory) = harness_with_client();
    let reply = harness.send(get("/assets/")).await;
    assert!(!reply.text.contains("app-3f9c.js"), "{}", reply.text);
}

#[tokio::test]
async fn paths_cannot_escape_the_client_directory() {
    let (harness, directory) = harness_with_client();
    let outside = directory
        .path()
        .parent()
        .unwrap()
        .join("tandem-outside-secret.txt");
    std::fs::write(&outside, "outside").unwrap();
    for path in [
        "/../tandem-outside-secret.txt",
        "/%2e%2e/tandem-outside-secret.txt",
        "/assets/..%2f..%2ftandem-outside-secret.txt",
    ] {
        let reply = harness.send(get(path)).await;
        assert!(!reply.text.contains("outside"), "{path}: {}", reply.text);
    }
    std::fs::remove_file(outside).unwrap();
}

#[tokio::test]
async fn api_paths_never_fall_back_to_the_client_page() {
    let (harness, _directory) = harness_with_client();
    let reply = harness.send(get("/api/not-a-route")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.error_code(), "not_found");
    assert!(!reply.text.contains("entry"));
}

#[tokio::test]
async fn without_a_client_directory_unknown_paths_are_plain_not_found() {
    let harness = Harness::new();
    let reply = harness.send(get("/")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}
