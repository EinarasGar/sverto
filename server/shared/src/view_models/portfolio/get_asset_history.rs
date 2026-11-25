use serde::{Deserialize, Serialize};

use crate::view_models::assets::base_models::asset_id::RequiredAssetId;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(default)]
pub struct GetPortfolioAssetHistoryRequestParams {
    #[param(default = "1d", pattern = "^(1d|1w|1m|3m|6m|1y|all)$")]
    /// The range time for which to retrieve the sums for
    pub range: String,

    #[param(default = "From user settings.")]
    /// The asset to value the holding in. If not provided, the default asset id from the user will be used. Passing the asset itself returns units held
    pub default_asset_id: Option<RequiredAssetId>,

    /// Leave out credit card, mortgage and loan accounts
    pub exclude_liabilities: bool,
}

impl Default for GetPortfolioAssetHistoryRequestParams {
    fn default() -> Self {
        Self {
            range: "1d".to_string(),
            default_asset_id: None,
            exclude_liabilities: false,
        }
    }
}
