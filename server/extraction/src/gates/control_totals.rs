use std::collections::BTreeSet;

use rust_decimal::Decimal;

use super::balances::BalanceGroup;
use crate::models::finding::{Finding, ItemRef};
use crate::models::output::{ControlTotalKind, ExtractionOutput};

pub fn check(output: &ExtractionOutput, evidenced: &BTreeSet<BalanceGroup>) -> Vec<Finding> {
    output
        .control_totals
        .iter()
        .enumerate()
        .filter_map(|(index, total)| {
            let lines = output
                .lines
                .iter()
                .filter(|line| line.account_ref == total.account_ref);
            let quantities = |asset: &str| {
                lines
                    .clone()
                    .flat_map(|line| line.legs.iter())
                    .filter(|leg| leg.asset_ref == asset)
                    .map(|leg| leg.quantity)
                    .collect::<Vec<Decimal>>()
            };
            let asset = total.asset_ref.as_deref();
            let computed = match (total.kind, asset) {
                (ControlTotalKind::RowCount, _) => Decimal::from(lines.clone().count()),
                (ControlTotalKind::DebitTotal, Some(asset)) => quantities(asset)
                    .into_iter()
                    .filter(Decimal::is_sign_negative)
                    .map(|q| q.abs())
                    .sum(),
                (ControlTotalKind::CreditTotal, Some(asset)) => quantities(asset)
                    .into_iter()
                    .filter(Decimal::is_sign_positive)
                    .sum(),
                (ControlTotalKind::NetTotal, Some(asset)) => quantities(asset).into_iter().sum(),
                _ => return None,
            };
            let has_evidence = match asset {
                Some(asset) => evidenced.contains(&(total.account_ref.clone(), asset.to_string())),
                None => evidenced
                    .iter()
                    .any(|(account, _)| *account == total.account_ref),
            };
            (computed != total.quantity).then_some(Finding::ControlTotalMismatch {
                item: ItemRef::ControlTotal(index),
                computed,
                blocking: !has_evidence,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::date;

    use super::*;
    use crate::models::output::{
        ExtractedFeeType, ExtractedLeg, ExtractedLine, StatedControlTotal,
    };

    fn line(legs: Vec<(Decimal, Option<ExtractedFeeType>)>) -> ExtractedLine {
        ExtractedLine {
            refs: vec![1],
            date_refs: Vec::new(),
            date: date!(2024 - 01 - 02),
            date_text: "2024-01-02".to_string(),
            time: None,
            description: None,
            account_ref: "LT01".to_string(),
            transaction_key: None,
            external_id: None,
            related_asset_ref: None,
            legs: legs
                .into_iter()
                .map(|(quantity, fee_type)| ExtractedLeg {
                    asset_ref: "EUR".to_string(),
                    quantity,
                    amount_text: quantity.abs().to_string(),
                    fee_type,
                })
                .collect(),
            balances: Vec::new(),
        }
    }

    fn total(kind: ControlTotalKind, quantity: Decimal) -> StatedControlTotal {
        StatedControlTotal {
            refs: vec![9],
            kind,
            account_ref: "LT01".to_string(),
            asset_ref: (kind != ControlTotalKind::RowCount).then(|| "EUR".to_string()),
            quantity,
            amount_text: quantity.to_string(),
        }
    }

    fn output(totals: Vec<StatedControlTotal>) -> ExtractionOutput {
        ExtractionOutput {
            lines: vec![
                line(vec![
                    (dec!(-12), None),
                    (dec!(-0.50), Some(ExtractedFeeType::Transaction)),
                ]),
                line(vec![(dec!(100), None)]),
                line(vec![(dec!(-3), None)]),
            ],
            control_totals: totals,
            ..Default::default()
        }
    }

    #[test]
    fn totals_that_match_the_lines_pass() {
        let output = output(vec![
            total(ControlTotalKind::DebitTotal, dec!(15.50)),
            total(ControlTotalKind::CreditTotal, dec!(100)),
            total(ControlTotalKind::NetTotal, dec!(84.50)),
            total(ControlTotalKind::RowCount, dec!(3)),
        ]);

        assert!(check(&output, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_mismatch_blocks_only_without_balance_evidence() {
        let output = output(vec![total(ControlTotalKind::DebitTotal, dec!(27.50))]);
        let mismatch = |blocking| {
            vec![Finding::ControlTotalMismatch {
                item: ItemRef::ControlTotal(0),
                computed: dec!(15.50),
                blocking,
            }]
        };

        assert_eq!(check(&output, &BTreeSet::new()), mismatch(true));
        let evidenced = BTreeSet::from([("LT01".to_string(), "EUR".to_string())]);
        assert_eq!(check(&output, &evidenced), mismatch(false));
    }
}
