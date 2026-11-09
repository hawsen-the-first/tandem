//! JSON logs on standard output with redaction and correlation IDs (SE-4, OB-2).

use crate::redact::{REDACTED, is_sensitive};
use serde_json::{Map, Value};
use std::io::Write;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Span fields lifted to the top level of every log line inside the span.
const CORRELATION_KEYS: &[&str] = &["request_id", "job_id", "trace_id", "user_id", "component"];

/// Collects tracing fields into a JSON map, redacting sensitive names.
struct FieldCollector<'a>(&'a mut Map<String, Value>);

impl FieldCollector<'_> {
    fn put(&mut self, field: &Field, value: Value) {
        let name = field.name();
        let key = if name == "message" { "msg" } else { name };
        let value = if is_sensitive(name) {
            Value::String(REDACTED.to_owned())
        } else {
            value
        };
        self.0.insert(key.to_owned(), value);
    }
}

impl Visit for FieldCollector<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, Value::String(value.to_owned()));
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, Value::from(value));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, Value::from(value));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, Value::from(value));
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, Value::from(value));
    }
    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.put(field, Value::String(value.to_string()));
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, Value::String(format!("{value:?}")));
    }
}

#[derive(Default)]
struct SpanFields(Map<String, Value>);

/// The layer that writes one JSON object per event.
#[derive(Debug)]
pub struct JsonLogLayer<W> {
    make_writer: W,
}

impl<W> JsonLogLayer<W> {
    /// Write to whatever `make_writer` produces.
    pub fn new(make_writer: W) -> Self {
        Self { make_writer }
    }
}

impl<S, W> Layer<S> for JsonLogLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = SpanFields::default();
        attributes.record(&mut FieldCollector(&mut fields.0));
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(fields) = span.extensions_mut().get_mut::<SpanFields>()
        {
            values.record(&mut FieldCollector(&mut fields.0));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().as_str().to_ascii_lowercase();
        metrics::counter!(crate::metrics::names::LOG_EVENTS_TOTAL, "level" => level.clone())
            .increment(1);

        let mut line = Map::new();
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        line.insert("ts".to_owned(), Value::String(timestamp));
        line.insert("level".to_owned(), Value::String(level));
        line.insert(
            "target".to_owned(),
            Value::String(metadata.target().to_owned()),
        );
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<SpanFields>() {
                    for key in CORRELATION_KEYS {
                        if let Some(value) = fields.0.get(*key) {
                            line.insert((*key).to_owned(), value.clone());
                        }
                    }
                }
            }
        }
        event.record(&mut FieldCollector(&mut line));

        if let Ok(mut text) = serde_json::to_vec(&Value::Object(line)) {
            text.push(b'\n');
            let _ = self.make_writer.make_writer().write_all(&text);
        }
    }
}

/// Why logging could not be started.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    /// The level or filter text is not understood.
    #[error("invalid log level: {0}")]
    Level(String),
    /// A global subscriber is already installed.
    #[error("logging is already initialised")]
    AlreadyInitialised,
}

/// Build the subscriber without installing it (used by tests and by [`init_logging`]).
///
/// # Errors
/// [`InitError::Level`] for an invalid level.
pub fn subscriber<W>(
    level: &str,
    make_writer: W,
) -> Result<impl Subscriber + Send + Sync + 'static, InitError>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let filter = EnvFilter::try_new(format!("{level},sqlx=warn"))
        .map_err(|error| InitError::Level(error.to_string()))?;
    Ok(tracing_subscriber::registry()
        .with(filter)
        .with(JsonLogLayer::new(make_writer)))
}

/// Install JSON logging on standard output for the whole process.
///
/// # Errors
/// [`InitError`] for a bad level or a second initialisation.
pub fn init_logging(level: &str) -> Result<(), InitError> {
    subscriber(level, std::io::stdout)?
        .try_init()
        .map_err(|_| InitError::AlreadyInitialised)
}

/// The tracing level for a log-level word, used by tests and callers that gate work on it.
pub fn level_from_text(text: &str) -> Option<Level> {
    text.parse().ok()
}
