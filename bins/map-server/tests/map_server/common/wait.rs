//! Waiting for asynchronous effects.

use std::time::Duration;

/// Polls `cond` every 20 ms for up to 2 s.
pub async fn wait_until(cond: impl Fn() -> bool) -> bool {
    wait_until_for(cond, Duration::from_secs(2)).await
}

pub async fn wait_until_for(cond: impl Fn() -> bool, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cond()
}
pub const LONG: Duration = Duration::from_secs(4);

pub const SHORT: Duration = Duration::from_millis(150);
