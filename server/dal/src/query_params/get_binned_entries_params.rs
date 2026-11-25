use sqlx::types::{time::OffsetDateTime, Uuid};
use time::Duration;

pub struct GetBinnedEntriesParams {
    pub start_date: Option<OffsetDateTime>,
    pub interval: Duration,
    pub user_id: Uuid,
    pub account_id: Option<Uuid>,
    pub asset_id: Option<i32>,
    pub excluded_account_types: Vec<i32>,
    pub apply_ownership_share: bool,
}
