use dal::models::import_models::{
    ImportLineClassificationModel, ImportStagedDecisionModel, ImportVerb,
};
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use super::import_balance_dto::{
    ImportAdjustmentDto, ImportBalanceChecksDto, ImportStartingBalanceDto,
};
use super::import_classification_dto::ImportLineTypeDto;
use crate::dtos::ledger_matching_dto::MatchCandidateDto;
use crate::dtos::transaction_dto::TransactionDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportVerbDto {
    Add,
    Amend,
    Delete,
    Skip,
}

impl From<ImportVerb> for ImportVerbDto {
    fn from(value: ImportVerb) -> Self {
        match value {
            ImportVerb::Add => Self::Add,
            ImportVerb::Amend => Self::Amend,
            ImportVerb::Delete => Self::Delete,
            ImportVerb::Skip => Self::Skip,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportVerbPinDto {
    Skip,
    Add,
    Auto,
    Same { candidate_id: Uuid },
    Apart,
}

#[derive(Debug, Clone)]
pub struct ImportLineClassificationDto {
    pub parsed_line_id: Uuid,
    pub line_type: ImportLineTypeDto,
    pub category_id: Option<i32>,
}

impl TryFrom<ImportLineClassificationModel> for ImportLineClassificationDto {
    type Error = anyhow::Error;

    fn try_from(model: ImportLineClassificationModel) -> Result<Self, Self::Error> {
        Ok(Self {
            parsed_line_id: model.parsed_line_id,
            line_type: model.type_id.try_into()?,
            category_id: model.category_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ImportStagedDecisionDto {
    pub id: Uuid,
    pub ask_key: Option<String>,
    pub verb: ImportVerbDto,
    pub transaction_id: Option<Uuid>,
}

impl TryFrom<ImportStagedDecisionModel> for ImportStagedDecisionDto {
    type Error = anyhow::Error;

    fn try_from(model: ImportStagedDecisionModel) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            ask_key: model.ask_key,
            verb: ImportVerb::try_from(model.verb.as_str())?.into(),
            transaction_id: model.transaction_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedMovementDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub source_id: Uuid,
    pub date: Date,
    pub description: Option<String>,
    pub itemised_fee: Option<Decimal>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StagedTransactionDto {
    pub id: Uuid,
    pub source_ids: Vec<Uuid>,
    pub transaction: TransactionDto,
    pub movements: Vec<StagedMovementDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnstageableGroupDto {
    pub id: Uuid,
    pub source_ids: Vec<Uuid>,
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportNearMissCandidateDto {
    pub candidate: MatchCandidateDto,
    pub amend: bool,
    pub parts: Vec<MatchCandidateDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportClaimDto {
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub booked_on: Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRecordedOnDto {
    Transaction(Uuid),
    Row(Uuid),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportRowMatchDto {
    pub verb: ImportVerbDto,
    pub transaction_id: Option<Uuid>,
    pub claims: Vec<ImportClaimDto>,
    pub near_miss: Vec<ImportNearMissCandidateDto>,
    pub recorded_on: Vec<Option<ImportRecordedOnDto>>,
}

impl Default for ImportRowMatchDto {
    fn default() -> Self {
        Self {
            verb: ImportVerbDto::Add,
            transaction_id: None,
            claims: Vec::new(),
            near_miss: Vec::new(),
            recorded_on: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportRowKindDto {
    Movement,
    StartingBalance(ImportStartingBalanceDto),
    JoinedTransfer([Uuid; 2]),
    Adjustment(ImportAdjustmentDto),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAmendRefusalDto {
    TargetGone,
    NotOneEntry,
    Unreadable,
}

#[derive(Debug, Clone)]
pub struct ImportProposalRowDto {
    pub id: Uuid,
    pub kind: ImportRowKindDto,
    pub source_ids: Vec<Uuid>,
    pub ask_key: Option<String>,
    pub verb: ImportVerbDto,
    pub verb_locked: bool,
    pub transaction: TransactionDto,
    pub near_miss: Vec<ImportNearMissCandidateDto>,
    pub movements: Vec<StagedMovementDto>,
    pub claims: Vec<ImportClaimDto>,
    pub recorded_on: Vec<Option<ImportRecordedOnDto>>,
    pub amend_refusal: Option<ImportAmendRefusalDto>,
    pub parts: Option<ImportPartsDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPartsDto {
    pub transaction_ids: Vec<Uuid>,
    pub description: String,
    pub category_id: i32,
}

#[derive(Debug, Clone)]
pub struct ImportProposalDto {
    pub rows: Vec<ImportProposalRowDto>,
    pub unstageable: Vec<UnstageableGroupDto>,
    pub fingerprint: Uuid,
    pub balance_checks: Option<ImportBalanceChecksDto>,
    pub read_as_owed: Vec<Uuid>,
}
