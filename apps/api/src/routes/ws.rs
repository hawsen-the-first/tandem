//! The event feed over WebSocket (PF-3, BR-13): cookie and Origin checks, a per-user connection
//! limit, ping and pong, `LISTEN`-driven wake-ups with a 1 s poll as the safety net, replay from
//! the last event ID and a resync signal when history has aged out.

use crate::error::ApiError;
use crate::pipeline::access::{Access, FullScopeUser};
use crate::pipeline::rate_limit::Bucket;
use crate::routes::{ApiQuery, RouteTable};
use crate::state::AppState;
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tandem_core::ids::{OutboxEventId, UserId};
use tandem_core::outbox::EventPayload;
use tandem_telemetry::metric_names;
use tokio::sync::{broadcast, watch};

const MAX_CONNECTIONS_PER_USER: usize = 10;
const PING_EVERY: Duration = Duration::from_secs(25);
const MAX_MISSED_PONGS: u32 = 2;
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_EVERY: Duration = Duration::from_secs(1);
const PAGE: usize = 100;
const NOTIFY_CHANNEL: &str = "tandem_events";

pub(super) fn register(table: RouteTable) -> RouteTable {
    table.protected::<FullScopeUser, _, _>(Method::GET, "/api/events", Bucket::General, events)
}

/// Wake-ups from the database and the per-user connection count.
#[derive(Debug, Clone)]
pub struct EventHub {
    wake: broadcast::Sender<()>,
    connections: Arc<Mutex<HashMap<UserId, usize>>>,
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Releases a connection slot when dropped.
#[derive(Debug)]
pub struct ConnectionSlot {
    hub: EventHub,
    user: UserId,
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        let mut connections = self
            .hub
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(count) = connections.get_mut(&self.user) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&self.user);
            }
        }
        let total: usize = connections.values().sum();
        set_connection_gauge(total);
    }
}

fn set_connection_gauge(total: usize) {
    tandem_telemetry::metrics::set_gauge(metric_names::WS_CONNECTIONS, total as f64);
}

impl EventHub {
    /// A hub with no connections.
    pub fn new() -> Self {
        Self {
            wake: broadcast::channel(64).0,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Tell every connection to look for new events.
    pub fn notify(&self) {
        let _ = self.wake.send(());
    }

    /// A receiver of wake-ups.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.wake.subscribe()
    }

    /// Take a connection slot for `user`, or `None` at the per-user limit.
    pub fn try_acquire(&self, user: UserId) -> Option<ConnectionSlot> {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = connections.entry(user).or_insert(0);
        if *count >= MAX_CONNECTIONS_PER_USER {
            return None;
        }
        *count += 1;
        let total: usize = connections.values().sum();
        set_connection_gauge(total);
        Some(ConnectionSlot {
            hub: self.clone(),
            user,
        })
    }

    /// How many connections a user holds (for tests).
    pub fn connections_for(&self, user: UserId) -> usize {
        self.connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&user)
            .copied()
            .unwrap_or(0)
    }
}

/// Keep one `LISTEN` connection open and wake the hub on every notification, reconnecting with a
/// pause after a failure. The 1 s poll in each connection covers any gap.
pub fn spawn_listener(pool: PgPool, hub: EventHub, mut shutdown: watch::Receiver<bool>) {
    tokio::spawn(async move {
        loop {
            if *shutdown.borrow() {
                return;
            }
            match PgListener::connect_with(&pool).await {
                Ok(mut listener) => {
                    if listener.listen(NOTIFY_CHANNEL).await.is_err() {
                        tracing::warn!("could not LISTEN for events; retrying");
                    } else {
                        loop {
                            tokio::select! {
                                notification = listener.recv() => match notification {
                                    Ok(_) => hub.notify(),
                                    Err(error) => {
                                        tracing::warn!(%error, "event listener lost its connection");
                                        break;
                                    }
                                },
                                _ = shutdown.changed() => return,
                            }
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "event listener could not connect"),
            }
            tokio::select! {
                () = tokio::time::sleep(Duration::from_secs(2)) => {}
                _ = shutdown.changed() => return,
            }
        }
    });
}

/// Where to resume the feed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsQuery {
    /// The last event ID the client saw; omit to start from now.
    pub after: Option<i64>,
}

async fn events(
    State(state): State<AppState>,
    user: FullScopeUser,
    upgrade: WebSocketUpgrade,
    headers: HeaderMap,
    ApiQuery(query): ApiQuery<EventsQuery>,
) -> Result<Response, ApiError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    if origin != Some(state.config.public_origin().as_str()) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "bad_origin",
            "That request was refused.",
        ));
    }
    let user_id = user.session().actor.user_id;
    let Some(slot) = state.hub.try_acquire(user_id) else {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "too_many_connections",
            "Too many open connections. Close another tab and try again.",
        ));
    };
    Ok(upgrade.on_upgrade(move |socket| run(socket, state, user_id, query.after, slot)))
}

async fn send_json(socket: &mut WebSocket, value: serde_json::Value) -> bool {
    matches!(
        tokio::time::timeout(
            SEND_TIMEOUT,
            socket.send(Message::Text(value.to_string().into()))
        )
        .await,
        Ok(Ok(()))
    )
}

async fn deliver(
    socket: &mut WebSocket,
    state: &AppState,
    user_id: UserId,
    after: &mut OutboxEventId,
) -> bool {
    loop {
        let Ok(page) = state.app.events_for_user(user_id, *after, PAGE).await else {
            return true;
        };
        if page.resync_required {
            let latest = state.app.latest_event_id().await.unwrap_or(*after);
            *after = latest;
            return send_json(
                socket,
                json!({"type": "resync", "latest_event_id": latest.0}),
            )
            .await;
        }
        for event in &page.events {
            let EventPayload::MessagePosted {
                chat_session_id,
                message_id,
                author_is_agent,
                ..
            } = &event.payload;
            let frame = json!({
                "type": "event",
                "id": event.id.0,
                "kind": event.payload.kind(),
                "chat_session_id": chat_session_id.to_string(),
                "message_id": message_id.to_string(),
                "author_is_agent": author_is_agent,
            });
            if !send_json(socket, frame).await {
                return false;
            }
        }
        let advanced = page.scanned_up_to > *after;
        *after = page.scanned_up_to;
        if !advanced {
            return true;
        }
    }
}

async fn run(
    mut socket: WebSocket,
    state: AppState,
    user_id: UserId,
    after: Option<i64>,
    slot: ConnectionSlot,
) {
    let _slot = slot;
    let mut after = match after {
        Some(id) => OutboxEventId(id),
        None => state
            .app
            .latest_event_id()
            .await
            .unwrap_or(OutboxEventId(0)),
    };
    if !send_json(
        &mut socket,
        json!({"type": "ready", "latest_event_id": after.0}),
    )
    .await
    {
        return;
    }
    let mut wake = state.hub.subscribe();
    let mut shutdown = state.shutdown.subscribe();
    let mut poll = tokio::time::interval(POLL_EVERY);
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    let mut missed_pongs = 0_u32;
    loop {
        tokio::select! {
            _ = poll.tick() => {
                if !deliver(&mut socket, &state, user_id, &mut after).await { return; }
            }
            woke = wake.recv() => {
                if matches!(woke, Err(broadcast::error::RecvError::Closed)) { return; }
                if !deliver(&mut socket, &state, user_id, &mut after).await { return; }
            }
            _ = ping.tick() => {
                if missed_pongs >= MAX_MISSED_PONGS { return; }
                missed_pongs += 1;
                let sent = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Ping(Vec::new().into()))).await;
                if !matches!(sent, Ok(Ok(()))) { return; }
            }
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Pong(_))) => missed_pongs = 0,
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
            _ = shutdown.changed() => {
                let _ = socket
                    .send(Message::Close(Some(CloseFrame { code: 1001, reason: "server shutting down".into() })))
                    .await;
                return;
            }
        }
    }
}
