use std::collections::HashMap;

use shared::view_models::accounts::base_models::account_id::RequiredAccountId;
use shared::view_models::accounts::base_models::account_name::AccountName;
use shared::view_models::accounts::base_models::ownership_share::OwnershipShare;
use shared::view_models::assets::base_models::asset_id::RequiredAssetId;
use shared::view_models::assets::base_models::positive_rate::PositiveRate;
use shared::view_models::properties::base_models::property::{
    IdentifiablePropertyViewModel, PropertyKind as PropertyKindViewModel,
};
use shared::view_models::properties::create_property::{
    CreatePropertyRequestViewModel, PropertyPurchaseViewModel,
};
use shared::view_models::properties::get_properties::GetPropertiesResponseViewModel;
use time::OffsetDateTime;

use crate::error::ApiError;
use crate::models::{
    AccountListItem, ChartPoint, CreatePropertyInput, PropertyKind, PropertySummary,
};

pub fn extract_properties(body: &str) -> Result<Vec<(String, PropertySummary)>, String> {
    let resp: GetPropertiesResponseViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(resp
        .properties
        .into_iter()
        .map(|row| {
            (
                row.account_id.0.to_string(),
                PropertySummary {
                    property_id: row.property_id.0.to_string(),
                    kind: match row.kind {
                        PropertyKindViewModel::RealEstate => PropertyKind::RealEstate,
                        PropertyKindViewModel::Vehicle => PropertyKind::Vehicle,
                    },
                    asset_id: row.asset_id.0,
                    currency_asset_id: row.currency_asset_id.0,
                    loan_account_ids: row
                        .loan_account_ids
                        .into_iter()
                        .map(|id| id.0.to_string())
                        .collect(),
                    value: None,
                    loan_balance: None,
                    equity: None,
                    equity_share: None,
                    loan_to_value: None,
                },
            )
        })
        .collect())
}

pub fn build_create_property_body(input: CreatePropertyInput) -> Result<String, ApiError> {
    let parse = |reason: String| ApiError::Parse { reason };
    let decimal = |amount: f64| {
        rust_decimal::Decimal::try_from(amount)
            .map_err(|e| parse(format!("invalid amount {amount}: {e}")))
    };
    let request = CreatePropertyRequestViewModel {
        kind: match input.kind {
            PropertyKind::RealEstate => PropertyKindViewModel::RealEstate,
            PropertyKind::Vehicle => PropertyKindViewModel::Vehicle,
        },
        name: AccountName::from_trusted(input.name),
        currency_asset_id: RequiredAssetId(input.currency_asset_id),
        ownership_share: OwnershipShare::from_trusted(decimal(input.ownership_share)?),
        value: PositiveRate::from_trusted(decimal(input.value)?),
        purchase: input
            .purchase
            .map(|purchase| {
                Ok::<_, ApiError>(PropertyPurchaseViewModel {
                    date: OffsetDateTime::from_unix_timestamp(purchase.date)
                        .map_err(|e| parse(format!("invalid date: {e}")))?,
                    price: PositiveRate::from_trusted(decimal(purchase.price)?),
                })
            })
            .transpose()?,
        loan_account_ids: input
            .loan_account_ids
            .iter()
            .map(|id| {
                uuid::Uuid::parse_str(id)
                    .map(RequiredAccountId)
                    .map_err(|e| parse(format!("invalid account id {id}: {e}")))
            })
            .collect::<Result<_, _>>()?,
    };
    serde_json::to_string(&request).map_err(|e| parse(e.to_string()))
}

pub fn extract_created_property_id(body: &str) -> Result<String, String> {
    let property: IdentifiablePropertyViewModel =
        serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(property.property_id.0.to_string())
}

pub fn attach_properties(
    accounts: &mut [AccountListItem],
    properties: Vec<(String, PropertySummary)>,
) {
    for (account_id, property) in properties {
        if !accounts.iter().any(|a| a.id == account_id) {
            continue;
        }
        for loan_id in &property.loan_account_ids {
            if let Some(loan) = accounts.iter_mut().find(|a| &a.id == loan_id) {
                loan.linked_property_id = Some(property.property_id.clone());
            }
        }
        if let Some(account) = accounts.iter_mut().find(|a| a.id == account_id) {
            account.property = Some(property);
        }
    }
}

pub fn fill_property_figures(accounts: &mut [AccountListItem]) {
    let balances: HashMap<String, (f64, f64)> = accounts
        .iter()
        .map(|a| (a.id.clone(), (a.balance.unwrap_or(0.0), a.ownership_share)))
        .collect();
    for account in accounts.iter_mut() {
        let share = account.ownership_share;
        let value = account.balance.unwrap_or(0.0);
        if let Some(property) = account.property.as_mut() {
            let loans: Vec<(f64, f64)> = property
                .loan_account_ids
                .iter()
                .filter_map(|id| balances.get(id).copied())
                .collect();
            let loan_balance: f64 = loans.iter().map(|(balance, _)| balance).sum();
            property.value = Some(value);
            property.loan_balance = Some(loan_balance);
            property.equity = Some(value + loan_balance);
            property.loan_to_value = (value > 0.0).then(|| -loan_balance / value);
            property.equity_share = Some(
                value * share
                    + loans
                        .iter()
                        .map(|(balance, loan_share)| balance * loan_share)
                        .sum::<f64>(),
            );
        }
    }
}

const TREND_SAMPLES: usize = 120;
const TREND_BANDWIDTH_SHARE_OF_SPAN: f64 = 1.0 / 12.0;
const TREND_MAX_BANDWIDTH_SECONDS: f64 = 90.0 * 86_400.0;

/// Valuations are sparse estimates, so a straight line through them shows a cliff wherever two
/// sit close together. The trend smooths the piecewise-linear line with a Gaussian kernel, then
/// shifts it by a linear correction so it still starts at the first valuation and ends at today's
/// value.
pub fn value_trend(
    valuations: &[ChartPoint],
    now: i64,
    current_value: Option<f64>,
) -> Vec<ChartPoint> {
    let mut points: Vec<(f64, f64)> = valuations
        .iter()
        .filter(|p| p.timestamp <= now)
        .map(|p| (p.timestamp as f64, p.value))
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let Some(&(_, last_value)) = points.last() else {
        return vec![];
    };
    points.push((now as f64, current_value.unwrap_or(last_value)));

    let start = points[0].0;
    let span = now as f64 - start;
    if span <= 0.0 {
        return vec![ChartPoint {
            timestamp: now,
            value: points[points.len() - 1].1,
        }];
    }

    let times: Vec<f64> = (0..TREND_SAMPLES)
        .map(|k| start + span * k as f64 / (TREND_SAMPLES - 1) as f64)
        .collect();
    let linear: Vec<f64> = times.iter().map(|&t| interpolate(&points, t)).collect();
    let sigma = (span * TREND_BANDWIDTH_SHARE_OF_SPAN).min(TREND_MAX_BANDWIDTH_SECONDS);
    let smooth: Vec<f64> = times
        .iter()
        .map(|&t| {
            let (weighted, total) =
                times
                    .iter()
                    .zip(&linear)
                    .fold((0.0, 0.0), |(w, n), (&u, &v)| {
                        let weight = (-((t - u) / sigma).powi(2) / 2.0).exp();
                        (w + weight * v, n + weight)
                    });
            weighted / total
        })
        .collect();

    let first_gap = linear[0] - smooth[0];
    let last_gap = linear[TREND_SAMPLES - 1] - smooth[TREND_SAMPLES - 1];
    times
        .iter()
        .zip(&smooth)
        .enumerate()
        .map(|(k, (&t, &v))| ChartPoint {
            timestamp: t.round() as i64,
            value: v + first_gap + (last_gap - first_gap) * k as f64 / (TREND_SAMPLES - 1) as f64,
        })
        .collect()
}

fn interpolate(points: &[(f64, f64)], t: f64) -> f64 {
    match points.iter().position(|&(time, _)| time >= t) {
        None => points[points.len() - 1].1,
        Some(0) => points[0].1,
        Some(i) => {
            let (t0, v0) = points[i - 1];
            let (t1, v1) = points[i];
            if t1 == t0 {
                v1
            } else {
                v0 + (v1 - v0) * (t - t0) / (t1 - t0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str, share: f64, balance: f64) -> AccountListItem {
        AccountListItem {
            id: id.to_string(),
            name: id.to_string(),
            account_type_id: 9,
            liquidity_type_id: 1,
            ownership_share: share,
            balance: Some(balance),
            unrealized_gain: None,
            holdings_count: None,
            suggested_currency: None,
            property: None,
            linked_property_id: None,
        }
    }

    const DAY: i64 = 86_400;

    fn valuation(day: i64, value: f64) -> ChartPoint {
        ChartPoint {
            timestamp: day * DAY,
            value,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
    }

    #[test]
    fn trend_spreads_a_cliff_between_close_valuations() {
        let line = value_trend(
            &[
                valuation(0, 300000.0),
                valuation(100, 300000.0),
                valuation(103, 330000.0),
            ],
            365 * DAY,
            Some(330000.0),
        );

        assert_eq!(line.len(), 120);
        assert_eq!((line[0].timestamp, line[119].timestamp), (0, 365 * DAY));
        for (k, expected) in [
            (0, 300000.0),
            (30, 311321.9947),
            (60, 329887.1903),
            (90, 329993.9930),
            (119, 330000.0),
        ] {
            assert_close(line[k].value, expected);
        }
        assert!(line.windows(2).all(|w| w[1].value >= w[0].value));
        let max_step = line
            .windows(2)
            .map(|w| w[1].value - w[0].value)
            .fold(0.0, f64::max);
        assert!(max_step < 1300.0, "max step {max_step}");
    }

    #[test]
    fn trend_ends_at_todays_value_for_a_real_history() {
        let line = value_trend(
            &[
                valuation(0, 296000.0),
                valuation(341, 310000.0),
                valuation(397, 328000.0),
            ],
            400 * DAY,
            Some(328000.0),
        );

        for (k, expected) in [
            (0, 296000.0),
            (30, 301136.3605),
            (60, 307311.9406),
            (90, 314033.3317),
            (119, 328000.0),
        ] {
            assert_close(line[k].value, expected);
        }
    }

    #[test]
    fn trend_carries_the_last_valuation_when_todays_value_is_unknown() {
        let line = value_trend(&[valuation(0, 100.0), valuation(10, 120.0)], 20 * DAY, None);
        assert_close(line[119].value, 120.0);
    }

    #[test]
    fn trend_of_a_valuation_made_now_is_a_single_point() {
        assert!(value_trend(&[], 5 * DAY, Some(1.0)).is_empty());
        let line = value_trend(&[valuation(5, 18500.0)], 5 * DAY, Some(18500.0));
        assert_eq!(line.len(), 1);
        assert_close(line[0].value, 18500.0);
    }

    #[test]
    fn create_body_deserializes_as_the_server_request() {
        let body = build_create_property_body(CreatePropertyInput {
            kind: PropertyKind::Vehicle,
            name: "Car".into(),
            currency_asset_id: 45,
            ownership_share: 0.5,
            value: 18500.5,
            purchase: Some(crate::models::PropertyPurchaseInput {
                date: 1_700_000_000,
                price: 21000.0,
            }),
            loan_account_ids: vec!["00000000-0000-0000-0000-000000000002".into()],
        })
        .unwrap();

        let request: CreatePropertyRequestViewModel = serde_json::from_str(&body).unwrap();
        assert_eq!(request.kind, PropertyKindViewModel::Vehicle);
        assert_eq!(request.value.as_decimal().to_string(), "18500.5");
        assert_eq!(
            request.purchase.unwrap().date.unix_timestamp(),
            1_700_000_000
        );
        assert_eq!(request.loan_account_ids.len(), 1);
    }

    #[test]
    fn folds_linked_loans_into_the_property_at_each_accounts_share() {
        let mut accounts = vec![
            account("house", 0.5, 328000.0),
            account("mortgage", 0.5, -178000.0),
            account("second-charge", 1.0, -10000.0),
            account("current", 1.0, 2500.0),
        ];
        let body = r#"{"properties":[{"property_id":"00000000-0000-0000-0000-0000000000aa","kind":"real_estate","name":"House","account_id":"00000000-0000-0000-0000-000000000001","asset_id":7,"currency_asset_id":45,"ownership_share":0.5,"loan_account_ids":["00000000-0000-0000-0000-000000000002","00000000-0000-0000-0000-000000000003"]}]}"#;
        let ids = [
            ("house", "00000000-0000-0000-0000-000000000001"),
            ("mortgage", "00000000-0000-0000-0000-000000000002"),
            ("second-charge", "00000000-0000-0000-0000-000000000003"),
        ];
        for (name, id) in ids {
            accounts.iter_mut().find(|a| a.id == name).unwrap().id = id.to_string();
        }

        attach_properties(&mut accounts, extract_properties(body).unwrap());
        fill_property_figures(&mut accounts);

        let property = accounts[0].property.as_ref().unwrap();
        assert_eq!(property.kind, PropertyKind::RealEstate);
        assert_eq!(property.value, Some(328000.0));
        assert_eq!(property.loan_balance, Some(-188000.0));
        assert_eq!(property.equity, Some(140000.0));
        assert_eq!(property.loan_to_value, Some(188000.0 / 328000.0));
        assert_eq!(property.equity_share, Some(164000.0 - 89000.0 - 10000.0));
        assert_eq!(
            accounts[1].linked_property_id.as_deref(),
            Some("00000000-0000-0000-0000-0000000000aa")
        );
        assert!(accounts[2].linked_property_id.is_some());
        assert!(accounts[3].linked_property_id.is_none() && accounts[3].property.is_none());
    }
}
