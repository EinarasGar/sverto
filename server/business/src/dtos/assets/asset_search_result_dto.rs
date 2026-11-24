use dal::models::asset_models::VisibleAsset;

#[derive(Clone, Debug)]
pub struct AssetSearchResultDto {
    pub asset_id: i32,
    pub name: String,
    pub ticker: String,
    pub asset_type: i32,
    pub held: bool,
}

impl From<VisibleAsset> for AssetSearchResultDto {
    fn from(asset: VisibleAsset) -> Self {
        Self {
            asset_id: asset.id,
            name: asset.asset_name,
            ticker: asset.ticker,
            asset_type: asset.asset_type,
            held: asset.held,
        }
    }
}
