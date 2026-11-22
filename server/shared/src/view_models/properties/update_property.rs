#[cfg(feature = "backend")]
use business::dtos::properties::update_property_dto::UpdatePropertyDto;
use serde::{Deserialize, Serialize};

use crate::view_models::accounts::base_models::account_name::AccountName;
use crate::view_models::accounts::base_models::ownership_share::OwnershipShare;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdatePropertyViewModel {
    #[schema(example = "Flat 12")]
    pub name: AccountName,
    pub ownership_share: OwnershipShare,
}

#[cfg(feature = "backend")]
impl From<UpdatePropertyViewModel> for UpdatePropertyDto {
    fn from(request: UpdatePropertyViewModel) -> Self {
        Self {
            name: request.name.into_inner(),
            ownership_share: request.ownership_share.as_decimal(),
        }
    }
}
