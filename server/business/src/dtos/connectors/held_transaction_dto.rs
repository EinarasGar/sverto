use dal::models::connector_models::{HeldTransactionRow, SyncedTransactionLineRow};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::transaction_dto::TransactionDto;

#[derive(Debug, Clone)]
pub struct HeldTransactionDto {
    pub id: Uuid,
    pub binding_id: Uuid,
    pub account_id: Uuid,
    pub write_mode: String,
    pub provider_kind: String,
    pub external_id: String,
    pub external_hash: String,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub date_transacted: OffsetDateTime,
    pub description: String,
    pub candidate_ids: Vec<Uuid>,
}

impl From<HeldTransactionRow> for HeldTransactionDto {
    fn from(row: HeldTransactionRow) -> Self {
        Self {
            id: row.id,
            binding_id: row.binding_id,
            account_id: row.account_id,
            write_mode: row.write_mode,
            provider_kind: row.provider_kind,
            external_id: row.external_id,
            external_hash: row.external_hash,
            asset_id: row.asset_id,
            quantity: row.quantity,
            date_transacted: row.date_transacted,
            description: row.description,
            candidate_ids: row.candidate_ids,
        }
    }
}

pub struct HeldTransactionWithCandidatesDto {
    pub held: HeldTransactionDto,
    pub candidates: Vec<TransactionDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldDecisionDto {
    Same { transaction_id: Uuid },
    Different,
}

#[derive(Debug, Clone)]
pub struct SyncedTransactionLineDto {
    pub transaction_id: Uuid,
    pub description: Option<String>,
    pub quantity: Decimal,
    pub ticker: String,
    pub account_name: String,
}

impl From<SyncedTransactionLineRow> for SyncedTransactionLineDto {
    fn from(row: SyncedTransactionLineRow) -> Self {
        Self {
            transaction_id: row.transaction_id,
            description: row.description,
            quantity: row.quantity,
            ticker: row.ticker,
            account_name: row.account_name,
        }
    }
}
