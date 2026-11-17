#[cfg(feature = "backend")]
use business::dtos::imports::import_pending_dto::{ImportPendingLegDto, ImportPendingLineDto};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::view_models::transactions::base_models::transaction_fee::TransactionFeeType;

/// A parsed leg awaiting asset resolution. Raw reference on purpose — the
/// row is not a typed transaction until every reference resolves.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PendingLeg {
    pub asset_ref: String,
    /// Signed: negative leaves the account.
    pub quantity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_type: Option<TransactionFeeType>,
}

/// A parsed row that cannot be staged yet — an asset, account, or
/// interpretation it depends on is unresolved. Read-only: it is unblocked by
/// answering the asks in `blocked_by`, after which it moves to the staged
/// items list.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableImportPendingTransactionViewModel {
    pub id: Uuid,
    pub file_id: Uuid,
    pub account_ref: String,
    #[serde(with = "time::serde::rfc3339")]
    pub date: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub legs: Vec<PendingLeg>,
    /// Ask keys that must be answered before this row can be staged.
    pub blocked_by: Vec<String>,
}

#[cfg(feature = "backend")]
impl From<ImportPendingLegDto> for PendingLeg {
    fn from(value: ImportPendingLegDto) -> Self {
        Self {
            asset_ref: value.asset_ref,
            quantity: value.quantity.to_string(),
            fee_type: value.fee_type.map(Into::into),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportPendingLineDto> for IdentifiableImportPendingTransactionViewModel {
    fn from(value: ImportPendingLineDto) -> Self {
        Self {
            id: value.id,
            file_id: value.file_id,
            account_ref: value.account_ref,
            date: value.date,
            description: value.description,
            legs: value.legs.into_iter().map(Into::into).collect(),
            blocked_by: value.blocked_by,
        }
    }
}
