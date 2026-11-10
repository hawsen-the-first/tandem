//! Graceful shutdown: one signal observed by every task.

use tokio::sync::watch;

/// Broadcasts "the process is stopping" to every task that holds a receiver.
#[derive(Debug, Clone)]
pub struct Shutdown {
    sender: watch::Sender<bool>,
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Shutdown {
    /// A shutdown that has not been triggered.
    pub fn new() -> Self {
        Self {
            sender: watch::channel(false).0,
        }
    }

    /// A receiver that turns `true` when shutdown begins.
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }

    /// Begin shutdown. Safe to call more than once.
    pub fn trigger(&self) {
        self.sender.send_replace(true);
    }

    /// Whether shutdown has begun.
    pub fn is_triggered(&self) -> bool {
        *self.sender.borrow()
    }
}

/// Wait for SIGTERM or SIGINT (Ctrl-C), or just Ctrl-C where Unix signals do not exist.
pub async fn wait_for_termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = terminate.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
