use std::collections::HashMap;

use shared::view_models::portfolio::get_holdings::GetHoldingsResponseViewModel;
use shared::view_models::properties::base_models::property::PropertyKind as PropertyKindViewModel;
use shared::view_models::properties::get_properties::GetPropertiesResponseViewModel;
use uuid::Uuid;

use crate::api::accounts::is_liability_account_type;
use crate::models::{
    AccountCurrencyBalance, CurrencyAccount, HoldingItem, HoldingProperty, LiabilityItem,
    PropertyKind,
};

const CURRENCIES: i32 = 1;

pub struct PortfolioHoldings {
    pub holdings: Vec<HoldingItem>,
    pub liabilities: Vec<LiabilityItem>,
}

pub fn extract_holdings(
    holdings_body: &str,
    properties_body: Option<&str>,
) -> Result<PortfolioHoldings, String> {
    let resp: GetHoldingsResponseViewModel =
        serde_json::from_str(holdings_body).map_err(|e| e.to_string())?;
    let properties = properties_body
        .and_then(|body| serde_json::from_str::<GetPropertiesResponseViewModel>(body).ok())
        .map(|r| r.properties)
        .unwrap_or_default();

    let accounts: HashMap<Uuid, _> = resp
        .lookup_tables
        .accounts
        .iter()
        .map(|a| (a.account_id.0, a))
        .collect();
    let assets: HashMap<i32, _> = resp
        .lookup_tables
        .assets
        .iter()
        .map(|a| (a.asset_id.0, a))
        .collect();
    let property_of_account: HashMap<Uuid, usize> = properties
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            std::iter::once((p.account_id.0, i))
                .chain(p.loan_account_ids.iter().map(move |id| (id.0, i)))
        })
        .collect();

    let mut property_sums: Vec<Option<(f64, f64, f64)>> = vec![None; properties.len()];
    let mut liability_sums: HashMap<Uuid, f64> = HashMap::new();
    let mut asset_sums: HashMap<i32, (f64, f64)> = HashMap::new();
    for row in &resp.holdings {
        let units = decimal_to_f64(&row.units);
        let value = row.value.as_ref().map(decimal_to_f64).unwrap_or(0.0);
        let account_id = row.account_id.0;
        if let Some(&i) = property_of_account.get(&account_id) {
            let (asset_value, loan_balance, asset_units) =
                property_sums[i].get_or_insert((0.0, 0.0, 0.0));
            if account_id == properties[i].account_id.0 {
                *asset_value += value;
                if row.asset_id.0 == properties[i].asset_id.0 {
                    *asset_units += units;
                }
            } else {
                *loan_balance += value;
            }
        } else if accounts
            .get(&account_id)
            .is_some_and(|a| is_liability_account_type(a.account.account_type.0))
        {
            *liability_sums.entry(account_id).or_insert(0.0) += value;
        } else {
            let sums = asset_sums.entry(row.asset_id.0).or_insert((0.0, 0.0));
            sums.0 += units;
            sums.1 += value;
        }
    }

    let mut holdings: Vec<HoldingItem> = asset_sums
        .into_iter()
        .map(|(asset_id, (units, value))| {
            let asset = assets.get(&asset_id);
            HoldingItem {
                asset_name: asset
                    .map(|a| a.asset.name.as_str().to_string())
                    .unwrap_or_default(),
                ticker: asset
                    .map(|a| a.asset.ticker.as_str().to_string())
                    .unwrap_or_default(),
                units,
                value,
                asset_type_id: asset.map(|a| a.asset.asset_type.0).unwrap_or(0),
                asset_id,
                property: None,
            }
        })
        .collect();
    holdings.extend(
        properties
            .iter()
            .zip(property_sums)
            .filter_map(|(p, sums)| {
                let (asset_value, loan_balance, units) = sums?;
                let asset = assets.get(&p.asset_id.0);
                Some(HoldingItem {
                    asset_name: p.name.as_str().to_string(),
                    ticker: asset
                        .map(|a| a.asset.ticker.as_str().to_string())
                        .unwrap_or_default(),
                    units,
                    value: asset_value + loan_balance,
                    asset_type_id: asset.map(|a| a.asset.asset_type.0).unwrap_or(0),
                    asset_id: p.asset_id.0,
                    property: Some(HoldingProperty {
                        property_id: p.property_id.0.to_string(),
                        kind: match p.kind {
                            PropertyKindViewModel::RealEstate => PropertyKind::RealEstate,
                            PropertyKindViewModel::Vehicle => PropertyKind::Vehicle,
                        },
                        asset_value,
                        loan_balance,
                    }),
                })
            }),
    );
    holdings.sort_by(|a, b| b.value.total_cmp(&a.value));

    let mut liabilities: Vec<LiabilityItem> = liability_sums
        .into_iter()
        .map(|(account_id, value)| {
            let account = accounts.get(&account_id);
            LiabilityItem {
                account_id: account_id.to_string(),
                account_name: account
                    .map(|a| a.account.name.as_str().to_string())
                    .unwrap_or_default(),
                account_type_id: account.map(|a| a.account.account_type.0).unwrap_or(0),
                value,
            }
        })
        .collect();
    liabilities.sort_by(|a, b| a.value.total_cmp(&b.value));

    Ok(PortfolioHoldings {
        holdings,
        liabilities,
    })
}

pub struct CurrencyHoldings {
    pub name: String,
    pub ticker: String,
    pub units: f64,
    pub value: Option<f64>,
    pub accounts: Vec<CurrencyAccount>,
}

/// Skips property accounts so the total matches the currency's Portfolio row, where their cash
/// is part of the property's equity.
pub fn extract_currency_holdings(
    holdings_body: &str,
    properties_body: Option<&str>,
    asset_id: i32,
) -> Result<CurrencyHoldings, String> {
    let resp: GetHoldingsResponseViewModel =
        serde_json::from_str(holdings_body).map_err(|e| e.to_string())?;
    let property_accounts: Vec<Uuid> = properties_body
        .and_then(|body| serde_json::from_str::<GetPropertiesResponseViewModel>(body).ok())
        .map(|r| r.properties.into_iter().map(|p| p.account_id.0).collect())
        .unwrap_or_default();
    let asset = resp
        .lookup_tables
        .assets
        .iter()
        .find(|a| a.asset_id.0 == asset_id);
    let accounts: HashMap<Uuid, _> = resp
        .lookup_tables
        .accounts
        .iter()
        .map(|a| (a.account_id.0, a))
        .collect();

    let mut units = 0.0;
    let mut value = Some(0.0);
    let mut holding_accounts = Vec::new();
    for row in resp.holdings.iter().filter(|r| r.asset_id.0 == asset_id) {
        let Some(account) = accounts.get(&row.account_id.0) else {
            continue;
        };
        let account_type_id = account.account.account_type.0;
        if is_liability_account_type(account_type_id)
            || property_accounts.contains(&row.account_id.0)
            || row.units.is_zero()
        {
            continue;
        }
        let row_units = decimal_to_f64(&row.units);
        units += row_units;
        value = value
            .zip(row.value.as_ref())
            .map(|(sum, v)| sum + decimal_to_f64(v));
        holding_accounts.push(CurrencyAccount {
            account_id: row.account_id.0.to_string(),
            account_name: account.account.name.as_str().to_string(),
            account_type_id,
            units: row_units,
        });
    }
    holding_accounts.sort_by(|a, b| b.units.total_cmp(&a.units));

    Ok(CurrencyHoldings {
        name: asset
            .map(|a| a.asset.name.as_str().to_string())
            .unwrap_or_default(),
        ticker: asset
            .map(|a| a.asset.ticker.as_str().to_string())
            .unwrap_or_default(),
        units,
        value,
        accounts: holding_accounts,
    })
}

/// Whole-account balances, including assets the account no longer holds any of.
pub fn account_holdings_path(user_id: &str) -> String {
    format!(
        "/api/users/{user_id}/portfolio/holdings?apply_ownership_share=false&include_zero_balances=true"
    )
}

pub fn extract_account_currency_balances(
    holdings_body: &str,
    account_id: &str,
) -> Result<Vec<AccountCurrencyBalance>, String> {
    let resp: GetHoldingsResponseViewModel =
        serde_json::from_str(holdings_body).map_err(|e| e.to_string())?;
    let mut balances: Vec<AccountCurrencyBalance> = resp
        .holdings
        .iter()
        .filter(|row| row.account_id.0.to_string() == account_id)
        .filter_map(|row| {
            let asset = resp
                .lookup_tables
                .assets
                .iter()
                .find(|a| a.asset_id.0 == row.asset_id.0)?;
            (asset.asset.asset_type.0 == CURRENCIES).then(|| AccountCurrencyBalance {
                asset_id: row.asset_id.0,
                ticker: asset.asset.ticker.as_str().to_string(),
                name: asset.asset.name.as_str().to_string(),
                units: decimal_to_f64(&row.units),
                value: row.value.as_ref().map(decimal_to_f64),
            })
        })
        .collect();
    balances.sort_by(|a, b| {
        b.value
            .unwrap_or(0.0)
            .total_cmp(&a.value.unwrap_or(0.0))
            .then_with(|| a.ticker.cmp(&b.ticker))
    });
    Ok(balances)
}

pub fn extract_account_currency_holdings(
    holdings_body: &str,
    account_id: &str,
    asset_id: i32,
) -> Result<CurrencyHoldings, String> {
    let resp: GetHoldingsResponseViewModel =
        serde_json::from_str(holdings_body).map_err(|e| e.to_string())?;
    let asset = resp
        .lookup_tables
        .assets
        .iter()
        .find(|a| a.asset_id.0 == asset_id);
    let account = resp
        .lookup_tables
        .accounts
        .iter()
        .find(|a| a.account_id.0.to_string() == account_id);
    let row = resp
        .holdings
        .iter()
        .find(|r| r.asset_id.0 == asset_id && r.account_id.0.to_string() == account_id);
    let units = row.map(|r| decimal_to_f64(&r.units)).unwrap_or(0.0);

    Ok(CurrencyHoldings {
        name: asset
            .map(|a| a.asset.name.as_str().to_string())
            .unwrap_or_default(),
        ticker: asset
            .map(|a| a.asset.ticker.as_str().to_string())
            .unwrap_or_default(),
        units,
        value: row.and_then(|r| r.value.as_ref().map(decimal_to_f64)),
        accounts: account
            .map(|a| CurrencyAccount {
                account_id: account_id.to_string(),
                account_name: a.account.name.as_str().to_string(),
                account_type_id: a.account.account_type.0,
                units,
            })
            .into_iter()
            .collect(),
    })
}

fn decimal_to_f64(value: &rust_decimal::Decimal) -> f64 {
    value.to_string().parse().unwrap_or(0.0)
}

pub fn extract_account_balances(body: &str) -> Result<HashMap<String, f64>, String> {
    let resp: GetHoldingsResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;

    let mut balances: HashMap<String, f64> = HashMap::new();
    for row in &resp.holdings {
        let value: f64 = row
            .value
            .as_ref()
            .map(|v| v.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);
        *balances.entry(row.account_id.0.to_string()).or_insert(0.0) += value;
    }

    Ok(balances)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLDINGS_JSON: &str = r#"{
        "holdings": [
            { "account_id": "00000000-0000-0000-0000-000000000001", "asset_id": 42, "units": "100", "value": "87.0" },
            { "account_id": "00000000-0000-0000-0000-000000000001", "asset_id": 45, "units": "10", "value": "10.0" },
            { "account_id": "00000000-0000-0000-0000-000000000002", "asset_id": 45, "units": "-50", "value": "-50.0" },
            { "account_id": "00000000-0000-0000-0000-000000000003", "asset_id": 99, "units": "5", "value": null }
        ],
        "lookup_tables": { "accounts": [], "assets": [] }
    }"#;

    #[test]
    fn debt_leaves_currency_rows_and_folds_into_its_property_or_a_liability_row() {
        let holdings = r#"{
            "holdings": [
                { "account_id": "00000000-0000-0000-0000-00000000000a", "asset_id": 45, "units": "1000", "value": "1000" },
                { "account_id": "00000000-0000-0000-0000-00000000000b", "asset_id": 45, "units": "-150000", "value": "-150000" },
                { "account_id": "00000000-0000-0000-0000-00000000000c", "asset_id": 45, "units": "-300", "value": "-300" },
                { "account_id": "00000000-0000-0000-0000-00000000000d", "asset_id": 7, "units": "1", "value": "200000" },
                { "account_id": "00000000-0000-0000-0000-00000000000d", "asset_id": 45, "units": "50", "value": "50" }
            ],
            "lookup_tables": {
                "accounts": [
                    { "account_id": "00000000-0000-0000-0000-00000000000a", "name": "Current", "account_type": 1 },
                    { "account_id": "00000000-0000-0000-0000-00000000000b", "name": "Mortgage", "account_type": 7 },
                    { "account_id": "00000000-0000-0000-0000-00000000000c", "name": "Card", "account_type": 4 },
                    { "account_id": "00000000-0000-0000-0000-00000000000d", "name": "House", "account_type": 9 }
                ],
                "assets": []
            }
        }"#;
        let properties = r#"{"properties":[{"property_id":"00000000-0000-0000-0000-0000000000aa","kind":"real_estate","name":"House","account_id":"00000000-0000-0000-0000-00000000000d","asset_id":7,"currency_asset_id":45,"ownership_share":1,"loan_account_ids":["00000000-0000-0000-0000-00000000000b"]}]}"#;

        let folded = extract_holdings(holdings, Some(properties)).unwrap();

        let rows: Vec<(i32, f64)> = folded
            .holdings
            .iter()
            .map(|h| (h.asset_id, h.value))
            .collect();
        assert_eq!(rows, vec![(7, 50050.0), (45, 1000.0)]);
        let property = folded.holdings[0].property.as_ref().unwrap();
        assert_eq!(
            (property.asset_value, property.loan_balance),
            (200050.0, -150000.0)
        );
        assert_eq!(folded.liabilities.len(), 1);
        assert_eq!(folded.liabilities[0].account_name, "Card");
        assert_eq!(folded.liabilities[0].value, -300.0);

        let unlinked = extract_holdings(holdings, None).unwrap();
        let gbp = unlinked.holdings.iter().find(|h| h.asset_id == 45).unwrap();
        assert_eq!(gbp.value, 1050.0);
        assert_eq!(unlinked.liabilities.len(), 2);
    }

    #[test]
    fn account_balances_sum_value_per_account_and_treat_null_as_zero() {
        let balances = extract_account_balances(HOLDINGS_JSON).unwrap();
        assert!((balances["00000000-0000-0000-0000-000000000001"] - 97.0).abs() < 1e-9);
        assert!((balances["00000000-0000-0000-0000-000000000002"] - (-50.0)).abs() < 1e-9);
        assert!((balances["00000000-0000-0000-0000-000000000003"]).abs() < 1e-9);
    }
}
