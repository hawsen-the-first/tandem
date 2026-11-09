use crate::logging::subscriber;
use serde_json::Value;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(Arc::clone(&self.0))
    }
}

impl Capture {
    fn lines(&self) -> Vec<Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line is JSON"))
            .collect()
    }
}

fn run(level: &str, body: impl FnOnce()) -> Vec<Value> {
    let capture = Capture::default();
    let subscriber = subscriber(level, capture.clone()).unwrap();
    tracing::subscriber::with_default(subscriber, body);
    capture.lines()
}

#[test]
fn a_line_has_the_documented_fields() {
    let lines = run("info", || tracing::info!(route = "/api/x", "handled"));
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line["level"], "info");
    assert_eq!(line["msg"], "handled");
    assert_eq!(line["route"], "/api/x");
    assert!(line["ts"].as_str().unwrap().ends_with('Z'));
    assert!(line["target"].as_str().unwrap().contains("logging_tests"));
}

#[test]
fn sensitive_fields_are_redacted_on_events_and_spans() {
    let lines = run("info", || {
        let span = tracing::info_span!("request", request_id = "r-1", password = "hunter2");
        let _guard = span.enter();
        tracing::info!(
            token = "abc123",
            recovery_code = "AAAA-BBBB",
            email = "who@example.test",
            authorization = "Bearer xyz",
            status_code = 200,
            "signed in"
        );
    });
    let text = serde_json::to_string(&lines).unwrap();
    for leaked in [
        "hunter2",
        "abc123",
        "AAAA-BBBB",
        "who@example.test",
        "Bearer xyz",
    ] {
        assert!(!text.contains(leaked), "{leaked} leaked: {text}");
    }
    assert_eq!(lines[0]["token"], "[redacted]");
    assert_eq!(lines[0]["status_code"], 200);
}

#[test]
fn span_correlation_ids_appear_on_events_inside_the_span() {
    let lines = run("info", || {
        let outer = tracing::info_span!("request", request_id = "req-9", user_id = "u-1");
        let _outer = outer.enter();
        let inner = tracing::info_span!("db", trace_id = "t-5", note = "ignored");
        let _inner = inner.enter();
        tracing::info!("inside");
    });
    assert_eq!(lines[0]["request_id"], "req-9");
    assert_eq!(lines[0]["user_id"], "u-1");
    assert_eq!(lines[0]["trace_id"], "t-5");
    assert!(
        lines[0].get("note").is_none(),
        "only correlation ids are lifted"
    );
}

#[test]
fn recorded_span_fields_reach_later_events() {
    let lines = run("info", || {
        let span = tracing::info_span!("request", user_id = tracing::field::Empty);
        let _guard = span.enter();
        span.record("user_id", "u-7");
        tracing::info!("after auth");
    });
    assert_eq!(lines[0]["user_id"], "u-7");
}

#[test]
fn the_level_filters_events() {
    let lines = run("warn", || {
        tracing::info!("quiet");
        tracing::warn!("loud");
    });
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["msg"], "loud");
}

#[test]
fn an_invalid_level_is_an_error() {
    assert!(subscriber("tandem=nonsense", Capture::default()).is_err());
}

#[test]
fn events_without_a_span_have_no_correlation_ids() {
    let lines = run("info", || tracing::info!("plain"));
    assert!(lines[0].get("request_id").is_none());
}
