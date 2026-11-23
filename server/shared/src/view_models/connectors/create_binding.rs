use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::create_import::ConnectorImportFrom;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBindingRequestViewModel {
    pub sverto_account_id: uuid::Uuid,
    pub provider_account_id: Option<String>,
    pub import_from: ConnectorImportFrom,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBindingResponseViewModel {
    pub binding_id: uuid::Uuid,
}
