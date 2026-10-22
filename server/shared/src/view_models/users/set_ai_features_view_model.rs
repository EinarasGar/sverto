use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SetAiFeaturesRequestViewModel {
    pub enabled: bool,
}
