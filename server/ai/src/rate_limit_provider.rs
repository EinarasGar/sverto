//! Rate-limit policy abstraction. The `Conversation` wrapper calls into this
//! before each turn (to gate admission) and after each turn (to record actual
//! token usage). The implementation lives in the business crate; the ai
//! crate doesn't know how quotas are tracked or what counts as a slot.
//!
//! Lifecycle for one turn:
//! 2. *(LLM run happens)*
//!    (releases the slot, refunds the reservation).

use thiserror::Error;
use time::OffsetDateTime;

use crate::models::error::LimitScope;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Per-request input token cap exceeded")]
    PerRequestCap,

    #[error("Daily token quota exceeded")]
    QuotaExceeded {
        scope: LimitScope,
        reset_at: Option<OffsetDateTime>,
    },

    #[error("Concurrent request limit reached")]
    ConcurrencyLimit,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub trait RateLimitProvider: Send + Sync + 'static {
    fn pre_check(
        &self,
        estimated_input_tokens: u64,
    ) -> impl std::future::Future<Output = Result<(), RateLimitError>> + Send;

    fn record_usage(
        &self,
        input_tokens: u64,
        output_tokens: u64,
    ) -> impl std::future::Future<Output = ()> + Send;

    fn release(&self) -> impl std::future::Future<Output = ()> + Send;
}
