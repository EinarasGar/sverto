use rust_decimal::prelude::ToPrimitive;
use shared::view_models::held_transactions::decide_held_transaction::DecideHeldTransactionRequestViewModel;
use shared::view_models::held_transactions::get_held_transactions::GetHeldTransactionsResponseViewModel;
use shared::view_models::transactions::transaction_types::TransactionWithId;

use super::transactions::{
    find_account, find_ticker, primary_display, transaction_base, transaction_candidate, ET,
};
use crate::models::{HeldTransaction, HeldTransactionDecision};
use crate::money::format_money;

fn amount_in(
    tx: &TransactionWithId,
    entries: &[ET],
    account_id: &str,
    asset_id: i32,
) -> Option<f64> {
    let fees = transaction_base(tx)
        .base
        .fees
        .iter()
        .flatten()
        .map(|fee| &fee.entry.entry);
    let amounts: Vec<f64> = entries
        .iter()
        .filter(|entry| entry.0 == account_id && entry.1 == asset_id)
        .map(|entry| entry.2)
        .chain(
            fees.filter(|fee| {
                fee.account_id.0.to_string() == account_id && fee.asset_id.0 == asset_id
            })
            .filter_map(|fee| fee.amount.0.to_f64()),
        )
        .collect();
    (!amounts.is_empty()).then(|| amounts.iter().sum())
}

pub fn extract_held_transactions(body: &str) -> Result<Vec<HeldTransaction>, String> {
    let resp: GetHeldTransactionsResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    let tables = &resp.lookup_tables;
    resp.held_transactions
        .into_iter()
        .map(|held| {
            let account_id = held.entry.account_id.0.to_string();
            let amount =
                held.entry.amount.0.to_f64().ok_or_else(|| {
                    format!("held transaction {} has no readable amount", held.id)
                })?;
            let candidates = held
                .candidates
                .iter()
                .map(|candidate| {
                    transaction_candidate(candidate, tables, |entries| {
                        match amount_in(candidate, entries, &account_id, held.entry.asset_id.0) {
                            Some(amount) => format_money(
                                amount,
                                find_ticker(tables, held.entry.asset_id.0),
                                false,
                            ),
                            None => primary_display(entries, tables),
                        }
                    })
                })
                .collect::<Result<_, String>>()?;
            Ok(HeldTransaction {
                id: held.id.to_string(),
                provider_kind: held.provider_kind,
                date: held.date.unix_timestamp(),
                description: held.description,
                account_name: find_account(tables, &account_id).unwrap_or_default(),
                account_id,
                amount_display: format_money(
                    amount,
                    find_ticker(tables, held.entry.asset_id.0),
                    false,
                ),
                candidates,
            })
        })
        .collect()
}

pub fn decision_body(decision: HeldTransactionDecision) -> Result<String, String> {
    let request = match decision {
        HeldTransactionDecision::Same { transaction_id } => {
            DecideHeldTransactionRequestViewModel::Same {
                transaction_id: transaction_id
                    .parse()
                    .map_err(|_| format!("invalid transaction id '{transaction_id}'"))?,
            }
        }
        HeldTransactionDecision::Different => DecideHeldTransactionRequestViewModel::Different,
    };
    serde_json::to_string(&request).map_err(|e| e.to_string())
}
