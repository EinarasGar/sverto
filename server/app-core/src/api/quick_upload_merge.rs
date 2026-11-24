use rust_decimal::Decimal;
use shared::view_models::ai::quick_upload::{
    GetQuickUploadMatchesResponseViewModel, MergeQuickUploadLineViewModel,
    MergeQuickUploadRequestViewModel,
};
use shared::view_models::transactions::base_models::category_id::RequiredCategoryId;
use shared::view_models::transactions::base_models::description::Description;
use shared::view_models::transactions::value_types::Amount;
use time::OffsetDateTime;

use super::transactions::{primary_display, transaction_candidate};
use crate::error::ApiError;
use crate::models::{QuickUploadReceipt, TransactionCandidate};

pub fn extract_matches(body: &str) -> Result<Vec<TransactionCandidate>, String> {
    let resp: GetQuickUploadMatchesResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    let tables = &resp.lookup_tables;
    resp.matches
        .iter()
        .map(|transaction| {
            transaction_candidate(transaction, tables, |entries| {
                primary_display(entries, tables)
            })
        })
        .collect()
}

fn description(text: Option<String>) -> Option<Description> {
    text.filter(|text| !text.trim().is_empty())
        .map(Description::from_trusted)
}

pub fn merge_body(transaction_id: &str, receipt: QuickUploadReceipt) -> Result<String, ApiError> {
    let invalid = |reason: String| ApiError::Parse { reason };
    let lines = receipt
        .lines
        .into_iter()
        .map(|line| {
            let amount = Decimal::try_from(line.amount)
                .map_err(|e| invalid(format!("invalid amount {}: {e}", line.amount)))?;
            Ok(MergeQuickUploadLineViewModel {
                description: description(line.description),
                category_id: RequiredCategoryId(line.category_id),
                amount: Amount(amount),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let request = MergeQuickUploadRequestViewModel {
        transaction_id: transaction_id
            .parse()
            .map_err(|_| invalid(format!("invalid transaction id '{transaction_id}'")))?,
        description: description(receipt.description),
        category_id: RequiredCategoryId(receipt.category_id),
        date: OffsetDateTime::from_unix_timestamp(receipt.date)
            .map_err(|e| invalid(format!("invalid date: {e}")))?,
        lines,
    };
    serde_json::to_string(&request).map_err(|e| invalid(e.to_string()))
}
