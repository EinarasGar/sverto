#[cfg(feature = "backend")]
use business::dtos::imports::{
    import_classification_dto::{ImportClassificationDto, ImportClassificationStatusDto},
    import_file_dto::{ImportDateOrderDto, ImportFileDto, ImportFileStatusDto},
    import_session_dto::{
        ImportSessionCountsDto, ImportSessionDetailDto, ImportSessionDto, ImportSessionStatusDto,
    },
    import_source_dto::{ImportSourceBankAccountDto, ImportSourceDto, ImportSourceKindDto},
};
use macros::type_tag;
use serde::{Deserialize, Serialize};
use time::{serde::rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::view_models::imports::asks::IdentifiableImportAskViewModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportSessionStatus {
    Collecting,
    Parsing,
    Reconciling,
    NeedsInput,
    ProposalReady,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportFileStatus {
    Pending,
    Processing,
    Parsed,
    Failed,
}

/// How dates written in digits are read: `03/11` is 3 November day-first, 11 March month-first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportDateOrder {
    DayFirst,
    MonthFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportClassificationStatus {
    NotStarted,
    Waiting,
    Running,
    Classified,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportClassificationViewModel {
    pub status: ImportClassificationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableImportSessionResponseViewModel {
    pub id: Uuid,
    pub accounts: Vec<Uuid>,
    pub status: ImportSessionStatus,
    pub classification: ImportClassificationViewModel,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportSessionResponseViewModel {
    pub accounts: Vec<Uuid>,
    pub status: ImportSessionStatus,
    pub classification: ImportClassificationViewModel,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableImportFileViewModel {
    pub file_id: Uuid,
    pub status: ImportFileStatus,
    /// Set when the file failed to parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Set when the file paused on AI budget exhaustion rather than a real failure.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub paused_until: Option<OffsetDateTime>,
    /// Set when every date of the file reads both day-first and month-first and nothing in it
    /// settles which: the order they are read in. The file can be read again in the other one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_order: Option<ImportDateOrder>,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// How many ledger transactions this file is recorded on; zero until the import is applied.
    pub recorded_transactions: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case", untagged)]
#[schema(discriminator = "kind")]
pub enum IdentifiableImportSourceViewModel {
    File(ImportFileSource),
    BankAccount(ImportBankAccountSource),
}

#[type_tag(value = "file", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportFileSource {
    pub id: Uuid,
    pub file_id: Uuid,
    pub status: ImportFileStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub paused_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_order: Option<ImportDateOrder>,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    pub recorded_transactions: i64,
}

#[type_tag(value = "bank_account", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportBankAccountSource {
    pub id: Uuid,
    pub binding_id: Uuid,
    pub provider_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_number_last4: Option<String>,
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    pub recorded_transactions: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportSessionDetailResponseViewModel {
    pub session: ImportSessionResponseViewModel,
    pub conversation_id: Uuid,
    pub sources: Vec<IdentifiableImportSourceViewModel>,
    pub open_asks: Vec<IdentifiableImportAskViewModel>,
    pub counts: ImportSessionCountsViewModel,
}

/// What apply would do, summarized. `pending` rows are parsed but not yet
/// staged — they are unblocked by answering asks.
#[derive(Clone, Debug, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportSessionCountsViewModel {
    pub add: i64,
    pub amend: i64,
    pub delete: i64,
    pub skip: i64,
    pub pending: i64,
    pub unstageable: i64,
    /// False when the proposal could not be built; the staged counts are then zero.
    pub staged_available: bool,
}

#[cfg(feature = "backend")]
impl From<ImportSessionStatusDto> for ImportSessionStatus {
    fn from(value: ImportSessionStatusDto) -> Self {
        match value {
            ImportSessionStatusDto::Collecting => Self::Collecting,
            ImportSessionStatusDto::Parsing => Self::Parsing,
            ImportSessionStatusDto::Reconciling => Self::Reconciling,
            ImportSessionStatusDto::NeedsInput => Self::NeedsInput,
            ImportSessionStatusDto::ProposalReady => Self::ProposalReady,
            ImportSessionStatusDto::Completed => Self::Completed,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportClassificationStatusDto> for ImportClassificationStatus {
    fn from(value: ImportClassificationStatusDto) -> Self {
        match value {
            ImportClassificationStatusDto::NotStarted => Self::NotStarted,
            ImportClassificationStatusDto::Waiting => Self::Waiting,
            ImportClassificationStatusDto::Running => Self::Running,
            ImportClassificationStatusDto::Classified => Self::Classified,
            ImportClassificationStatusDto::Failed => Self::Failed,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportClassificationDto> for ImportClassificationViewModel {
    fn from(value: ImportClassificationDto) -> Self {
        Self {
            status: value.status.into(),
            error_message: value.error_message,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportSessionDto> for IdentifiableImportSessionResponseViewModel {
    fn from(value: ImportSessionDto) -> Self {
        Self {
            id: value.id,
            accounts: value.accounts,
            status: value.status.into(),
            classification: value.classification.into(),
            created_at: value.created_at,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportSessionDto> for ImportSessionResponseViewModel {
    fn from(value: ImportSessionDto) -> Self {
        Self {
            accounts: value.accounts,
            status: value.status.into(),
            classification: value.classification.into(),
            created_at: value.created_at,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportSessionCountsDto> for ImportSessionCountsViewModel {
    fn from(value: ImportSessionCountsDto) -> Self {
        Self {
            add: value.add,
            amend: value.amend,
            delete: value.delete,
            skip: value.skip,
            pending: value.pending,
            unstageable: value.unstageable,
            staged_available: value.staged_available,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportSessionDetailDto> for ImportSessionDetailResponseViewModel {
    fn from(value: ImportSessionDetailDto) -> Self {
        Self {
            conversation_id: value.session.conversation_id,
            session: value.session.into(),
            sources: value.sources.into_iter().map(Into::into).collect(),
            open_asks: value.open_asks.into_iter().map(Into::into).collect(),
            counts: value.counts.into(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportFileStatusDto> for ImportFileStatus {
    fn from(value: ImportFileStatusDto) -> Self {
        match value {
            ImportFileStatusDto::Pending => Self::Pending,
            ImportFileStatusDto::Processing => Self::Processing,
            ImportFileStatusDto::Parsed => Self::Parsed,
            ImportFileStatusDto::Failed => Self::Failed,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportDateOrderDto> for ImportDateOrder {
    fn from(value: ImportDateOrderDto) -> Self {
        match value {
            ImportDateOrderDto::DayFirst => Self::DayFirst,
            ImportDateOrderDto::MonthFirst => Self::MonthFirst,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportDateOrder> for ImportDateOrderDto {
    fn from(value: ImportDateOrder) -> Self {
        match value {
            ImportDateOrder::DayFirst => Self::DayFirst,
            ImportDateOrder::MonthFirst => Self::MonthFirst,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportFileDto> for IdentifiableImportFileViewModel {
    fn from(value: ImportFileDto) -> Self {
        Self {
            file_id: value.file_id,
            status: value.status.into(),
            error_message: value.error_message,
            paused_until: value.paused_until,
            date_order: value.date_order.map(Into::into),
            created_at: value.created_at,
            recorded_transactions: value.recorded_transactions,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportSourceDto> for IdentifiableImportSourceViewModel {
    fn from(value: ImportSourceDto) -> Self {
        match value.kind {
            ImportSourceKindDto::File(file) => Self::File(ImportFileSource {
                kind: Default::default(),
                id: value.id,
                file_id: file.file_id,
                status: file.status.into(),
                error_message: file.error_message,
                paused_until: file.paused_until,
                date_order: file.date_order.map(Into::into),
                created_at: value.created_at,
                recorded_transactions: value.recorded_transactions,
            }),
            ImportSourceKindDto::BankAccount(ImportSourceBankAccountDto {
                binding_id,
                provider_kind,
                display_name,
                currency,
                account_number_last4,
            }) => Self::BankAccount(ImportBankAccountSource {
                kind: Default::default(),
                id: value.id,
                binding_id,
                provider_kind,
                display_name,
                currency,
                account_number_last4,
                created_at: value.created_at,
                recorded_transactions: value.recorded_transactions,
            }),
        }
    }
}
