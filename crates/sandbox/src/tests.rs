use super::*;
use std::sync::{Arc, Mutex};
use tokio::net::UnixListener;

const DOCKER: &str = r#"{"Platform":{"Name":"Docker Engine - Community"},"Version":"27.0.3","ApiVersion":"1.46","Components":[{"Name":"Engine","Version":"27.0.3"}]}"#;

fn http(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// A runtime double that answers one connection with `answer`, returning the request it saw.
fn serve(socket: &std::path::Path, answer: Vec<u8>) -> Arc<Mutex<Vec<u8>>> {
    let listener = UnixListener::bind(socket).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0_u8; 4096];
        let read = stream.read(&mut buffer).await.unwrap();
        record.lock().unwrap().extend_from_slice(&buffer[..read]);
        stream.write_all(&answer).await.unwrap();
        stream.shutdown().await.unwrap();
    });
    seen
}

async fn probe_with(answer: Vec<u8>) -> ProbeResult {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("runtime.sock");
    serve(&socket, answer);
    DockerRuntimeProbe::new(Some(socket)).probe().await
}

#[tokio::test]
async fn a_docker_runtime_is_reachable_and_reports_its_name_and_version() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("docker.sock");
    let seen = serve(&socket, http("200 OK", DOCKER));
    let result = DockerRuntimeProbe::new(Some(socket)).probe().await;
    assert_eq!(result.subject, ProbeSubject::Runtime);
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(
        result.details.runtime_name.as_deref(),
        Some("Docker Engine - Community")
    );
    assert_eq!(result.details.runtime_version.as_deref(), Some("27.0.3"));
    let request = String::from_utf8(seen.lock().unwrap().clone()).unwrap();
    assert!(
        request.starts_with("GET /version HTTP/1.0\r\n"),
        "{request}"
    );
    assert_eq!(request.matches("HTTP/1.").count(), 1, "one request only");
}

#[tokio::test]
async fn a_podman_style_answer_without_a_platform_uses_the_first_component() {
    let body = r#"{"Version":"5.2.1","Components":[{"Name":"Podman Engine","Version":"5.2.1"}]}"#;
    let result = probe_with(http("200 OK", body)).await;
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(
        result.details.runtime_name.as_deref(),
        Some("Podman Engine")
    );
}

#[tokio::test]
async fn no_socket_configured_is_not_configured() {
    let result = DockerRuntimeProbe::new(None).probe().await;
    assert_eq!(result.status, TestStatus::NotConfigured);
}

#[tokio::test]
async fn a_socket_that_does_not_exist_is_unreachable() {
    let directory = tempfile::tempdir().unwrap();
    let result = DockerRuntimeProbe::new(Some(directory.path().join("missing.sock")))
        .probe()
        .await;
    assert_eq!(result.status, TestStatus::Unreachable);
}

#[tokio::test]
async fn a_plain_file_where_the_socket_should_be_is_unreachable() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("not-a-socket");
    std::fs::write(&file, "x").unwrap();
    let result = DockerRuntimeProbe::new(Some(file)).probe().await;
    assert_eq!(result.status, TestStatus::Unreachable);
}

#[tokio::test]
async fn a_runtime_that_never_answers_times_out() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("silent.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let result = DockerRuntimeProbe::new(Some(socket))
        .with_timeout(Duration::from_millis(200))
        .probe()
        .await;
    assert_eq!(result.status, TestStatus::Timeout);
}

#[tokio::test]
async fn answers_that_are_not_a_version_document_are_bad_responses() {
    for answer in [
        http(
            "500 Internal Server Error",
            r#"{"message":"SECRET-REMOTE-TEXT"}"#,
        ),
        http("404 Not Found", ""),
        http("200 OK", "not json"),
        http("200 OK", r#"{"nothing":1}"#),
        b"garbage with no header end".to_vec(),
        Vec::new(),
    ] {
        let result = probe_with(answer.clone()).await;
        assert_eq!(result.status, TestStatus::BadResponse, "{answer:?}");
        assert!(!format!("{result:?}").contains("SECRET-REMOTE-TEXT"));
    }
}

#[tokio::test]
async fn an_answer_over_64_kib_is_a_bad_response() {
    let padding = "x".repeat(MAX_ANSWER_BYTES);
    let body = format!(r#"{{"Version":"1.0","Padding":"{padding}"}}"#);
    let result = probe_with(http("200 OK", &body)).await;
    assert_eq!(result.status, TestStatus::BadResponse);
}

#[tokio::test]
async fn a_name_or_version_that_could_carry_anything_else_is_dropped() {
    let body =
        r#"{"Platform":{"Name":"<script>alert(1)</script>"},"Version":"1.0\nInjected: yes"}"#;
    let result = probe_with(http("200 OK", body)).await;
    assert_eq!(result.status, TestStatus::Ok);
    assert_eq!(result.details.runtime_name, None);
    assert_eq!(result.details.runtime_version, None);
}

#[test]
fn names_and_versions_are_cleaned_strictly() {
    assert_eq!(clean_name("Docker Engine"), Some("Docker Engine".into()));
    assert_eq!(clean_name(""), None);
    assert_eq!(clean_name(&"a".repeat(65)), None);
    assert_eq!(clean_version("27.0.3-rc1+b2"), Some("27.0.3-rc1+b2".into()));
    assert_eq!(clean_version("1 2"), None);
    assert_eq!(clean_version(&"1".repeat(41)), None);
}
