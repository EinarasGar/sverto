use rust_decimal::Decimal;
use sqlx::types::Uuid;
use time::{Date, OffsetDateTime};

use crate::enums::transaction_types::DatabaseTransactionTypes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSourceKind {
    Connector,
    Statement,
    Receipt,
    Manual,
    Balance,
}

impl TransactionSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Statement => "statement",
            Self::Receipt => "receipt",
            Self::Manual => "manual",
            Self::Balance => "balance",
        }
    }
}

impl From<TransactionSourceKind> for sea_query::Value {
    fn from(value: TransactionSourceKind) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

impl TryFrom<&str> for TransactionSourceKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "connector" => Ok(Self::Connector),
            "statement" => Ok(Self::Statement),
            "receipt" => Ok(Self::Receipt),
            "manual" => Ok(Self::Manual),
            "balance" => Ok(Self::Balance),
            other => Err(anyhow::anyhow!("Unknown transaction source kind: {other}")),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct LedgerMovementModel {
    pub id: Uuid,
    pub group_id: Option<Uuid>,
    pub type_id: DatabaseTransactionTypes,
    pub date_transacted: OffsetDateTime,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub description: Option<String>,
    pub connector_binding_ids: Vec<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TransactionSourceModel {
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub occurred_on: Date,
    pub description: Option<String>,
    pub kind: String,
    pub stream: String,
    pub external_ref: Option<String>,
    pub itemised_fee: Option<Decimal>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct LedgerTotalModel {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
}

pub struct TransactionSourceInsert {
    pub user_id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub occurred_on: Date,
    pub description: Option<String>,
    pub kind: TransactionSourceKind,
    pub stream: String,
    pub external_ref: Option<String>,
    pub itemised_fee: Option<Decimal>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct RecordedCountModel {
    pub stream: String,
    pub transactions: i64,
}
