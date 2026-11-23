use std::collections::BTreeMap;

use serde_path_to_error::Segment;
use time::Date;

use super::balances::end_of_day;
use super::dates::{
    date_matches, day_month_swapped, has_month_name, prints_day_first, runs_one_way,
};
use super::{balance_column, balances, control_totals, coverage, provenance, structure};
use crate::models::accepted::{AcceptedExtraction, DateOrder, ExcludedRecords, ExtractedAssertion};
use crate::models::finding::{ExtractionReport, Finding};
use crate::models::output::{ExtractionOutput, SkipClass};
use crate::models::source::ImportSource;

impl ExtractionOutput {
    pub fn from_json(value: serde_json::Value) -> Result<Self, ExtractionReport> {
        serde_path_to_error::deserialize(&value).map_err(|error| {
            let mut item = Some(&value);
            let mut refs = Vec::new();
            for segment in error.path() {
                item = match segment {
                    Segment::Seq { index } => item.and_then(|node| node.get(index)),
                    Segment::Map { key } => item.and_then(|node| node.get(key)),
                    _ => None,
                };
                let Some(cited) = item.and_then(|node| node.get("refs")?.as_array()) else {
                    continue;
                };
                refs = cited
                    .iter()
                    .filter_map(|r| usize::try_from(r.as_u64()?).ok())
                    .collect();
                break;
            }
            ExtractionReport {
                findings: vec![Finding::InvalidOutput {
                    message: format!("{}: {}", error.path(), error.inner()),
                    refs,
                }],
            }
        })
    }

    pub fn evaluate(
        &self,
        source: &ImportSource,
        uploaded_on: Date,
    ) -> Result<AcceptedExtraction, ExtractionReport> {
        self.evaluate_in(source, uploaded_on, None)
    }

    pub fn evaluate_in_date_order(
        &self,
        source: &ImportSource,
        uploaded_on: Date,
        order: DateOrder,
    ) -> Result<AcceptedExtraction, ExtractionReport> {
        self.evaluate_in(source, uploaded_on, Some(order))
    }

    fn evaluate_in(
        &self,
        source: &ImportSource,
        uploaded_on: Date,
        wanted: Option<DateOrder>,
    ) -> Result<AcceptedExtraction, ExtractionReport> {
        let accepted = self.accept(source, uploaded_on)?;
        let Some(order) = accepted.unsettled_date_order else {
            return Ok(accepted);
        };
        let settled = |accepted: AcceptedExtraction| AcceptedExtraction {
            unsettled_date_order: None,
            ..accepted
        };
        let exchanged = self.with_day_and_month_exchanged();
        let Ok(other) = exchanged.accept(source, uploaded_on) else {
            return Ok(settled(accepted));
        };
        match (self.rows_run_one_way(), exchanged.rows_run_one_way()) {
            (true, false) => Ok(settled(accepted)),
            (false, true) => Ok(settled(other)),
            _ if wanted.is_some_and(|wanted| wanted != order) => Ok(other),
            _ => Ok(accepted),
        }
    }

    fn rows_run_one_way(&self) -> bool {
        let mut by_account: BTreeMap<&str, Vec<(Option<usize>, Date)>> = BTreeMap::new();
        for line in &self.lines {
            by_account
                .entry(line.account_ref.as_str())
                .or_default()
                .push((line.refs.iter().min().copied(), line.date));
        }
        by_account.into_values().all(|mut rows| {
            rows.sort_by_key(|(first_ref, _)| *first_ref);
            let dates: Vec<Date> = rows.into_iter().map(|(_, date)| date).collect();
            runs_one_way(&dates)
        })
    }

    fn with_day_and_month_exchanged(&self) -> ExtractionOutput {
        let exchange = |text: &str, date: Date| {
            if has_month_name(text) {
                date
            } else {
                day_month_swapped(date).unwrap_or(date)
            }
        };
        let mut output = self.clone();
        for line in &mut output.lines {
            line.date = exchange(&line.date_text, line.date);
        }
        for balance in &mut output.balances {
            if let Some(printed) = provenance::printed_date(balance) {
                balance.as_of_date =
                    exchange(&balance.date_text, printed) + (balance.as_of_date - printed);
            }
        }
        if let Some(period) = &mut output.period {
            period.from = exchange(&period.from_text, period.from);
            period.to = exchange(&period.to_text, period.to);
        }
        output
    }

    fn accept(
        &self,
        source: &ImportSource,
        uploaded_on: Date,
    ) -> Result<AcceptedExtraction, ExtractionReport> {
        let findings = structure::check(source, self);
        if !findings.is_empty() {
            return Err(ExtractionReport { findings });
        }

        let mut findings = coverage::check(source, self);
        findings.extend(provenance::check(source, self, uploaded_on));
        let balances = balances::check(self);
        findings.extend(balances.findings);
        findings.extend(balance_column::check(source, self));
        findings.extend(control_totals::check(self, &balances.evidenced));
        if findings.iter().any(Finding::is_blocking) {
            return Err(ExtractionReport { findings });
        }

        let stated: Vec<ExtractedAssertion> = self
            .balances
            .iter()
            .enumerate()
            .filter(|(index, _)| !balances.read_mid_day.contains(index))
            .map(|(index, b)| ExtractedAssertion {
                kind: b.kind,
                account_ref: b.account_ref.clone(),
                asset_ref: b.asset_ref.clone(),
                as_of_date: balances
                    .redated
                    .get(&index)
                    .copied()
                    .unwrap_or(b.as_of_date),
                quantity: b.quantity,
            })
            .collect();
        let derived: Vec<ExtractedAssertion> = balances
            .derived
            .into_iter()
            .filter(|d| {
                !stated.iter().any(|s| {
                    s.account_ref == d.account_ref
                        && s.asset_ref == d.asset_ref
                        && (s.kind == d.kind
                            || end_of_day(s.kind, s.as_of_date) == end_of_day(d.kind, d.as_of_date))
                })
            })
            .collect();
        let excluded = self
            .skipped
            .iter()
            .filter(|s| s.class == SkipClass::Excluded)
            .map(|s| ExcludedRecords {
                refs: s.refs.clone(),
                reason: s.reason.clone().unwrap_or_default(),
                text: s
                    .refs
                    .iter()
                    .map(|r| source.records[*r].text.trim())
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
            .collect();
        Ok(AcceptedExtraction {
            lines: self.lines.clone(),
            assertions: stated.into_iter().chain(derived).collect(),
            end_of_day_balances: balances.end_of_day,
            excluded,
            unsettled_date_order: unsettled_date_order(self),
        })
    }
}

fn unsettled_date_order(output: &ExtractionOutput) -> Option<DateOrder> {
    if !ambiguous_day_month(output) {
        return None;
    }
    output
        .lines
        .iter()
        .map(|line| (line.date_text.as_str(), line.date))
        .chain(output.balances.iter().filter_map(|balance| {
            Some((
                balance.date_text.as_str(),
                provenance::printed_date(balance)?,
            ))
        }))
        .find_map(|(text, date)| prints_day_first(text, date))
        .map(|day_first| {
            if day_first {
                DateOrder::DayFirst
            } else {
                DateOrder::MonthFirst
            }
        })
}

fn ambiguous_day_month(output: &ExtractionOutput) -> bool {
    let numeric: Vec<(&str, Date)> = output
        .lines
        .iter()
        .map(|line| (line.date_text.as_str(), line.date))
        .chain(output.balances.iter().filter_map(|balance| {
            Some((
                balance.date_text.as_str(),
                provenance::printed_date(balance)?,
            ))
        }))
        .chain(output.period.iter().flat_map(|period| {
            [
                (period.from_text.as_str(), period.from),
                (period.to_text.as_str(), period.to),
            ]
        }))
        .filter(|(text, _)| !has_month_name(text))
        .collect();
    numeric
        .iter()
        .any(|(_, date)| date.day() != u8::from(date.month()))
        && numeric.iter().all(|(text, date)| {
            day_month_swapped(*date).is_some_and(|swapped| date_matches(text, swapped))
        })
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use serde_json::{json, Value};
    use time::macros::date;
    use time::Date;

    use super::*;
    use crate::models::finding::ItemRef;
    use crate::models::output::BalanceKind;
    use crate::source::text;

    const UPLOADED: Date = date!(2026 - 11 - 17);

    fn parse(value: Value) -> ExtractionOutput {
        serde_json::from_value(value).expect("valid extraction output")
    }

    fn assertions(accepted: &AcceptedExtraction) -> Vec<(BalanceKind, &str, &str, Date, Decimal)> {
        accepted
            .assertions
            .iter()
            .map(|a| {
                (
                    a.kind,
                    a.account_ref.as_str(),
                    a.asset_ref.as_str(),
                    a.as_of_date,
                    a.quantity,
                )
            })
            .collect()
    }

    fn problems(
        report: &ExtractionReport,
        source: &ImportSource,
        output: &ExtractionOutput,
    ) -> Vec<String> {
        report.feedback(source, output)["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["problem"].as_str().unwrap().to_string())
            .collect()
    }

    fn leg(asset: &str, quantity: &str, text: &str) -> Value {
        json!({"asset_ref": asset, "quantity": quantity, "amount_text": text})
    }

    fn screenshot(rows: [(&str, &str, &str, &str); 3]) -> ExtractionOutput {
        let lines: Vec<Value> = rows
            .iter()
            .map(|(date, date_text, amount, balance)| {
                json!({"date": date, "date_text": date_text, "account_ref": "Current",
                       "legs": [leg("GBP", amount, amount)],
                       "balances": [{"asset_ref": "GBP", "quantity": balance, "amount_text": balance}]})
            })
            .collect();
        parse(json!({
            "lines": lines,
            "balances": [{"kind": "closing", "account_ref": "Current", "asset_ref": "GBP",
                          "as_of_date": "2026-11-17", "date_text": "Balance",
                          "quantity": "96.50", "amount_text": "£96.50"}]
        }))
    }

    #[test]
    fn a_reading_of_an_image_is_held_to_its_balances_and_dates() {
        let image = ImportSource::read(b"\x89PNG\r\n\x1A\nrest").unwrap();
        let read = |today: &str, middle_balance: &str, november: &str| {
            screenshot([
                (today, "Today", "-3.50", "96.50"),
                ("2026-11-16", "Yesterday", "-10.00", middle_balance),
                (november, "14 Nov", "50.00", "110.00"),
            ])
            .evaluate(&image, UPLOADED)
        };

        let accepted = read("2026-11-17", "100.00", "2026-11-14").expect("accepted");
        assert_eq!(
            accepted
                .lines
                .iter()
                .map(|line| line.date)
                .collect::<Vec<_>>(),
            [
                date!(2026 - 11 - 17),
                date!(2026 - 11 - 16),
                date!(2026 - 11 - 14)
            ]
        );

        let refused = |today, middle_balance, november| {
            read(today, middle_balance, november)
                .expect_err("refused")
                .findings
        };
        assert!(matches!(
            refused("2026-11-17", "101.00", "2026-11-14")[..],
            [Finding::BalanceChainBroken { line: 1, .. }]
        ));
        assert_eq!(
            refused("2026-11-17", "100.00", "2025-11-14"),
            vec![Finding::InvalidItem {
                item: ItemRef::Line(2),
                message: "date_text \"14 Nov\" does not print the whole year; nothing in the document prints a year and it was uploaded on 2026-11-17, so it is 2026-11-14, not 2025-11-14".to_string(),
            }]
        );
        assert!(matches!(
            refused("2026-11-01", "100.00", "2026-11-14")[..],
            [Finding::DateNotFound {
                item: ItemRef::Line(0),
                ..
            }]
        ));
    }

    const TURNOVER_EXPORT: &str = "\"Account\",\"\",\"Date\",\"Payee\",\"Details\",\"Amount\",\"Currency\",\"D/C\",\"Id\",\"Code\",\r\n\
        \"LT01\",\"10\",\"2024-08-01\",\"\",\"Opening balance\",\"6.80\",\"EUR\",\"C\",\"\",\"AS\",\r\n\
        \"LT01\",\"20\",\"2024-08-02\",\"\",\"Monthly fee 2024.07\",\"1.00\",\"EUR\",\"D\",\"2024080200001\",\"M\",\r\n\
        \"LT01\",\"20\",\"2024-08-06\",\"EMPLOYER LTD\",\"Salary\",\"212.40\",\"EUR\",\"C\",\"2024080600002\",\"MK\",\r\n\
        \"LT01\",\"82\",\"2024-08-31\",\"\",\"Turnover\",\"1.00\",\"EUR\",\"D\",\"\",\"K2\",\r\n\
        \"LT01\",\"82\",\"2024-08-31\",\"\",\"Turnover\",\"212.40\",\"EUR\",\"C\",\"\",\"K2\",\r\n\
        \"LT01\",\"86\",\"2024-08-31\",\"\",\"Closing balance\",\"218.20\",\"EUR\",\"C\",\"\",\"LS\",\r\n";

    fn turnover_export_output(salary: &str) -> Value {
        json!({
            "lines": [
                {"refs": [2], "date": "2024-08-02", "date_text": "2024-08-02", "account_ref": "LT01",
                 "external_id": "2024080200001", "legs": [leg("EUR", "-1.00", "1.00")]},
                {"refs": [3], "date": "2024-08-06", "date_text": "2024-08-06", "account_ref": "LT01",
                 "external_id": "2024080600002", "description": "EMPLOYER LTD — Salary",
                 "legs": [leg("EUR", salary, salary)]}
            ],
            "balances": [
                {"refs": [1], "kind": "opening", "account_ref": "LT01", "asset_ref": "EUR",
                 "as_of_date": "2024-08-01", "date_text": "2024-08-01", "quantity": "6.80", "amount_text": "6.80"},
                {"refs": [6], "kind": "closing", "account_ref": "LT01", "asset_ref": "EUR",
                 "as_of_date": "2024-08-31", "date_text": "2024-08-31", "quantity": "218.20", "amount_text": "218.20"}
            ],
            "control_totals": [
                {"refs": [4], "kind": "debit_total", "account_ref": "LT01", "asset_ref": "EUR", "quantity": "1.00", "amount_text": "1.00"},
                {"refs": [5], "kind": "credit_total", "account_ref": "LT01", "asset_ref": "EUR", "quantity": "212.40", "amount_text": "212.40"}
            ],
            "skipped": [{"refs": [0], "class": "header"}],
            "period": {"refs": [1, 6], "from": "2024-08-01", "from_text": "2024-08-01", "to": "2024-08-31", "to_text": "2024-08-31"}
        })
    }

    #[test]
    fn a_misread_amount_is_rejected_with_feedback() {
        let source = text::read(TURNOVER_EXPORT);
        let output = parse(turnover_export_output("214.20"));
        let report = output.evaluate(&source, UPLOADED).expect_err("rejected");

        assert_eq!(
            problems(&report, &source, &output),
            vec!["amount_not_found", "balances_disagree"]
        );
        assert_eq!(
            report.feedback(&source, &output)["problems"][0]["records"][0]["ref"],
            json!(3)
        );
    }

    const NEWEST_FIRST: &str = "Date,Type,Description,Debit,Credit,Balance\n\
        18/03/2026,DEB,Shop A,16.00,,1278.42\n\
        17/03/2026,DEB,Shop B,7.00,,1294.42\n\
        17/03/2026,DEB,Shop C,13.00,,1301.42\n\
        15/03/2026,FPI,Salary,,1000.00,1314.42\n\
        14/03/2026,DEB,Shop D,.95,,314.42\n\
        13/03/2026,DEB,Shop E,4.00,,315.37\n";

    fn newest_first_lines(with_balances: bool) -> Value {
        let rows = [
            (1, "18/03/2026", "2026-03-18", "-16.00", "16.00", "1278.42"),
            (2, "17/03/2026", "2026-03-17", "-7.00", "7.00", "1294.42"),
            (3, "17/03/2026", "2026-03-17", "-13.00", "13.00", "1301.42"),
            (
                4,
                "15/03/2026",
                "2026-03-15",
                "1000.00",
                "1000.00",
                "1314.42",
            ),
            (5, "14/03/2026", "2026-03-14", "-0.95", ".95", "314.42"),
            (6, "13/03/2026", "2026-03-13", "-4.00", "4.00", "315.37"),
        ];
        json!(rows
            .iter()
            .map(|(r, text, iso, q, amount, balance)| {
                let mut line = json!({"refs": [r], "date": iso, "date_text": text, "account_ref": "acct",
                                      "legs": [leg("GBP", q, amount)]});
                if with_balances {
                    line["balances"] = json!([{"asset_ref": "GBP", "quantity": balance, "amount_text": balance}]);
                }
                line
            })
            .collect::<Vec<_>>())
    }

    #[test]
    fn leaving_out_a_printed_balance_column_is_refused() {
        let source = text::read(NEWEST_FIRST);
        let output = parse(json!({
            "lines": newest_first_lines(false),
            "skipped": [{"refs": [0], "class": "header"}]
        }));

        let report = output.evaluate(&source, UPLOADED).expect_err("rejected");

        assert_eq!(
            problems(&report, &source, &output),
            vec!["balance_column_ignored"]
        );
    }

    const CARD_EXPORT: &str = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n\
        Card Payment,Current,2026-02-02 08:12:40,2026-02-02 14:05:11,Hosting,-31.62,0.58,EUR,COMPLETED,742.18\n\
        Card Payment,Current,2026-02-06 10:00:00,,Shop,-48.20,0.00,EUR,REVERTED,\n\
        Card Payment,Current,2026-02-11 19:27:52,2026-02-12 03:10:09,Music,-11.99,0.00,EUR,COMPLETED,730.19\n\
        Topup,Current,2026-03-24 18:45:03,2026-03-24 18:45:03,\"Payment from ACME, S.A.\",540.00,0.00,EUR,COMPLETED,1270.19\n";

    #[test]
    fn a_balance_accepted_against_its_printed_date_is_passed_on_at_the_date_it_holds() {
        let source = text::read(
            "Previous balance 05/01/2024,-100.00\nDate,Description,Amount\n28/12/2023,Hotel,-50.00\n\
             10/01/2024,Shop,-20.00\nNew balance 04/02/2024,-170.00\n",
        );
        let balance = |record: usize, kind: &str, date: &str, text: &str, quantity: &str| {
            json!({"refs": [record], "kind": kind, "account_ref": "Card", "asset_ref": "GBP",
                "as_of_date": date, "date_text": text, "quantity": quantity, "amount_text": quantity})
        };
        let output = |closing: Value| {
            parse(json!({
                "lines": [
                    {"refs": [2], "date": "2023-12-28", "date_text": "28/12/2023", "account_ref": "Card",
                     "legs": [leg("GBP", "-50.00", "-50.00")]},
                    {"refs": [3], "date": "2024-01-10", "date_text": "10/01/2024", "account_ref": "Card",
                     "legs": [leg("GBP", "-20.00", "-20.00")]}
                ],
                "balances": [balance(0, "opening", "2024-01-05", "05/01/2024", "-100.00"), closing],
                "skipped": [{"refs": [1], "class": "header"}]
            }))
        };
        let card = "Card";

        let purchase_before_the_period =
            output(balance(4, "closing", "2024-02-04", "04/02/2024", "-170.00"))
                .evaluate(&source, UPLOADED)
                .expect("accepted");
        assert_eq!(
            assertions(&purchase_before_the_period),
            vec![
                (
                    BalanceKind::Opening,
                    card,
                    "GBP",
                    date!(2023 - 12 - 28),
                    dec!(-100.00)
                ),
                (
                    BalanceKind::Closing,
                    card,
                    "GBP",
                    date!(2024 - 02 - 04),
                    dec!(-170.00)
                ),
            ]
        );

        for kind in ["closing", "snapshot"] {
            let closing_given_another_printed_date =
                output(balance(4, kind, "2024-01-05", "05/01/2024", "-170.00"))
                    .evaluate(&source, UPLOADED)
                    .expect("accepted");
            assert_eq!(
                assertions(&closing_given_another_printed_date)[1].3,
                date!(2024 - 01 - 10),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_brokerage_statement_with_trades_dividends_and_withholding_is_accepted() {
        let source = text::read(
            "Date;Action;Symbol;Quantity;Price;Amount;Commission;Cash balance;Holding\n\
             2024-03-01;Deposit;;;;5000.00;;5000.00;\n\
             2024-03-04;Buy;AAPL;10;175.20;-1752.00;-1.00;3247.00;10\n\
             2024-05-16;Dividend;AAPL;;;2.40;;3249.40;10\n\
             2024-05-16;Withholding tax;AAPL;;;-0.36;;3249.04;10\n\
             2024-06-10;Sell;AAPL;4;190.00;760.00;-1.00;4008.04;6\n",
        );
        let output = parse(json!({
            "lines": [
                {"refs": [1], "date": "2024-03-01", "date_text": "2024-03-01", "account_ref": "Broker",
                 "legs": [leg("USD", "5000.00", "5000.00")],
                 "balances": [{"asset_ref": "USD", "quantity": "5000.00", "amount_text": "5000.00"}]},
                {"refs": [2], "date": "2024-03-04", "date_text": "2024-03-04", "account_ref": "Broker",
                 "legs": [leg("USD", "-1752.00", "-1752.00"), leg("AAPL", "10", "10"),
                          {"asset_ref": "USD", "quantity": "-1.00", "amount_text": "-1.00", "fee_type": "transaction"}],
                 "balances": [{"asset_ref": "USD", "quantity": "3247.00", "amount_text": "3247.00"},
                              {"asset_ref": "AAPL", "quantity": "10", "amount_text": "10"}]},
                {"refs": [3], "date": "2024-05-16", "date_text": "2024-05-16", "account_ref": "Broker",
                 "transaction_key": "div-aapl", "related_asset_ref": "AAPL",
                 "legs": [leg("USD", "2.40", "2.40")],
                 "balances": [{"asset_ref": "USD", "quantity": "3249.40", "amount_text": "3249.40"}]},
                {"refs": [4], "date": "2024-05-16", "date_text": "2024-05-16", "account_ref": "Broker",
                 "transaction_key": "div-aapl", "related_asset_ref": "AAPL",
                 "legs": [{"asset_ref": "USD", "quantity": "-0.36", "amount_text": "-0.36", "fee_type": "withholding_tax"}],
                 "balances": [{"asset_ref": "USD", "quantity": "3249.04", "amount_text": "3249.04"}]},
                {"refs": [5], "date": "2024-06-10", "date_text": "2024-06-10", "account_ref": "Broker",
                 "legs": [leg("USD", "760.00", "760.00"), leg("AAPL", "-4", "4"),
                          {"asset_ref": "USD", "quantity": "-1.00", "amount_text": "-1.00", "fee_type": "transaction"}],
                 "balances": [{"asset_ref": "USD", "quantity": "4008.04", "amount_text": "4008.04"},
                              {"asset_ref": "AAPL", "quantity": "6", "amount_text": "6"}]}
            ],
            "skipped": [{"refs": [0], "class": "header"}]
        }));

        let accepted = output.evaluate(&source, UPLOADED).expect("accepted");

        assert_eq!(
            assertions(&accepted),
            vec![
                (
                    BalanceKind::Opening,
                    "Broker",
                    "AAPL",
                    date!(2024 - 03 - 04),
                    dec!(0)
                ),
                (
                    BalanceKind::Closing,
                    "Broker",
                    "AAPL",
                    date!(2024 - 06 - 10),
                    dec!(6)
                ),
                (
                    BalanceKind::Opening,
                    "Broker",
                    "USD",
                    date!(2024 - 03 - 01),
                    dec!(0)
                ),
                (
                    BalanceKind::Closing,
                    "Broker",
                    "USD",
                    date!(2024 - 06 - 10),
                    dec!(4008.04)
                ),
            ]
        );
    }

    #[test]
    fn a_fee_inside_a_printed_total_is_counted_once_where_a_balance_is_printed() {
        let source = text::read(
            "Date,Action,Total,Fee,Balance\n\
             2024-03-01,Deposit,1000.00,,1000.00\n\
             2024-03-04,Buy,779.41,1.17,220.59\n\
             2024-03-05,Sell,389.05,0.58,609.64\n",
        );
        let trade = |record: usize, date: &str, cash: &str, [total, fee, balance]: [&str; 3]| {
            json!({"refs": [record], "date": date, "date_text": date, "account_ref": "Broker",
                   "legs": [leg("GBP", cash, total),
                            {"asset_ref": "GBP", "quantity": format!("-{fee}"),
                             "amount_text": fee, "fee_type": "exchange"}],
                   "balances": [{"asset_ref": "GBP", "quantity": balance, "amount_text": balance}]})
        };
        let output = |buy: &str, sell: &str| {
            parse(json!({
                "lines": [
                    {"refs": [1], "date": "2024-03-01", "date_text": "2024-03-01", "account_ref": "Broker",
                     "legs": [leg("GBP", "1000.00", "1000.00")],
                     "balances": [{"asset_ref": "GBP", "quantity": "1000.00", "amount_text": "1000.00"}]},
                    trade(2, "2024-03-04", buy, ["779.41", "1.17", "220.59"]),
                    trade(3, "2024-03-05", sell, ["389.05", "0.58", "609.64"]),
                ],
                "skipped": [{"refs": [0], "class": "header"}]
            }))
        };

        let fee_counted_twice = output("-779.41", "389.05");
        let report = fee_counted_twice
            .evaluate(&source, UPLOADED)
            .expect_err("rejected");
        assert_eq!(
            problems(&report, &source, &fee_counted_twice),
            vec!["balance_chain_broken"]
        );

        let accepted = output("-778.24", "389.63")
            .evaluate(&source, UPLOADED)
            .expect("accepted");
        assert_eq!(
            assertions(&accepted)[1],
            (
                BalanceKind::Closing,
                "Broker",
                "GBP",
                date!(2024 - 03 - 05),
                dec!(609.64)
            )
        );
    }

    #[test]
    fn rows_that_cross_a_month_settle_the_day_month_order() {
        let source =
            text::read("Date,Amount\n05/10/2024,-1.00\n10/10/2024,-2.00\n02/11/2024,-3.00\n");
        let read = |dates: [&str; 3]| {
            let lines: Vec<Value> = dates
                .iter()
                .zip([
                    ("05/10/2024", "-1.00"),
                    ("10/10/2024", "-2.00"),
                    ("02/11/2024", "-3.00"),
                ])
                .enumerate()
                .map(|(index, (date, (text, amount)))| {
                    json!({"refs": [index + 1], "date": date, "date_text": text, "account_ref": "a",
                           "legs": [leg("EUR", amount, amount)]})
                })
                .collect();
            parse(json!({"lines": lines, "skipped": [{"refs": [0], "class": "header"}]}))
                .evaluate(&source, UPLOADED)
                .expect("accepted")
        };
        let dates = |accepted: &AcceptedExtraction| {
            accepted
                .lines
                .iter()
                .map(|line| line.date)
                .collect::<Vec<_>>()
        };
        let day_first = [
            date!(2024 - 10 - 05),
            date!(2024 - 10 - 10),
            date!(2024 - 11 - 02),
        ];

        let month_first = read(["2024-05-10", "2024-10-10", "2024-02-11"]);
        assert_eq!(dates(&month_first), day_first);
        assert_eq!(month_first.unsettled_date_order, None);

        let as_printed = read(["2024-10-05", "2024-10-10", "2024-11-02"]);
        assert_eq!(dates(&as_printed), day_first);
        assert_eq!(as_printed.unsettled_date_order, None);
    }

    #[test]
    fn the_users_order_reads_dates_the_document_leaves_either_way() {
        let source = text::read("Date,Amount\n02/01/2024,-1.00\n03/01/2024,-2.00\n");
        let output = parse(json!({
            "lines": [
                {"refs": [1], "date": "2024-01-02", "date_text": "02/01/2024", "account_ref": "a",
                 "legs": [leg("EUR", "-1.00", "-1.00")]},
                {"refs": [2], "date": "2024-01-03", "date_text": "03/01/2024", "account_ref": "a",
                 "legs": [leg("EUR", "-2.00", "-2.00")]}
            ],
            "skipped": [{"refs": [0], "class": "header"}]
        }));
        let read = |accepted: AcceptedExtraction| {
            (
                accepted
                    .lines
                    .iter()
                    .map(|line| line.date)
                    .collect::<Vec<_>>(),
                accepted.unsettled_date_order,
            )
        };
        let day_first = (
            vec![date!(2024 - 01 - 02), date!(2024 - 01 - 03)],
            Some(DateOrder::DayFirst),
        );

        assert_eq!(read(output.evaluate(&source, UPLOADED).unwrap()), day_first);
        assert_eq!(
            read(
                output
                    .evaluate_in_date_order(&source, UPLOADED, DateOrder::MonthFirst)
                    .unwrap()
            ),
            (
                vec![date!(2024 - 02 - 01), date!(2024 - 03 - 01)],
                Some(DateOrder::MonthFirst)
            )
        );
        assert_eq!(
            read(
                output
                    .evaluate_in_date_order(&source, UPLOADED, DateOrder::DayFirst)
                    .unwrap()
            ),
            day_first
        );
    }

    #[test]
    fn a_stated_period_settles_the_day_month_order() {
        let source = text::read(
            "Period: 01/01/2024 - 31/01/2024\nDate,Amount\n02/01/2024,-1.00\n03/01/2024,-2.00\n",
        );
        let output = |period: Value| {
            let mut output = json!({
                "lines": [
                    {"refs": [2], "date": "2024-01-02", "date_text": "02/01/2024", "account_ref": "a",
                     "legs": [leg("EUR", "-1.00", "-1.00")]},
                    {"refs": [3], "date": "2024-01-03", "date_text": "03/01/2024", "account_ref": "a",
                     "legs": [leg("EUR", "-2.00", "-2.00")]}
                ],
                "skipped": [{"refs": [1], "class": "header"}, {"refs": [0], "class": "header"}]
            });
            if !period.is_null() {
                output["skipped"] = json!([{"refs": [1], "class": "header"}]);
                output["period"] = period;
            }
            parse(output).evaluate(&source, UPLOADED).expect("accepted")
        };

        assert!(output(Value::Null).unsettled_date_order.is_some());
        assert!(!output(
            json!({"refs": [0], "from": "2024-01-01", "from_text": "01/01/2024",
                           "to": "2024-01-31", "to_text": "31/01/2024"})
        )
        .unsettled_date_order
        .is_some());
    }

    #[test]
    fn a_pdf_style_layout_with_dates_and_balances_once_per_day_is_accepted() {
        let source = text::read(
            "Date          Description               Paid out    Paid in    Balance\n\
             14 Dec 2023   Opening balance                                     100.00\n\
             15 Dec 2023   Coffee shop                   3.50\n\
                           Bookshop                     12.00\n\
                           Refund                                    5.00      89.50\n\
             16 Dec 2023   Bus                           2.00                  87.50\n",
        );
        let output = parse(json!({
            "balances": [
                {"refs": [1], "kind": "snapshot", "account_ref": "Current", "asset_ref": "GBP",
                 "as_of_date": "2023-12-14", "date_text": "14 Dec 2023", "quantity": "100.00", "amount_text": "100.00"}
            ],
            "lines": [
                {"refs": [2], "date": "2023-12-15", "date_text": "15 Dec 2023", "account_ref": "Current",
                 "legs": [leg("GBP", "-3.50", "3.50")]},
                {"refs": [3], "date_refs": [2], "date": "2023-12-15", "date_text": "15 Dec 2023", "account_ref": "Current",
                 "legs": [leg("GBP", "-12.00", "12.00")]},
                {"refs": [4], "date_refs": [2], "date": "2023-12-15", "date_text": "15 Dec 2023", "account_ref": "Current",
                 "legs": [leg("GBP", "5.00", "5.00")],
                 "balances": [{"asset_ref": "GBP", "quantity": "89.50", "amount_text": "89.50"}]},
                {"refs": [5], "date": "2023-12-16", "date_text": "16 Dec 2023", "account_ref": "Current",
                 "legs": [leg("GBP", "-2.00", "2.00")],
                 "balances": [{"asset_ref": "GBP", "quantity": "87.50", "amount_text": "87.50"}]}
            ],
            "skipped": [{"refs": [0], "class": "header"}]
        }));

        let accepted = output.evaluate(&source, UPLOADED).expect("accepted");

        assert_eq!(
            assertions(&accepted),
            vec![
                (
                    BalanceKind::Snapshot,
                    "Current",
                    "GBP",
                    date!(2023 - 12 - 14),
                    dec!(100.00)
                ),
                (
                    BalanceKind::Closing,
                    "Current",
                    "GBP",
                    date!(2023 - 12 - 16),
                    dec!(87.50)
                ),
            ]
        );
    }

    #[test]
    fn structural_problems_are_reported_before_anything_else() {
        let source = text::read(CARD_EXPORT);
        let output = parse(json!({
            "lines": [{"refs": [42], "date": "2026-02-01", "date_text": "x", "account_ref": "", "legs": []}]
        }));

        let report = output.evaluate(&source, UPLOADED).expect_err("rejected");

        assert!(report
            .findings
            .iter()
            .all(|f| matches!(f, Finding::InvalidItem { .. })));
        assert_eq!(report.findings.len(), 3);
    }

    #[test]
    fn leaving_every_row_out_is_not_an_accepted_reading() {
        let source = text::read(
            "Date,Description,Amount,Status\n2024-01-02,Coffee,-3.50,PENDING\n2024-01-03,Shop,-4.00,PENDING\n",
        );
        let pending = |class: &str| {
            parse(json!({
                "skipped": [
                    {"refs": [0], "class": "header"},
                    {"refs": [1, 2], "class": class, "reason": "pending"}
                ]
            }))
        };

        for (class, problem) in [
            ("excluded", "nothing_extracted"),
            ("noise", "suspect_skipped"),
        ] {
            let output = pending(class);
            let report = output.evaluate(&source, UPLOADED).expect_err("rejected");
            assert_eq!(problems(&report, &source, &output), vec![problem]);
        }
    }

    const CARD_SUMMARY: &str = "Card statement\n\
        Statement date,30/04/2024\n\
        Statement period,01/04/2024 - 30/04/2024\n\
        Previous balance,\"1,000.00\"\n\
        Payments and credits,\"1,050.00\"\n\
        Purchases and charges,820.00\n\
        New balance,770.00\n\
        Date,Description,Amount\n\
        31/03/2024,Shop,300.00\n\
        05/04/2024,Payment,\"-1,000.00\"\n\
        10/04/2024,Shop,500.00\n\
        12/04/2024,Refund,-50.00\n\
        20/04/2024,Fee,20.00\n";

    fn card_summary_output(balances: Value, skipped: &[usize]) -> ExtractionOutput {
        let line = |record: usize, date: &str, iso: &str, quantity: &str, text: &str| {
            json!({"refs": [record], "date": iso, "date_text": date, "account_ref": "Card",
                   "legs": [leg("GBP", quantity, text)]})
        };
        parse(json!({
            "lines": [
                line(8, "31/03/2024", "2024-03-31", "-300.00", "300.00"),
                line(9, "05/04/2024", "2024-04-05", "1000.00", "-1,000.00"),
                line(10, "10/04/2024", "2024-04-10", "-500.00", "500.00"),
                line(11, "12/04/2024", "2024-04-12", "50.00", "-50.00"),
                line(12, "20/04/2024", "2024-04-20", "-20.00", "20.00"),
            ],
            "balances": balances,
            "period": {"refs": [2], "from": "2024-04-01", "from_text": "01/04/2024",
                       "to": "2024-04-30", "to_text": "30/04/2024"},
            "skipped": [{"refs": skipped, "class": "header"}]
        }))
    }

    #[test]
    fn a_card_statement_states_its_summary_balances_dated_by_its_period() {
        let source = text::read(CARD_SUMMARY);
        let balances = |opening: &str, closing: &str| {
            json!([
                {"refs": [3], "kind": "opening", "account_ref": "Card", "asset_ref": "GBP",
                 "as_of_date": "2024-04-01", "date_text": "01/04/2024",
                 "quantity": opening, "amount_text": "1,000.00"},
                {"refs": [6], "kind": "closing", "account_ref": "Card", "asset_ref": "GBP",
                 "as_of_date": "2024-04-30", "date_text": "30/04/2024",
                 "quantity": closing, "amount_text": "770.00"}
            ])
        };

        let accepted = card_summary_output(balances("-1000.00", "-770.00"), &[0, 1, 4, 5, 7])
            .evaluate(&source, UPLOADED)
            .expect("accepted");
        assert_eq!(
            assertions(&accepted),
            vec![
                (
                    BalanceKind::Opening,
                    "Card",
                    "GBP",
                    date!(2024 - 03 - 31),
                    dec!(-1000.00)
                ),
                (
                    BalanceKind::Closing,
                    "Card",
                    "GBP",
                    date!(2024 - 04 - 30),
                    dec!(-770.00)
                ),
            ]
        );

        let as_printed = card_summary_output(balances("1000.00", "770.00"), &[0, 1, 4, 5, 7]);
        let report = as_printed
            .evaluate(&source, UPLOADED)
            .expect_err("rejected");
        assert_eq!(
            problems(&report, &source, &as_printed),
            vec!["balances_disagree"]
        );

        let skipped = card_summary_output(json!([]), &[0, 1, 3, 4, 5, 6, 7]);
        let report = skipped.evaluate(&source, UPLOADED).expect_err("rejected");
        assert_eq!(
            report.findings,
            vec![Finding::BalancesSkipped {
                account_ref: "Card".to_string(),
                asset_ref: "GBP".to_string(),
                refs: vec![3, 6],
                lines_net: dec!(230.00),
            }]
        );
    }

    #[test]
    fn an_unreadable_item_is_reported_with_the_records_it_cites() {
        let source = text::read("Date,Amount\n2024-01-02,-1.00\n2024-01-03,n/a\n");
        let report = ExtractionOutput::from_json(json!({
            "lines": [
                {"refs": [1], "date": "2024-01-02", "date_text": "2024-01-02", "account_ref": "a",
                 "legs": [leg("EUR", "-1.00", "-1.00")]},
                {"refs": [2], "date": "2024-01-03", "date_text": "2024-01-03", "account_ref": "a",
                 "legs": [{"asset_ref": "EUR", "quantity": null, "amount_text": "n/a"}]}
            ]
        }))
        .unwrap_err();

        assert!(
            matches!(
                report.findings.as_slice(),
                [Finding::InvalidOutput { message, refs }]
                    if message.starts_with("lines[1].legs[0].quantity: null is not a decimal string")
                        && refs == &[2]
            ),
            "{:?}",
            report.findings
        );
        let feedback = report.feedback(&source, &ExtractionOutput::default());
        assert_eq!(
            feedback["problems"][0]["records"],
            json!([{ "ref": 2, "text": "2024-01-03,n/a" }])
        );
    }

    #[test]
    fn unknown_fields_and_numeric_quantities_are_refused_when_parsing() {
        let unknown = serde_json::from_value::<ExtractionOutput>(json!({"rows": []})).unwrap_err();
        assert!(
            unknown.to_string().contains("unknown field `rows`"),
            "{unknown}"
        );

        let float = serde_json::from_value::<ExtractionOutput>(json!({
            "lines": [{"refs": [1], "date": "2024-01-02", "date_text": "2024-01-02", "account_ref": "a",
                       "legs": [{"asset_ref": "EUR", "quantity": 0.30000000000000004, "amount_text": "0.30"}]}]
        }))
        .unwrap_err();
        assert!(
            float
                .to_string()
                .contains("0.30000000000000004 is not a decimal string")
                && float.to_string().contains("negate with minus(a, b)"),
            "{float}"
        );
    }
}
