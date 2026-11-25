use shared::view_models::portfolio::get_networth_history::GetNetWorthHistoryResponseViewModel;

use super::infra::SharedInfra;
use super::{CHART_LABELS, CHART_RANGES};
use crate::api::account_transactions::{
    extract_account_transactions, scoped_transactions_page_path,
};
use crate::api::holdings::{
    account_holdings_path, extract_account_currency_holdings, extract_currency_holdings,
};
use crate::error::ApiError;
use crate::models::{ChartPeriodData, ChartPoint, CurrencyDetail, TransactionListScope};

const RECENT_TRANSACTIONS: u32 = 5;

pub async fn get_currency_detail(
    infra: &SharedInfra,
    asset_id: i32,
    account_id: Option<&str>,
    refresh: bool,
    auth_token: Option<&str>,
) -> Result<CurrencyDetail, ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    if refresh {
        infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/portfolio"));
        infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/properties"));
        infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/assets/{asset_id}/"));
        if let Some(account_id) = account_id {
            infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/accounts/{account_id}"));
        }
    }

    let holdings_path = match account_id {
        Some(_) => account_holdings_path(&user_id),
        None => format!("/api/users/{user_id}/portfolio/holdings"),
    };
    let properties_path = format!("/api/users/{user_id}/properties");
    let transactions_scope = match account_id {
        Some(account_id) => TransactionListScope::Asset {
            asset_id,
            account_id: Some(account_id.to_string()),
        },
        None => TransactionListScope::Currency { asset_id },
    };
    let transactions_path =
        scoped_transactions_page_path(&user_id, &transactions_scope, RECENT_TRANSACTIONS, 0);
    let chart_paths: Vec<String> = CHART_RANGES
        .iter()
        .map(|range| match account_id {
            Some(account_id) => format!(
                "/api/users/{user_id}/accounts/{account_id}/portfolio/assets/{asset_id}/history?range={range}&default_asset_id={asset_id}"
            ),
            None => format!(
                "/api/users/{user_id}/portfolio/assets/{asset_id}/history?range={range}&default_asset_id={asset_id}&exclude_liabilities=true"
            ),
        })
        .collect();

    let (holdings_result, properties_result, transactions_result, chart_results) = tokio::join!(
        infra.get(&holdings_path, auth_token),
        async {
            match account_id {
                Some(_) => None,
                None => infra.get(&properties_path, auth_token).await.ok(),
            }
        },
        infra.get(&transactions_path, auth_token),
        futures_util::future::join_all(chart_paths.iter().map(|path| infra.get(path, auth_token))),
    );

    let holdings_body = holdings_result?.body;
    let holdings = match account_id {
        Some(account_id) => extract_account_currency_holdings(&holdings_body, account_id, asset_id),
        None => extract_currency_holdings(
            &holdings_body,
            properties_result.as_ref().map(|resp| resp.body.as_str()),
            asset_id,
        ),
    }
    .map_err(|reason| ApiError::Parse { reason })?;
    let transactions = extract_account_transactions(&transactions_result?.body)
        .map_err(|reason| ApiError::Parse { reason })?;
    let chart_data = chart_results
        .into_iter()
        .zip(CHART_LABELS)
        .map(|(result, label)| {
            let parsed = serde_json::from_str::<GetNetWorthHistoryResponseViewModel>(&result?.body)
                .map_err(|e| ApiError::Parse {
                    reason: e.to_string(),
                })?;
            Ok(ChartPeriodData {
                period: label.to_string(),
                points: parsed
                    .sums
                    .into_iter()
                    .map(|p| ChartPoint {
                        timestamp: p.date,
                        value: p.rate,
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    let is_base_currency = infra.default_asset_id() == Some(asset_id);
    let rate = match holdings.value {
        Some(value) if !is_base_currency && holdings.units != 0.0 => Some(value / holdings.units),
        _ => None,
    };

    Ok(CurrencyDetail {
        asset_id,
        name: holdings.name,
        ticker: holdings.ticker,
        units: holdings.units,
        value: holdings.value,
        rate,
        is_base_currency,
        base_ticker: infra.default_asset_ticker().unwrap_or_default(),
        accounts: holdings.accounts,
        chart_data,
        recent_transactions: transactions.items,
        has_more_transactions: transactions.has_more,
    })
}
