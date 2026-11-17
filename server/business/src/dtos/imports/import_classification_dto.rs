use dal::enums::transaction_types::DatabaseTransactionTypes;
use dal::models::import_models::{ImportFeeType, ImportParsedLegModel, ImportSessionSummaryModel};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportClassificationStatusDto {
    NotStarted,
    Waiting,
    Running,
    Classified,
    Failed,
}

impl From<&ImportSessionSummaryModel> for ImportClassificationStatusDto {
    fn from(summary: &ImportSessionSummaryModel) -> Self {
        if summary.classification_run_id.is_none() {
            Self::NotStarted
        } else if summary.classified_at.is_some() {
            Self::Classified
        } else if summary.classification_error.is_some() {
            Self::Failed
        } else if summary.pending_files + summary.processing_files > 0 {
            Self::Waiting
        } else {
            Self::Running
        }
    }
}

pub struct ImportClassificationDto {
    pub status: ImportClassificationStatusDto,
    pub error_message: Option<String>,
}

impl From<&ImportSessionSummaryModel> for ImportClassificationDto {
    fn from(summary: &ImportSessionSummaryModel) -> Self {
        Self {
            status: summary.into(),
            error_message: summary.classification_error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportParsedLineDto {
    pub id: Uuid,
    pub file_id: Uuid,
    pub transaction_key: Option<String>,
    pub account_ref: String,
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub related_asset_ref: Option<String>,
    pub legs: Vec<ImportParsedLegDto>,
}

#[derive(Debug, Clone)]
pub struct ImportParsedLegDto {
    pub asset_ref: String,
    pub quantity: Decimal,
    pub fee_type: Option<FeeEntryTypesDto>,
}

impl TryFrom<ImportParsedLegModel> for ImportParsedLegDto {
    type Error = anyhow::Error;

    fn try_from(leg: ImportParsedLegModel) -> Result<Self, Self::Error> {
        Ok(Self {
            asset_ref: leg.asset_ref,
            quantity: leg.quantity,
            fee_type: leg
                .fee_type
                .as_deref()
                .map(ImportFeeType::try_from)
                .transpose()?
                .map(Into::into),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportLineTypeDto {
    Regular,
    CashTransferOut,
    CashTransferIn,
    CashDividend,
    AssetTrade,
    AssetSale,
    AssetPurchase,
    AccountFees,
    AssetTransferIn,
    AssetTransferOut,
}

impl From<ImportLineTypeDto> for DatabaseTransactionTypes {
    fn from(value: ImportLineTypeDto) -> Self {
        match value {
            ImportLineTypeDto::Regular => Self::RegularTransaction,
            ImportLineTypeDto::CashTransferOut => Self::CashTransferOut,
            ImportLineTypeDto::CashTransferIn => Self::CashTransferIn,
            ImportLineTypeDto::CashDividend => Self::CashDividend,
            ImportLineTypeDto::AssetTrade => Self::AssetTrade,
            ImportLineTypeDto::AssetSale => Self::AssetSale,
            ImportLineTypeDto::AssetPurchase => Self::AssetPurchase,
            ImportLineTypeDto::AccountFees => Self::AccountFees,
            ImportLineTypeDto::AssetTransferIn => Self::AssetTransferIn,
            ImportLineTypeDto::AssetTransferOut => Self::AssetTransferOut,
        }
    }
}

impl TryFrom<DatabaseTransactionTypes> for ImportLineTypeDto {
    type Error = anyhow::Error;

    fn try_from(value: DatabaseTransactionTypes) -> Result<Self, Self::Error> {
        Ok(match value {
            DatabaseTransactionTypes::RegularTransaction => Self::Regular,
            DatabaseTransactionTypes::CashTransferOut => Self::CashTransferOut,
            DatabaseTransactionTypes::CashTransferIn => Self::CashTransferIn,
            DatabaseTransactionTypes::CashDividend => Self::CashDividend,
            DatabaseTransactionTypes::AssetTrade => Self::AssetTrade,
            DatabaseTransactionTypes::AssetSale => Self::AssetSale,
            DatabaseTransactionTypes::AssetPurchase => Self::AssetPurchase,
            DatabaseTransactionTypes::AccountFees => Self::AccountFees,
            DatabaseTransactionTypes::AssetTransferIn => Self::AssetTransferIn,
            DatabaseTransactionTypes::AssetTransferOut => Self::AssetTransferOut,
            other => anyhow::bail!("Classification never assigns {other:?}"),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct ImportLineTypingDto {
    pub line_type: ImportLineTypeDto,
    pub category_id: Option<i32>,
    pub line_ids: Vec<Uuid>,
}
