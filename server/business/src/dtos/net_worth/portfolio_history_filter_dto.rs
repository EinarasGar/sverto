use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default)]
pub struct PortfolioHistoryFilterDto {
    pub account_id: Option<Uuid>,
    pub asset_id: Option<i32>,
    pub exclude_liabilities: bool,
}
