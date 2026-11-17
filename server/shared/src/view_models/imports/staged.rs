#[cfg(feature = "backend")]
use business::dtos::imports::import_staging_dto::{
    ImportAmendRefusalDto, ImportNearMissCandidateDto, ImportProposalRowDto, ImportRowKindDto,
    ImportVerbDto, ImportVerbPinDto, UnstageableGroupDto,
};
use serde::{Deserialize, Serialize};
use time::{serde::rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::view_models::imports::balance_checks::ImportBalanceChecksViewModel;
use crate::view_models::transactions::base_models::metadata_lookup::MetadataLookupTables;
use crate::view_models::transactions::transaction_types::TransactionWithId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportItemVerb {
    Add,
    Amend,
    Delete,
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportItemKind {
    /// A movement the files list, or a ledger transaction one of them replaces.
    Movement,
    /// What the account held before the import's first row, taken from a statement's balance
    /// because the ledger has nothing that early. Written as a transfer in or out.
    StartingBalance,
    /// Money out of one of the user's accounts and into another, joined from a row on each.
    /// `apart` imports each row on its own instead.
    JoinedTransfer,
    /// A transfer in or out that closes a difference between two statement balances no ledger
    /// transaction explains. Left out (`skip`) unless pinned `add`.
    Adjustment,
}

/// Why the ledger transaction an `amend` row matched can't take the statement's amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportAmendRefusal {
    /// The transaction was deleted after it was matched.
    TargetGone,
    /// It doesn't have exactly one entry on the row's account and asset.
    NotOneEntry,
    Unreadable,
}

/// The user's pin for staged rows. `auto` releases the rows back to the
/// engine's own derivation. Amend and delete are not pinnable — their targets
/// are chosen by matching and confirmed through asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportItemVerbOverride {
    Auto,
    Skip,
    Add,
    /// For a `joined_transfer`: add its two rows each on its own.
    Apart,
}

/// One of the transactions a `parts` candidate stands for.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportNearMissPartViewModel {
    pub id: Uuid,
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub quantity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportNearMissCandidateViewModel {
    pub id: Uuid,
    /// False when the match is another row of this import rather than a recorded transaction.
    pub in_ledger: bool,
    /// True when a bank feed or statement has already reported this transaction.
    pub confirmed: bool,
    /// True when picking this one replaces its amount with the statement's.
    pub amend: bool,
    /// True when the transaction has no entry for another movement of this row, so picking it
    /// leaves that movement unrecorded.
    pub leaves_movement_unrecorded: bool,
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub account_id: Option<Uuid>,
    pub asset_id: i32,
    pub quantity: String,
    /// Transactions typed one by one that together are this row's payment. Picking the
    /// candidate groups them when the import is applied.
    pub parts: Vec<ImportNearMissPartViewModel>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableStagedTransactionViewModel {
    pub id: Uuid,
    pub kind: ImportItemKind,
    /// Files the row's evidence came from. Several when lines from different
    /// files compose one transaction; empty for rows created by answering an ask.
    pub file_ids: Vec<Uuid>,
    pub verb: ImportItemVerb,
    pub verb_locked: bool,
    /// Transactions this row may duplicate, best first. Empty unless the match
    /// was too uncertain to decide alone.
    pub near_miss_candidates: Vec<ImportNearMissCandidateViewModel>,
    /// Set on an `amend` row that can't be written. Apply refuses the import until the row's
    /// pin is released.
    pub amend_refusal: Option<ImportAmendRefusal>,
    /// Ledger transactions typed one by one that the user said are this row's payment. Apply
    /// groups them; `auto` takes the answer back.
    pub parts: Vec<Uuid>,
    /// On a `delete` row, the `not_on_statement` answer that made it; answering `keep` on this key
    /// removes the row.
    pub ask_key: Option<String>,
    /// The typed transaction this row will write to the ledger; for `delete`
    /// rows, the ledger transaction being removed. Its `transaction_id` is the
    /// target for `amend`/`delete`, the matched transaction for `skip`, and
    /// empty for `add` until apply writes it.
    pub transaction: TransactionWithId,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportUnstageableGroupViewModel {
    pub id: Uuid,
    pub file_ids: Vec<Uuid>,
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportItemsResponseViewModel {
    /// False until the session's lines are classified, including after a failed
    /// classification; the list is then empty.
    pub ready: bool,
    pub items: Vec<IdentifiableStagedTransactionViewModel>,
    /// Absent while questions are open, because the lines they hold back would read as missing.
    pub balance_checks: Option<ImportBalanceChecksViewModel>,
    pub unstageable: Vec<ImportUnstageableGroupViewModel>,
    pub lookup_tables: MetadataLookupTables,
    /// Identifies this exact proposal; apply sends it back and is refused if the proposal changed.
    pub fingerprint: Option<Uuid>,
    /// Credit card, mortgage and loan accounts with a statement that printed what is owed as a
    /// positive amount: its amounts and balances are read the other way round.
    pub read_as_owed: Vec<Uuid>,
}

#[cfg(feature = "backend")]
impl From<ImportVerbDto> for ImportItemVerb {
    fn from(value: ImportVerbDto) -> Self {
        match value {
            ImportVerbDto::Add => Self::Add,
            ImportVerbDto::Amend => Self::Amend,
            ImportVerbDto::Delete => Self::Delete,
            ImportVerbDto::Skip => Self::Skip,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportRowKindDto> for ImportItemKind {
    fn from(value: ImportRowKindDto) -> Self {
        match value {
            ImportRowKindDto::Movement => Self::Movement,
            ImportRowKindDto::StartingBalance(_) => Self::StartingBalance,
            ImportRowKindDto::JoinedTransfer(_) => Self::JoinedTransfer,
            ImportRowKindDto::Adjustment(_) => Self::Adjustment,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportAmendRefusalDto> for ImportAmendRefusal {
    fn from(value: ImportAmendRefusalDto) -> Self {
        match value {
            ImportAmendRefusalDto::TargetGone => Self::TargetGone,
            ImportAmendRefusalDto::NotOneEntry => Self::NotOneEntry,
            ImportAmendRefusalDto::Unreadable => Self::Unreadable,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportItemVerbOverride> for ImportVerbPinDto {
    fn from(value: ImportItemVerbOverride) -> Self {
        match value {
            ImportItemVerbOverride::Auto => Self::Auto,
            ImportItemVerbOverride::Skip => Self::Skip,
            ImportItemVerbOverride::Add => Self::Add,
            ImportItemVerbOverride::Apart => Self::Apart,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportNearMissCandidateDto> for ImportNearMissCandidateViewModel {
    fn from(near: ImportNearMissCandidateDto) -> Self {
        Self {
            id: near.candidate.id,
            in_ledger: near.candidate.in_ledger,
            confirmed: near.candidate.confirmed,
            amend: near.amend,
            leaves_movement_unrecorded: near.candidate.leaves_movement_unrecorded,
            date: near.candidate.date.midnight().assume_utc(),
            description: near.candidate.description,
            account_id: near.candidate.account_id,
            asset_id: near.candidate.asset_id,
            quantity: near.candidate.quantity.normalize().to_string(),
            parts: near
                .parts
                .into_iter()
                .map(|part| ImportNearMissPartViewModel {
                    id: part.id,
                    date: part.date.midnight().assume_utc(),
                    description: part.description,
                    quantity: part.quantity.normalize().to_string(),
                })
                .collect(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportProposalRowDto> for IdentifiableStagedTransactionViewModel {
    fn from(row: ImportProposalRowDto) -> Self {
        Self {
            id: row.id,
            kind: row.kind.into(),
            file_ids: row.file_ids,
            verb: row.verb.into(),
            verb_locked: row.verb_locked,
            near_miss_candidates: row.near_miss.into_iter().map(Into::into).collect(),
            amend_refusal: row.amend_refusal.map(Into::into),
            parts: row
                .parts
                .map(|parts| parts.transaction_ids)
                .unwrap_or_default(),
            ask_key: row.ask_key,
            transaction: row.transaction.into(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<UnstageableGroupDto> for ImportUnstageableGroupViewModel {
    fn from(group: UnstageableGroupDto) -> Self {
        Self {
            id: group.id,
            file_ids: group.file_ids,
            date: group.date,
            description: group.description,
            reason: group.reason,
        }
    }
}
