use std::collections::HashMap;

use crate::models::finding::{Finding, ItemRef};
use crate::models::output::{ControlTotalKind, ExtractionOutput, SkipClass};
use crate::models::source::{ImportSource, SourceKind};

const MAX_LINES_PER_RECORD: usize = 4;
const MAX_CITATIONS_PER_RECORD: usize = 8;
const OUTPUT_ALLOWANCE: usize = 1000;

pub fn check(source: &ImportSource, output: &ExtractionOutput) -> Vec<Finding> {
    let record_count = source.records.len();
    let citations: usize = output
        .lines
        .iter()
        .map(|l| l.refs.len() + l.date_refs.len())
        .chain(output.balances.iter().map(|b| b.refs.len()))
        .chain(output.control_totals.iter().map(|t| t.refs.len()))
        .chain(output.skipped.iter().map(|s| s.refs.len()))
        .chain(output.period.iter().map(|p| p.refs.len()))
        .sum();
    if output.lines.len() > record_count * MAX_LINES_PER_RECORD + OUTPUT_ALLOWANCE
        || citations > record_count * MAX_CITATIONS_PER_RECORD + OUTPUT_ALLOWANCE
    {
        return vec![Finding::InvalidOutput {
            message: format!(
                "the output has {} lines and {citations} references for {record_count} records; cite each record once",
                output.lines.len()
            ),
            refs: Vec::new(),
        }];
    }

    let mut problems: Vec<(ItemRef, String)> = Vec::new();
    let cites = source.kind != SourceKind::Image;
    let check_refs = |item: ItemRef, refs: &[usize], required: bool, problems: &mut Vec<_>| {
        if required && cites && refs.is_empty() {
            problems.push((item, "refs must name at least one record".to_string()));
        } else if let Some(missing) = refs.iter().find(|r| **r >= record_count) {
            problems.push((
                item,
                format!(
                    "ref {missing} does not exist; refs run from 0 to {}",
                    record_count.saturating_sub(1)
                ),
            ));
        }
    };
    let blank = |value: &str| value.trim().is_empty();

    for (index, line) in output.lines.iter().enumerate() {
        let item = ItemRef::Line(index);
        check_refs(item, &line.refs, true, &mut problems);
        check_refs(item, &line.date_refs, false, &mut problems);
        if blank(&line.account_ref) {
            problems.push((
                item,
                "account_ref is empty; name the account as the document does, or with a short name of your own"
                    .to_string(),
            ));
        }
        if blank(&line.date_text) {
            problems.push((item, "date_text is empty".to_string()));
        }
        if [
            &line.external_id,
            &line.related_asset_ref,
            &line.transaction_key,
        ]
        .into_iter()
        .flatten()
        .any(|value| blank(value))
        {
            problems.push((
                item,
                "leave optional fields out instead of sending them empty".to_string(),
            ));
        }
        if line.legs.is_empty() {
            problems.push((item, "a line needs at least one leg".to_string()));
        }
        for (leg_index, leg) in line.legs.iter().enumerate() {
            if blank(&leg.asset_ref) || blank(&leg.amount_text) {
                problems.push((
                    item,
                    format!("leg {leg_index}: asset_ref and amount_text are required"),
                ));
            }
            if leg.quantity.is_zero() {
                problems.push((
                    item,
                    format!(
                        "leg {leg_index}: quantity is zero; skip a row that moves nothing as \"excluded\" with a reason"
                    ),
                ));
            }
        }
        if line
            .balances
            .iter()
            .any(|b| blank(&b.asset_ref) || blank(&b.amount_text))
        {
            problems.push((item, "balances need asset_ref and amount_text".to_string()));
        }
    }

    let mut first_with_key: HashMap<&str, usize> = HashMap::new();
    for (index, line) in output.lines.iter().enumerate() {
        let Some(key) = line.transaction_key.as_deref() else {
            continue;
        };
        match first_with_key.get(key) {
            Some(&first) if output.lines[first].date != line.date => problems.push((
                ItemRef::Line(index),
                format!(
                    "transaction_key {key:?} is shared with line {first}, which has a different date; the lines of one transaction share a date"
                ),
            )),
            Some(_) => {}
            None => {
                first_with_key.insert(key, index);
            }
        }
    }

    for (index, balance) in output.balances.iter().enumerate() {
        let item = ItemRef::Balance(index);
        check_refs(item, &balance.refs, true, &mut problems);
        if blank(&balance.account_ref)
            || blank(&balance.asset_ref)
            || blank(&balance.date_text)
            || blank(&balance.amount_text)
        {
            problems.push((
                item,
                "a balance needs account_ref, asset_ref, date_text and amount_text".to_string(),
            ));
        }
    }

    for (index, total) in output.control_totals.iter().enumerate() {
        let item = ItemRef::ControlTotal(index);
        check_refs(item, &total.refs, true, &mut problems);
        if blank(&total.account_ref) || blank(&total.amount_text) {
            problems.push((
                item,
                "a control total needs account_ref and amount_text".to_string(),
            ));
        }
        match total.kind {
            ControlTotalKind::RowCount
                if total.quantity.is_sign_negative() || !total.quantity.fract().is_zero() =>
            {
                problems.push((item, "a row count must be a whole number".to_string()))
            }
            ControlTotalKind::DebitTotal | ControlTotalKind::CreditTotal
                if total.quantity.is_sign_negative() =>
            {
                problems.push((
                    item,
                    "state debit and credit totals as positive magnitudes".to_string(),
                ))
            }
            ControlTotalKind::DebitTotal
            | ControlTotalKind::CreditTotal
            | ControlTotalKind::NetTotal
                if total.asset_ref.as_deref().is_none_or(blank) =>
            {
                problems.push((item, "a money total needs asset_ref".to_string()))
            }
            _ => {}
        }
    }

    for (index, skipped) in output.skipped.iter().enumerate() {
        let item = ItemRef::Skipped(index);
        check_refs(item, &skipped.refs, true, &mut problems);
        if skipped.class == SkipClass::Excluded && skipped.reason.as_deref().is_none_or(blank) {
            problems.push((
                item,
                "excluded records need a reason, e.g. \"not yet posted\"".to_string(),
            ));
        }
    }

    if let Some(period) = &output.period {
        check_refs(ItemRef::Period, &period.refs, true, &mut problems);
        if period.from > period.to || blank(&period.from_text) || blank(&period.to_text) {
            problems.push((
                ItemRef::Period,
                "a period needs from_text and to_text, and from must not be after to".to_string(),
            ));
        }
    }

    problems
        .into_iter()
        .map(|(item, message)| Finding::InvalidItem { item, message })
        .collect()
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::macros::date;

    use super::*;
    use crate::models::output::{ExtractedLeg, ExtractedLine};
    use crate::source::text;

    fn source() -> ImportSource {
        text::read("date,amount\n2024-01-02,-3.50\n2024-01-03,10.00\n")
    }

    fn line(refs: Vec<usize>, quantity: Decimal) -> ExtractedLine {
        ExtractedLine {
            refs,
            date_refs: Vec::new(),
            date: date!(2024 - 01 - 02),
            date_text: "2024-01-02".to_string(),
            time: None,
            description: None,
            account_ref: "Current".to_string(),
            transaction_key: None,
            external_id: None,
            related_asset_ref: None,
            legs: vec![ExtractedLeg {
                asset_ref: "EUR".to_string(),
                quantity,
                amount_text: "-3.50".to_string(),
                fee_type: None,
            }],
            balances: Vec::new(),
        }
    }

    fn messages(findings: Vec<Finding>) -> Vec<String> {
        findings
            .into_iter()
            .map(|f| match f {
                Finding::InvalidItem { item, message } => format!("{item:?}: {message}"),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn refs_must_exist_and_quantities_must_move_money() {
        let mut dated_elsewhere = line(vec![1], dec!(1));
        dated_elsewhere.date_refs = vec![9];
        let output = ExtractionOutput {
            lines: vec![
                line(vec![7], dec!(0)),
                line(vec![], dec!(1)),
                dated_elsewhere,
            ],
            ..Default::default()
        };

        assert_eq!(
            messages(check(&source(), &output)),
            vec![
                "Line(0): ref 7 does not exist; refs run from 0 to 2",
                "Line(0): leg 0: quantity is zero; skip a row that moves nothing as \"excluded\" with a reason",
                "Line(1): refs must name at least one record",
                "Line(2): ref 9 does not exist; refs run from 0 to 2",
            ]
        );
    }

    #[test]
    fn lines_of_one_transaction_share_a_date_but_may_span_accounts() {
        let mut from = line(vec![1], dec!(-3.50));
        from.transaction_key = Some("t1".to_string());
        let mut to = line(vec![1], dec!(3.50));
        to.transaction_key = Some("t1".to_string());
        to.account_ref = "Savings".to_string();
        let mut late_fee = line(vec![2], dec!(-1));
        late_fee.transaction_key = Some("t1".to_string());
        late_fee.date = date!(2024 - 01 - 03);
        let output = ExtractionOutput {
            lines: vec![from, to, late_fee],
            ..Default::default()
        };

        assert_eq!(
            messages(check(&source(), &output)),
            vec!["Line(2): transaction_key \"t1\" is shared with line 0, which has a different date; the lines of one transaction share a date"]
        );
    }
}
