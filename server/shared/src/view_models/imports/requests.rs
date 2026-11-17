#[cfg(feature = "backend")]
use business::dtos::imports::import_staging_dto::ImportVerbPinDto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::view_models::imports::session::ImportDateOrder;
use crate::view_models::imports::staged::ImportItemVerbOverride;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttachImportFilesRequestViewModel {
    pub file_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ReadImportFileDatesRequestViewModel {
    pub date_order: ImportDateOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApplyImportSessionRequestViewModel {
    /// The `fingerprint` of the items the user reviewed.
    pub fingerprint: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SetImportItemsVerbRequestViewModel {
    pub item_ids: Vec<Uuid>,
    pub verb: ImportItemVerbOverride,
    /// With `skip` and a single row: the suggested transaction that row is the same as.
    #[serde(default)]
    pub candidate_id: Option<Uuid>,
}

#[cfg(feature = "backend")]
impl From<&SetImportItemsVerbRequestViewModel> for ImportVerbPinDto {
    fn from(value: &SetImportItemsVerbRequestViewModel) -> Self {
        match value.candidate_id {
            Some(candidate_id) => Self::Same { candidate_id },
            None => value.verb.into(),
        }
    }
}
