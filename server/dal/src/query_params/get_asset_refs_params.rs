use sqlx::types::Uuid;

pub struct GetAssetRefsParams {
    pub user_id: Uuid,
    pub search_type: GetAssetRefsSearchType,
    pub asset_type: Option<i32>,
}

pub enum GetAssetRefsSearchType {
    ByTickers(Vec<String>),
    ByTickersSymbolsIsinsOrIds { refs: Vec<String>, ids: Vec<i32> },
}

impl GetAssetRefsParams {
    pub fn by_tickers(user_id: Uuid, tickers: Vec<String>) -> Self {
        Self {
            user_id,
            search_type: GetAssetRefsSearchType::ByTickers(tickers),
            asset_type: None,
        }
    }

    pub fn by_tickers_symbols_isins_or_ids(
        user_id: Uuid,
        refs: Vec<String>,
        ids: Vec<i32>,
    ) -> Self {
        Self {
            user_id,
            search_type: GetAssetRefsSearchType::ByTickersSymbolsIsinsOrIds { refs, ids },
            asset_type: None,
        }
    }

    pub fn with_asset_type(mut self, asset_type: i32) -> Self {
        self.asset_type = Some(asset_type);
        self
    }
}
