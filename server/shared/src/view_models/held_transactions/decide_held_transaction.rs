#[cfg(feature = "backend")]
use business::dtos::connectors::HeldDecisionDto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum DecideHeldTransactionRequestViewModel {
    Same { transaction_id: Uuid },
    Different,
}

#[cfg(feature = "backend")]
impl From<DecideHeldTransactionRequestViewModel> for HeldDecisionDto {
    fn from(request: DecideHeldTransactionRequestViewModel) -> Self {
        match request {
            DecideHeldTransactionRequestViewModel::Same { transaction_id } => {
                Self::Same { transaction_id }
            }
            DecideHeldTransactionRequestViewModel::Different => Self::Different,
        }
    }
}
