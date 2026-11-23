#[cfg(feature = "backend")]
use business::dtos::connectors::HeldTransactionWithCandidatesDto;
#[cfg(feature = "backend")]
use business::dtos::entry_dto::EntryDto;
use serde::{Deserialize, Serialize};
use time::{serde::rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::view_models::transactions::base_models::account_asset_entry::TransactionEntry;
use crate::view_models::transactions::base_models::metadata_lookup::MetadataLookupTables;
use crate::view_models::transactions::transaction_types::TransactionWithId;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableHeldTransactionViewModel {
    pub id: Uuid,
    pub provider_kind: String,
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub description: String,
    #[serde(flatten)]
    pub entry: TransactionEntry,
    pub candidates: Vec<TransactionWithId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GetHeldTransactionsResponseViewModel {
    pub held_transactions: Vec<IdentifiableHeldTransactionViewModel>,
    pub lookup_tables: MetadataLookupTables,
}

#[cfg(feature = "backend")]
impl From<HeldTransactionWithCandidatesDto> for IdentifiableHeldTransactionViewModel {
    fn from(dto: HeldTransactionWithCandidatesDto) -> Self {
        let held = dto.held;
        Self {
            id: held.id,
            provider_kind: held.provider_kind,
            date: held.date_transacted,
            description: held.description,
            entry: EntryDto::new(held.asset_id, held.account_id, held.quantity).into(),
            candidates: dto.candidates.into_iter().map(Into::into).collect(),
        }
    }
}
