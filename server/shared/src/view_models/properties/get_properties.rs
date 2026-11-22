use serde::{Deserialize, Serialize};

use super::base_models::property::IdentifiablePropertyViewModel;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GetPropertiesResponseViewModel {
    pub properties: Vec<IdentifiablePropertyViewModel>,
}
