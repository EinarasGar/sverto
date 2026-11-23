use crate::models::balance::ProviderBalance;
use crate::models::sync::{FetchedPage, RateLimited, RawPage, SyncCursor};
use crate::models::transaction::ProviderTransaction;
use crate::provider::ProviderKind;
use async_trait::async_trait;
use time::OffsetDateTime;

const SYNC_LOOKBACK_DAYS: i64 = 3;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ConnectorStore: Send + Sync {
    fn provider_kind(&self) -> ProviderKind;

    fn provider_key_id(&self) -> Option<String>;

    async fn latest_cursor(&self) -> anyhow::Result<Option<serde_json::Value>>;

    async fn append_page(&self, page: &FetchedPage) -> anyhow::Result<()>;

    async fn get_credential(&self) -> anyhow::Result<Option<Vec<u8>>>;

    async fn put_credential(&self, value: &[u8]) -> anyhow::Result<()>;

    async fn cache_get(&self, key: &str) -> Option<String>;

    async fn cache_put(&self, key: &str, value: &str, ttl_secs: u64);

    async fn cache_lock(&self, key: &str, ttl_secs: u64) -> bool;

    async fn cache_unlock(&self, key: &str);
}

#[derive(Debug, Clone, Copy)]
pub struct SyncParams {
    pub synced_through: Option<OffsetDateTime>,
    pub budget: Option<time::Duration>,
    pub inline_retries: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderCapabilities {
    pub max_history: Option<time::Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncRunOutcome {
    Complete {
        pages_fetched: i32,
    },
    Partial {
        pages_fetched: i32,
        next_cursor: Option<serde_json::Value>,
    },
}

#[async_trait]
pub trait Connector: Send + Sync + 'static {
    async fn fetch_page(
        &self,
        from: Option<OffsetDateTime>,
        cursor: Option<SyncCursor>,
    ) -> anyhow::Result<FetchedPage>;

    async fn fetch_balance(&self) -> anyhow::Result<ProviderBalance>;

    fn map_pages(&self, pages: &[RawPage]) -> Vec<ProviderTransaction>;

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn sync(
        &self,
        store: &dyn ConnectorStore,
        params: SyncParams,
    ) -> anyhow::Result<SyncRunOutcome> {
        let started_at = OffsetDateTime::now_utc();
        let mut pages_fetched: i32 = 0;

        let from = self.sync_window_from(params.synced_through, started_at);

        let mut cursor: Option<SyncCursor> = store
            .latest_cursor()
            .await?
            .and_then(|v| serde_json::from_value(v).ok());

        loop {
            if params
                .budget
                .is_some_and(|budget| OffsetDateTime::now_utc() >= started_at + budget)
            {
                return Ok(partial(pages_fetched, cursor));
            }

            let page =
                match fetch_page_with_retries(self, from, cursor.clone(), params.inline_retries)
                    .await
                {
                    Err(e) if e.is::<RateLimited>() => return Ok(partial(pages_fetched, cursor)),
                    page => page?,
                };
            store.append_page(&page).await?;
            pages_fetched += 1;

            match page.next_cursor {
                None => return Ok(SyncRunOutcome::Complete { pages_fetched }),
                Some(next) => cursor = Some(next),
            }
        }
    }

    fn sync_window_from(
        &self,
        synced_through: Option<OffsetDateTime>,
        now: OffsetDateTime,
    ) -> Option<OffsetDateTime> {
        let desired =
            synced_through.map(|through| through - time::Duration::days(SYNC_LOOKBACK_DAYS));
        let floor = self.capabilities().max_history.map(|max| now - max);

        match (desired, floor) {
            (None, None) => None,
            (None, Some(floor)) => {
                tracing::warn!(
                    floor = %floor,
                    "first sync bounded by provider max_history — older history is unreachable"
                );
                Some(floor)
            }
            (Some(desired), None) => Some(desired),
            (Some(desired), Some(floor)) => {
                if desired < floor {
                    tracing::warn!(
                        desired = %desired,
                        floor = %floor,
                        "sync window clamped to provider max_history — older history is unreachable"
                    );
                    Some(floor)
                } else {
                    Some(desired)
                }
            }
        }
    }
}

fn partial(pages_fetched: i32, cursor: Option<SyncCursor>) -> SyncRunOutcome {
    SyncRunOutcome::Partial {
        pages_fetched,
        next_cursor: cursor.and_then(|c| serde_json::to_value(c).ok()),
    }
}

async fn fetch_page_with_retries<C: Connector + ?Sized>(
    connector: &C,
    from: Option<OffsetDateTime>,
    cursor: Option<SyncCursor>,
    retries: u32,
) -> anyhow::Result<FetchedPage> {
    let mut last_err = None;
    for attempt in 0..=retries {
        match connector.fetch_page(from, cursor.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) if e.is::<RateLimited>() => return Err(e),
            Err(e) => {
                tracing::warn!(attempt, error = %e, "sync page fetch failed");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.expect("loop runs at least once"))
}
