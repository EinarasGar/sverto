use pgvector::Vector;
use sqlx::types::Uuid;

pub struct SearchVisibleAssetsParams {
    pub user_id: Uuid,
    pub search_type: SearchVisibleAssetsType,
    pub asset_type: Option<i32>,
    pub limit: u64,
}

pub enum SearchVisibleAssetsType {
    ByText(String),
    ByEmbedding {
        embedding: Vector,
        excluded_ids: Vec<i32>,
    },
}

impl SearchVisibleAssetsParams {
    pub fn by_text(user_id: Uuid, text: String, limit: u64) -> Self {
        Self {
            user_id,
            search_type: SearchVisibleAssetsType::ByText(text),
            asset_type: None,
            limit,
        }
    }

    pub fn by_embedding(
        user_id: Uuid,
        embedding: Vector,
        excluded_ids: Vec<i32>,
        limit: u64,
    ) -> Self {
        Self {
            user_id,
            search_type: SearchVisibleAssetsType::ByEmbedding {
                embedding,
                excluded_ids,
            },
            asset_type: None,
            limit,
        }
    }

    pub fn with_asset_type(mut self, asset_type: Option<i32>) -> Self {
        self.asset_type = asset_type;
        self
    }
}
