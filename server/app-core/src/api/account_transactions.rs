use shared::view_models::base_models::search::AccountTransactionsPage;

use crate::api::transactions::to_list_item;
use crate::models::{TransactionListItem, TransactionListScope};

pub struct AccountTransactionsData {
    pub items: Vec<TransactionListItem>,
    pub has_more: bool,
}

pub fn extract_account_transactions(body: &str) -> Result<AccountTransactionsData, String> {
    let resp: AccountTransactionsPage = serde_json::from_str(body).map_err(|e| e.to_string())?;

    let tables = &resp.lookup_tables;
    let items: Vec<TransactionListItem> = resp
        .results
        .iter()
        .map(|tx| to_list_item(tx, tables))
        .collect();

    let total = resp.total_results as i64;
    let has_more = (items.len() as i64) < total;

    Ok(AccountTransactionsData { items, has_more })
}

pub fn scoped_transactions_path(user_id: &str, scope: &TransactionListScope) -> String {
    match scope {
        TransactionListScope::Account { account_id } => {
            format!("/api/users/{user_id}/accounts/{account_id}/transactions")
        }
        TransactionListScope::Asset {
            asset_id,
            account_id: None,
        }
        | TransactionListScope::Currency { asset_id } => {
            format!("/api/users/{user_id}/assets/{asset_id}/transactions")
        }
        TransactionListScope::Asset {
            asset_id,
            account_id: Some(account_id),
        } => format!("/api/users/{user_id}/accounts/{account_id}/assets/{asset_id}/transactions"),
    }
}

pub fn scoped_transactions_page_path(
    user_id: &str,
    scope: &TransactionListScope,
    count: u32,
    start: u32,
) -> String {
    let exclude_liabilities = matches!(scope, TransactionListScope::Currency { .. });
    format!(
        "{}?count={count}&start={start}{}",
        scoped_transactions_path(user_id, scope),
        if exclude_liabilities {
            "&exclude_liabilities=true"
        } else {
            ""
        }
    )
}
