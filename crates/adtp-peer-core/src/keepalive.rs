use adtp_core::errors::AdtpResult;
use adtp_core::types::{
    DEAD_DETECTION_PINGS, KEEPALIVE_DEFAULT_MS, PEER_ID_SIZE, RECONNECT_BACKOFF_MAX_SECS,
};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub struct Keepalive {
    peer_id: [u8; PEER_ID_SIZE],
    missed_pongs: Arc<AtomicU8>,
    interval_ms: u64,
}

impl Keepalive {
    pub fn new(peer_id: [u8; PEER_ID_SIZE], interval_ms: Option<u64>) -> Self {
        Self {
            peer_id,
            missed_pongs: Arc::new(AtomicU8::new(0)),
            interval_ms: interval_ms.unwrap_or(KEEPALIVE_DEFAULT_MS),
        }
    }

    pub fn missed(&self) -> u8 {
        self.missed_pongs.load(Ordering::Relaxed)
    }

    pub fn record_pong(&self) {
        self.missed_pongs.store(0, Ordering::Relaxed);
    }

    pub fn record_ping_sent(&self) {
        self.missed_pongs.fetch_add(1, Ordering::Relaxed);
    }

    pub fn is_dead(&self) -> bool {
        self.missed_pongs.load(Ordering::Relaxed) >= DEAD_DETECTION_PINGS
    }

    pub async fn run<F, Fut>(self, mut send_ping: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = AdtpResult<()>>,
    {
        let mut ticker = interval(Duration::from_millis(self.interval_ms));

        loop {
            ticker.tick().await;
            self.record_ping_sent();

            match send_ping().await {
                Ok(()) => {
                    if self.is_dead() {
                        tracing::warn!("Keepalive dead for peer {}", hex::encode(self.peer_id));
                    }
                }
                Err(e) => {
                    tracing::error!("Keepalive ping error: {e}");
                    self.record_ping_sent();
                }
            }
        }
    }

    pub fn backoff(attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }
        let secs = std::cmp::min(2u64.pow(attempt), RECONNECT_BACKOFF_MAX_SECS);
        Duration::from_secs(secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keepalive_initial_state() {
        let ka = Keepalive::new([1u8; PEER_ID_SIZE], Some(1000));
        assert_eq!(ka.missed(), 0);
        assert!(!ka.is_dead());
    }

    #[test]
    fn test_dead_detection() {
        let ka = Keepalive::new([1u8; PEER_ID_SIZE], Some(1000));
        ka.record_pong();
        assert_eq!(ka.missed(), 0);
        ka.record_ping_sent();
        ka.record_ping_sent();
        ka.record_ping_sent();
        assert!(ka.is_dead());
        ka.record_pong();
        assert!(!ka.is_dead());
    }

    #[test]
    fn test_backoff_timing() {
        assert_eq!(Keepalive::backoff(0), Duration::from_millis(0));
        assert_eq!(Keepalive::backoff(1), Duration::from_secs(2));
        assert_eq!(Keepalive::backoff(2), Duration::from_secs(4));
        assert_eq!(Keepalive::backoff(6), Duration::from_secs(60)); // capped
    }
}
