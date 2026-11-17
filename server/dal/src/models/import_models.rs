use rust_decimal::Decimal;
use sqlx::types::Uuid;
use std::fmt;
use time::{Date, OffsetDateTime};

use crate::enums::transaction_types::DatabaseTransactionTypes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSessionLifecycle {
    Active,
    Completed,
}

impl ImportSessionLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
        }
    }
}

impl fmt::Display for ImportSessionLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for ImportSessionLifecycle {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            other => Err(anyhow::anyhow!("Unknown import session lifecycle: {other}")),
        }
    }
}

impl From<ImportSessionLifecycle> for sea_query::Value {
    fn from(value: ImportSessionLifecycle) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFileStatus {
    Pending,
    Processing,
    Parsed,
    Failed,
}

impl ImportFileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Parsed => "parsed",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for ImportFileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for ImportFileStatus {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "parsed" => Ok(Self::Parsed),
            "failed" => Ok(Self::Failed),
            other => Err(anyhow::anyhow!("Unknown import file status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDateOrder {
    DayFirst,
    MonthFirst,
}

impl ImportDateOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DayFirst => "day_first",
            Self::MonthFirst => "month_first",
        }
    }
}

impl TryFrom<&str> for ImportDateOrder {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "day_first" => Ok(Self::DayFirst),
            "month_first" => Ok(Self::MonthFirst),
            other => Err(anyhow::anyhow!("Unknown date order: {other}")),
        }
    }
}

impl From<ImportDateOrder> for sea_query::Value {
    fn from(value: ImportDateOrder) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

impl From<ImportFileStatus> for sea_query::Value {
    fn from(value: ImportFileStatus) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportVerb {
    Add,
    Amend,
    Delete,
    Skip,
}

impl ImportVerb {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Amend => "amend",
            Self::Delete => "delete",
            Self::Skip => "skip",
        }
    }
}

impl fmt::Display for ImportVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for ImportVerb {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "add" => Ok(Self::Add),
            "amend" => Ok(Self::Amend),
            "delete" => Ok(Self::Delete),
            "skip" => Ok(Self::Skip),
            other => Err(anyhow::anyhow!("Unknown import verb: {other}")),
        }
    }
}

impl From<ImportVerb> for sea_query::Value {
    fn from(value: ImportVerb) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFeeType {
    Transaction,
    Exchange,
    WithholdingTax,
}

impl ImportFeeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Transaction => "transaction",
            Self::Exchange => "exchange",
            Self::WithholdingTax => "withholding_tax",
        }
    }
}

impl TryFrom<&str> for ImportFeeType {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "transaction" => Ok(Self::Transaction),
            "exchange" => Ok(Self::Exchange),
            "withholding_tax" => Ok(Self::WithholdingTax),
            other => Err(anyhow::anyhow!("Unknown import fee type: {other}")),
        }
    }
}

impl From<ImportFeeType> for sea_query::Value {
    fn from(value: ImportFeeType) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAssertionKind {
    Opening,
    Closing,
    Snapshot,
    ValueTotal,
}

impl ImportAssertionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Closing => "closing",
            Self::Snapshot => "snapshot",
            Self::ValueTotal => "value_total",
        }
    }
}

impl TryFrom<&str> for ImportAssertionKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "opening" => Ok(Self::Opening),
            "closing" => Ok(Self::Closing),
            "snapshot" => Ok(Self::Snapshot),
            "value_total" => Ok(Self::ValueTotal),
            other => Err(anyhow::anyhow!("Unknown import assertion kind: {other}")),
        }
    }
}

impl From<ImportAssertionKind> for sea_query::Value {
    fn from(value: ImportAssertionKind) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

pub struct ImportParsedLineInsert {
    pub id: Uuid,
    pub session_id: Uuid,
    pub file_id: Uuid,
    pub transaction_key: Option<String>,
    pub account_ref: String,
    pub date_transacted: OffsetDateTime,
    pub description: Option<String>,
    pub related_asset_ref: Option<String>,
    pub external_id: Option<String>,
}

pub struct ImportParsedLegInsert {
    pub parsed_line_id: Uuid,
    pub asset_ref: String,
    pub quantity: Decimal,
    pub fee_type: Option<ImportFeeType>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportAssertionModel {
    pub file_id: Option<Uuid>,
    pub account_ref: Option<String>,
    pub asset_ref: Option<String>,
    pub account_id: Option<Uuid>,
    pub asset_id: Option<i32>,
    pub kind: String,
    pub as_of_date: Date,
    pub quantity: Decimal,
}

pub struct ImportAssertionInsert {
    pub session_id: Uuid,
    pub file_id: Uuid,
    pub account_ref: String,
    pub asset_ref: String,
    pub kind: ImportAssertionKind,
    pub as_of_date: Date,
    pub quantity: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportSessionModel {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub lifecycle: String,
    pub created_at: OffsetDateTime,
}

impl ImportSessionModel {
    pub fn session_lifecycle(&self) -> anyhow::Result<ImportSessionLifecycle> {
        ImportSessionLifecycle::try_from(self.lifecycle.as_str())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportSessionSummaryModel {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub lifecycle: String,
    pub created_at: OffsetDateTime,
    pub pending_files: i64,
    pub processing_files: i64,
    pub failed_files: i64,
    pub parsed_files: i64,
    pub classification_run_id: Option<Uuid>,
    pub classified_at: Option<OffsetDateTime>,
    pub classification_error: Option<String>,
}

impl ImportSessionSummaryModel {
    pub fn session_lifecycle(&self) -> anyhow::Result<ImportSessionLifecycle> {
        ImportSessionLifecycle::try_from(self.lifecycle.as_str())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportFileModel {
    pub file_id: Uuid,
    pub conversation_id: Uuid,
    pub status: String,
    pub error_message: Option<String>,
    pub paused_until: Option<OffsetDateTime>,
    pub date_order: Option<String>,
    pub created_at: OffsetDateTime,
}

impl ImportFileModel {
    pub fn file_status(&self) -> anyhow::Result<ImportFileStatus> {
        ImportFileStatus::try_from(self.status.as_str())
    }

    pub fn file_date_order(&self) -> anyhow::Result<Option<ImportDateOrder>> {
        self.date_order
            .as_deref()
            .map(ImportDateOrder::try_from)
            .transpose()
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportAccountRefModel {
    pub file_id: Uuid,
    pub account_ref: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportAccountMappingModel {
    pub file_id: Uuid,
    pub account_ref: String,
    pub account_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportAssetRefModel {
    pub file_id: Uuid,
    pub account_ref: String,
    pub asset_ref: String,
    pub counter_asset_ref: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportAssetMappingModel {
    pub asset_ref: String,
    pub asset_id: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportParsedLineModel {
    pub id: Uuid,
    pub file_id: Uuid,
    pub transaction_key: Option<String>,
    pub account_ref: String,
    pub date_transacted: OffsetDateTime,
    pub description: Option<String>,
    pub related_asset_ref: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportParsedLegModel {
    pub parsed_line_id: Uuid,
    pub asset_ref: String,
    pub quantity: Decimal,
    pub fee_type: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportStagedDecisionModel {
    pub id: Uuid,
    pub ask_key: Option<String>,
    pub verb: String,
    pub transaction_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportLineClassificationModel {
    pub parsed_line_id: Uuid,
    pub type_id: DatabaseTransactionTypes,
    pub category_id: Option<i32>,
}
