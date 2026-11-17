//! Per-user adapter that implements the ai crate's `RateLimitProvider`.
//! Bundles the shared `RateLimiter` with a specific user identity and
//! tracks the current turn's input estimate so `record_usage` and
//! `release` can reconcile the reservation properly.

use std::sync::Mutex;

use tokio::time::Instant;

use ai::rate_limit_provider::{RateLimitError as AiRateLimitError, RateLimitProvider};
use uuid::Uuid;

use crate::rate_limiting::constants::{
    PER_REQUEST_INPUT_TOKEN_CAP, SLOT_POLL_INTERVAL, SLOT_WAIT_LIMIT,
};
use crate::rate_limiting::rate_limiter::RateLimiter;

pub struct UserRateLimiter {
    rate_limiter: RateLimiter,
    user_id: Uuid,
    /// Tracks the input-token estimate reserved by `pre_check`, needed by
    /// `record_usage`/`release` to reconcile the reservation. `Mutex<i64>`
    /// (not `tokio::Mutex`) because contention is impossible: each turn has
    /// its own `UserRateLimiter` and methods are called serially.
    reserved_input_tokens: Mutex<Option<i64>>,
    waits_for_slot: bool,
}

impl UserRateLimiter {
    pub fn new(rate_limiter: RateLimiter, user_id: Uuid) -> Self {
        Self {
            rate_limiter,
            user_id,
            reserved_input_tokens: Mutex::new(None),
            waits_for_slot: false,
        }
    }

    pub fn waiting_for_slot(mut self) -> Self {
        self.waits_for_slot = true;
        self
    }

    fn take_reservation(&self) -> Option<i64> {
        self.reserved_input_tokens
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
    }
}

impl RateLimitProvider for UserRateLimiter {
    async fn pre_check(&self, estimated_input_tokens: u64) -> Result<(), AiRateLimitError> {
        let estimated = i64::try_from(estimated_input_tokens).unwrap_or(i64::MAX);
        if estimated > PER_REQUEST_INPUT_TOKEN_CAP {
            return Err(AiRateLimitError::PerRequestCap);
        }

        self.rate_limiter
            .check_quota(self.user_id, estimated)
            .await
            .map_err(|e| AiRateLimitError::QuotaExceeded {
                scope: e.scope.into(),
                reset_at: Some(e.reset_at),
            })?;

        let give_up = Instant::now() + SLOT_WAIT_LIMIT;
        while !self
            .rate_limiter
            .acquire_concurrency_slot(self.user_id)
            .await
        {
            if !self.waits_for_slot || Instant::now() >= give_up {
                self.rate_limiter
                    .release_input_reservation(self.user_id, estimated)
                    .await;
                return Err(AiRateLimitError::ConcurrencyLimit);
            }
            tokio::time::sleep(SLOT_POLL_INTERVAL).await;
        }

        if let Ok(mut guard) = self.reserved_input_tokens.lock() {
            *guard = Some(estimated);
        }
        Ok(())
    }

    async fn record_usage(&self, input_tokens: u64, output_tokens: u64) {
        let estimated = self.take_reservation().unwrap_or(0);
        self.rate_limiter
            .record_usage(self.user_id, input_tokens, output_tokens, estimated)
            .await;
        self.rate_limiter
            .release_concurrency_slot(self.user_id)
            .await;
    }

    async fn release(&self) {
        if let Some(estimated) = self.take_reservation() {
            self.rate_limiter
                .release_input_reservation(self.user_id, estimated)
                .await;
            self.rate_limiter
                .release_concurrency_slot(self.user_id)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[mockall_double::double]
    use dal::database_context::MyraDb;
    #[mockall_double::double]
    use dal::redis_connection::RedisConnection;

    use super::*;

    fn limiter(slot_counts: Vec<i64>, attempts: Arc<AtomicUsize>) -> UserRateLimiter {
        let mut redis = RedisConnection::default();
        redis
            .expect_execute_script_string()
            .returning(|_| Some("ok".to_string()));
        redis.expect_execute_script_int().returning(move |_| {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            Some(slot_counts[attempt.min(slot_counts.len() - 1)])
        });
        redis.expect_clone().returning(RedisConnection::default);
        UserRateLimiter::new(RateLimiter::new(redis, MyraDb::default()), Uuid::nil())
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiting_limiter_takes_the_slot_once_one_frees() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let limiter = limiter(vec![3, 3, 1], attempts.clone()).waiting_for_slot();

        assert!(limiter.pre_check(100).await.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiting_limiter_gives_up_after_the_limit() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let limiter = limiter(vec![3], attempts.clone()).waiting_for_slot();

        assert!(matches!(
            limiter.pre_check(100).await,
            Err(AiRateLimitError::ConcurrencyLimit)
        ));
        assert!(attempts.load(Ordering::SeqCst) > 1);
    }

    #[tokio::test]
    async fn a_limiter_that_does_not_wait_fails_at_once() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let limiter = limiter(vec![3], attempts.clone());

        assert!(matches!(
            limiter.pre_check(100).await,
            Err(AiRateLimitError::ConcurrencyLimit)
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
