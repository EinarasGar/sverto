use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CategoryResult {
    pub id: i32,
    pub category: String,
    pub category_type: String,
    pub icon: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Currency,
    Stock,
    Bond,
    MutualFund,
    Etf,
    Etc,
    Crypto,
    RealEstate,
    Vehicle,
}

#[derive(Serialize)]
pub struct AssetResult {
    pub id: i32,
    pub asset_name: String,
    pub ticker: String,
    pub asset_type: AssetType,
    pub held: bool,
}

#[derive(Serialize)]
pub struct AssetSearchResult {
    pub matches: Vec<AssetResult>,
    pub similar: Vec<AssetResult>,
}

pub struct AssetSearchParams {
    pub query: String,
    pub asset_type: Option<AssetType>,
    pub query_vector: Option<Vec<f64>>,
    pub max_matches: u64,
    pub max_similar: u64,
}
