//! Supervised background tasks: a failing or panicking task is restarted with backoff.

use crate::backoff::Backoff;
use std::future::Future;
use std::time::Duration as StdDuration;
use tokio::sync::watch;
use tokio::time::Instant;

/// A task that ran at least this long before failing is treated as healthy: the backoff restarts.
const HEALTHY_RUN: StdDuration = StdDuration::from_secs(60);

/// Boxed error type returned by supervised tasks.
pub type TaskError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Run `make()` repeatedly until it returns `Ok(())` or shutdown is signalled.
///
/// An `Err` or a panic is logged and the task restarts after `backoff`. The shutdown receiver turns
/// `true` when the process is stopping.
pub async fn supervise<F, Fut>(
    name: &'static str,
    backoff: Backoff,
    mut shutdown: watch::Receiver<bool>,
    mut make: F,
) where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = Result<(), TaskError>> + Send + 'static,
{
    let mut failures: u32 = 0;
    loop {
        if *shutdown.borrow() {
            return;
        }
        let started = Instant::now();
        let handle = tokio::spawn(make());
        let outcome = tokio::select! {
            joined = handle => Some(joined),
            _ = shutdown.changed() => None,
        };
        match outcome {
            None => return,
            Some(Ok(Ok(()))) => return,
            Some(Ok(Err(error))) => tracing::error!(task = name, %error, "supervised task failed"),
            Some(Err(join_error)) => {
                tracing::error!(
                    task = name,
                    panicked = join_error.is_panic(),
                    "supervised task aborted"
                );
            }
        }
        if started.elapsed() >= HEALTHY_RUN {
            failures = 0;
        }
        let delay = backoff.delay(failures);
        failures = failures.saturating_add(1);
        let sleep = tokio::time::sleep(StdDuration::from_millis(
            u64::try_from(delay.whole_milliseconds()).unwrap_or(u64::MAX),
        ));
        tokio::select! {
            () = sleep => {}
            _ = shutdown.changed() => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use time::Duration;

    fn backoff() -> Backoff {
        Backoff::new(Duration::seconds(1), Duration::seconds(30))
    }

    #[tokio::test(start_paused = true)]
    async fn restarts_a_failing_task_until_it_succeeds() {
        let attempts = Arc::new(AtomicU32::new(0));
        let (_tx, rx) = watch::channel(false);
        let counter = Arc::clone(&attempts);
        supervise("flaky", backoff(), rx, move || {
            let counter = Arc::clone(&counter);
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err("not yet".into())
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn restarts_a_panicking_task() {
        let attempts = Arc::new(AtomicU32::new(0));
        let (_tx, rx) = watch::channel(false);
        let counter = Arc::clone(&attempts);
        supervise("panicky", backoff(), rx, move || {
            let counter = Arc::clone(&counter);
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    #[allow(clippy::panic)]
                    {
                        panic!("first run panics");
                    }
                }
                Ok(())
            }
        })
        .await;
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_promptly_on_shutdown() {
        let (tx, rx) = watch::channel(false);
        let supervised = tokio::spawn(supervise("forever", backoff(), rx, || async {
            std::future::pending::<()>().await;
            Ok(())
        }));
        tokio::time::sleep(StdDuration::from_secs(5)).await;
        tx.send(true).unwrap();
        supervised.await.unwrap();
    }
}
