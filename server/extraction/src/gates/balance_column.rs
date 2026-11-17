use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use super::amounts::numeric_value;
use super::balances::BalanceGroup;
use super::dates::contains_date_shape;
use crate::models::finding::Finding;
use crate::models::output::{ExtractedLine, ExtractionOutput};
use crate::models::source::{ImportSource, SourceKind};

const MIN_PAIRS: usize = 5;
const STEPS_PER_ALLOWED_BREAK: usize = 10;

struct Printed {
    record: usize,
    field: usize,
    value: Decimal,
}

pub fn check(source: &ImportSource, output: &ExtractionOutput) -> Option<Finding> {
    let carried: BTreeSet<(usize, Decimal)> = output
        .lines
        .iter()
        .flat_map(|line| {
            line.balances.iter().flat_map(|balance| {
                let magnitude = printed_magnitude(&balance.amount_text, balance.quantity);
                line.refs.iter().map(move |record| (*record, magnitude))
            })
        })
        .collect();
    let ignored = |unattached: Vec<&Printed>| {
        let field = unattached
            .iter()
            .min_by_key(|printed| printed.record)?
            .field;
        let refs: BTreeSet<usize> = unattached.iter().map(|printed| printed.record).collect();
        Some(Finding::BalanceColumnIgnored {
            field,
            refs: refs.into_iter().collect(),
        })
    };

    let groups: BTreeSet<BalanceGroup> = output
        .lines
        .iter()
        .flat_map(|line| {
            line.legs
                .iter()
                .map(|leg| (line.account_ref.clone(), leg.asset_ref.clone()))
        })
        .collect();
    groups.iter().find_map(|(account_ref, asset_ref)| {
        let lines: Vec<&ExtractedLine> = output
            .lines
            .iter()
            .filter(|line| &line.account_ref == account_ref)
            .filter(|line| line.legs.iter().any(|leg| &leg.asset_ref == asset_ref))
            .collect();
        let printed: Vec<Vec<Printed>> = lines
            .iter()
            .map(|line| {
                line.refs
                    .iter()
                    .flat_map(|record| {
                        let cited = &source.records[*record];
                        let pieces: Vec<&str> = match &cited.fields {
                            Some(fields) => fields.iter().map(String::as_str).collect(),
                            None => cited.text.split_whitespace().collect(),
                        };
                        pieces
                            .into_iter()
                            .enumerate()
                            .filter(|(_, piece)| !contains_date_shape(piece))
                            .filter_map(|(field, piece)| {
                                numeric_value(piece).map(|value| Printed {
                                    record: *record,
                                    field,
                                    value,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .collect();

        let without_a_balance =
            |line: &ExtractedLine| !line.balances.iter().any(|b| &b.asset_ref == asset_ref);
        let table = matches!(source.kind, SourceKind::Delimited | SourceKind::Spreadsheet);

        let mut balance_fields: BTreeMap<usize, usize> = BTreeMap::new();
        for (line, printed) in lines.iter().zip(&printed) {
            for balance in line.balances.iter().filter(|b| &b.asset_ref == asset_ref) {
                let magnitude = printed_magnitude(&balance.amount_text, balance.quantity);
                for found in printed.iter().filter(|p| p.value.abs() == magnitude) {
                    *balance_fields.entry(found.field).or_default() += 1;
                }
            }
        }
        let balance_field = balance_fields
            .iter()
            .max_by_key(|(_, lines)| **lines)
            .map(|(field, _)| *field)
            .filter(|_| table);
        let beside_the_balances: Vec<&Printed> = lines
            .iter()
            .zip(&printed)
            .filter(|(line, _)| without_a_balance(line))
            .flat_map(|(_, printed)| printed.iter().filter(|p| Some(p.field) == balance_field))
            .filter(|p| !carried.contains(&(p.record, p.value.abs())))
            .collect();
        if !beside_the_balances.is_empty() {
            return ignored(beside_the_balances);
        }

        let nets: Vec<Decimal> = lines
            .iter()
            .map(|line| {
                line.legs
                    .iter()
                    .filter(|leg| &leg.asset_ref == asset_ref)
                    .map(|leg| {
                        let magnitude = printed_magnitude(&leg.amount_text, leg.quantity);
                        if leg.quantity.is_sign_negative() {
                            -magnitude
                        } else {
                            magnitude
                        }
                    })
                    .sum()
            })
            .collect();
        let mut dealt_to_other_lines = nets.clone();
        dealt_to_other_lines.reverse();
        let steps = nets
            .iter()
            .filter(|net| !net.is_zero())
            .count()
            .saturating_sub(1);
        let enough = |chained: &[Option<&Printed>]| {
            let lines = chained.iter().flatten().count();
            lines + 1 >= MIN_PAIRS && lines + 1 + 2 * (steps / STEPS_PER_ALLOWED_BREAK) >= steps
        };
        let columns: BTreeSet<Option<usize>> = if table {
            printed.iter().flatten().map(|p| Some(p.field)).collect()
        } else {
            BTreeSet::from([None])
        };
        let chained = columns
            .into_iter()
            .filter_map(|column| {
                let printed: Vec<Vec<&Printed>> = printed
                    .iter()
                    .map(|values| {
                        values
                            .iter()
                            .filter(|p| column.is_none_or(|field| p.field == field))
                            .collect()
                    })
                    .collect();
                let chained = chain(&printed, &nets);
                let by_accident = chain(&printed, &dealt_to_other_lines);
                (enough(&chained) && !enough(&by_accident)).then_some(chained)
            })
            .max_by_key(|chained| chained.iter().flatten().count())?;
        ignored(
            lines
                .iter()
                .zip(chained)
                .filter(|(line, _)| without_a_balance(line))
                .filter_map(|(_, printed)| printed)
                .filter(|p| !carried.contains(&(p.record, p.value.abs())))
                .collect(),
        )
    })
}

fn printed_magnitude(amount_text: &str, quantity: Decimal) -> Decimal {
    numeric_value(amount_text).map_or(quantity.abs(), |printed| printed.abs())
}

fn chain<'a>(printed: &[Vec<&'a Printed>], nets: &[Decimal]) -> Vec<Option<&'a Printed>> {
    let mut printed_by: BTreeMap<Decimal, BTreeSet<usize>> = BTreeMap::new();
    let mut preceding: BTreeMap<Decimal, BTreeSet<usize>> = BTreeMap::new();
    let values = |line: usize| printed[line].iter().filter(|p| !p.value.is_zero());
    for line in 0..printed.len() {
        for p in values(line) {
            printed_by.entry(p.value).or_default().insert(line);
            for before in [p.value - nets[line], p.value + nets[line]] {
                preceding.entry(before).or_default().insert(line);
            }
        }
    }
    let prints = |line: usize, value: &Decimal| {
        printed_by
            .get(value)
            .is_some_and(|lines| lines.contains(&line))
    };
    let step = |lines: Option<&BTreeSet<usize>>, line: usize, value: &Decimal| {
        lines.is_some_and(|lines| {
            lines
                .iter()
                .any(|other| *other != line && (nets[*other].is_zero() || !prints(*other, value)))
        })
    };
    (0..printed.len())
        .map(|line| {
            values(line)
                .find(|p| {
                    !nets[line].is_zero()
                        && [p.value - nets[line], p.value + nets[line]]
                            .iter()
                            .any(|before| {
                                !prints(line, before)
                                    && step(printed_by.get(before), line, &p.value)
                            })
                        && step(preceding.get(&p.value), line, &p.value)
                })
                .copied()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::date;

    use super::*;
    use crate::models::output::{ExtractedLeg, PrintedBalance};
    use crate::models::source::{SourceLocation, SourceRecord};
    use crate::source::text;

    const NEWEST_FIRST: &str = "Date,Type,Description,Debit,Credit,Balance\n\
        18/03/2026,DEB,Shop A,16.00,,1278.42\n\
        17/03/2026,DEB,Shop B,7.00,,1294.42\n\
        17/03/2026,DEB,Shop C,13.00,,1301.42\n\
        16/03/2026,FPI,Salary,,1000.00,1314.42\n\
        15/03/2026,DEB,Shop D,.95,,314.42\n\
        14/03/2026,DEB,Shop E,4.00,,315.37\n";
    const NEWEST_FIRST_ROWS: [(&str, &str); 6] = [
        ("-16.00", "1278.42"),
        ("-7.00", "1294.42"),
        ("-13.00", "1301.42"),
        ("1000.00", "1314.42"),
        ("-0.95", "314.42"),
        ("-4.00", "315.37"),
    ];

    fn line(record: usize, account: &str, amount: &str, balance: Option<&str>) -> ExtractedLine {
        let printed = amount.trim_start_matches('-');
        ExtractedLine {
            refs: vec![record],
            date_refs: Vec::new(),
            date: date!(2026 - 03 - 18),
            date_text: "18/03/2026".to_string(),
            time: None,
            description: None,
            account_ref: account.to_string(),
            transaction_key: None,
            external_id: None,
            related_asset_ref: None,
            legs: vec![ExtractedLeg {
                asset_ref: "GBP".to_string(),
                quantity: amount.parse().unwrap(),
                amount_text: printed.strip_prefix("0").unwrap_or(printed).to_string(),
                fee_type: None,
            }],
            balances: balance
                .map(|balance| PrintedBalance {
                    asset_ref: "GBP".to_string(),
                    quantity: balance.parse().unwrap(),
                    amount_text: balance.to_string(),
                })
                .into_iter()
                .collect(),
        }
    }

    fn output(rows: &[(&str, &str)], carrying: impl Fn(usize) -> bool) -> ExtractionOutput {
        ExtractionOutput {
            lines: rows
                .iter()
                .enumerate()
                .map(|(row, (amount, balance))| {
                    line(row + 1, "acct", amount, carrying(row).then_some(*balance))
                })
                .collect(),
            ..Default::default()
        }
    }

    fn layout(kind: SourceKind, rows: &[&[&str]]) -> ImportSource {
        ImportSource {
            kind,
            records: rows
                .iter()
                .enumerate()
                .map(|(row, fields)| SourceRecord {
                    location: SourceLocation::Page {
                        page: 1,
                        line: row as u32 + 1,
                    },
                    text: fields.join("  "),
                    fields: (kind == SourceKind::Pdf)
                        .then(|| fields.iter().map(|field| field.to_string()).collect()),
                })
                .collect(),
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        }
    }

    #[test]
    fn a_balance_equal_to_the_first_rows_amount_still_starts_the_chain() {
        let rows = [
            ("1000.00", "1000.00"),
            ("-12.50", "987.50"),
            ("-40.00", "947.50"),
            ("-7.25", "940.25"),
            ("250.00", "1190.25"),
            ("-99.99", "1090.26"),
            ("-15.00", "1075.26"),
            ("-3.20", "1072.06"),
            ("-61.00", "1011.06"),
        ];
        let mut text = String::from("Date,Description,Amount,Balance\n");
        for (amount, balance) in rows {
            text.push_str(&format!("18/03/2026,Payment,{amount},{balance}\n"));
        }

        assert!(matches!(
            check(&text::read(&text), &output(&rows, |_| false)),
            Some(Finding::BalanceColumnIgnored { field: 3, .. })
        ));
    }

    #[test]
    fn an_ignored_balance_column_is_found_in_either_row_order() {
        let source = text::read(NEWEST_FIRST);
        let ignoring = output(&NEWEST_FIRST_ROWS, |_| false);

        assert_eq!(
            check(&source, &ignoring),
            Some(Finding::BalanceColumnIgnored {
                field: 5,
                refs: vec![2, 3, 4, 5]
            })
        );

        let every_debit_as_a_credit =
            NEWEST_FIRST_ROWS.map(|(amount, balance)| (amount.trim_start_matches('-'), balance));
        assert!(matches!(
            check(&source, &output(&every_debit_as_a_credit, |_| false)),
            Some(Finding::BalanceColumnIgnored { field: 5, .. })
        ));

        let mut reversed: Vec<&str> = NEWEST_FIRST.lines().collect();
        reversed[1..].reverse();
        let mut rows = NEWEST_FIRST_ROWS;
        rows.reverse();
        assert!(matches!(
            check(&text::read(&reversed.join("\n")), &output(&rows, |_| false)),
            Some(Finding::BalanceColumnIgnored { field: 5, .. })
        ));
    }

    #[test]
    fn a_balance_on_some_lines_is_owed_by_every_line_that_prints_one() {
        let source = text::read(NEWEST_FIRST);

        assert_eq!(
            check(&source, &output(&NEWEST_FIRST_ROWS, |row| row == 0)),
            Some(Finding::BalanceColumnIgnored {
                field: 5,
                refs: vec![2, 3, 4, 5, 6]
            })
        );
        assert_eq!(
            check(&source, &output(&NEWEST_FIRST_ROWS, |row| row < 4)),
            Some(Finding::BalanceColumnIgnored {
                field: 5,
                refs: vec![5, 6]
            })
        );
    }

    #[test]
    fn the_column_that_chains_through_the_most_lines_is_the_one_named() {
        let source = text::read(
            "Date,Type,Amount,Equity,Balance\n\
             05/01/2026,Deposit,5000,5300,5200\n\
             06/01/2026,Open,1000,5300,4200\n\
             06/01/2026,Open,800,5300,3400\n\
             07/01/2026,Open,1200,5300,2200\n\
             20/01/2026,Fee,-0.21,5299.79,2199.79\n\
             21/01/2026,Fee,-0.21,5299.58,2199.58\n\
             23/01/2026,Fee,-0.63,5298.95,2198.95\n\
             13/02/2026,Dividend,0.95,5299.90,2199.90\n\
             02/03/2026,Deposit,750,6049.90,2949.90\n",
        );
        let rows = [
            ("5000", "5200"),
            ("-1000", "4200"),
            ("-800", "3400"),
            ("-1200", "2200"),
            ("-0.21", "2199.79"),
            ("-0.21", "2199.58"),
            ("-0.63", "2198.95"),
            ("0.95", "2199.90"),
            ("750", "2949.90"),
        ];

        assert_eq!(
            check(&source, &output(&rows, |_| false)),
            Some(Finding::BalanceColumnIgnored {
                field: 4,
                refs: vec![2, 3, 4, 5, 6, 7, 8]
            })
        );
        assert_eq!(check(&source, &output(&rows, |_| true)), None);
    }

    #[test]
    fn a_row_left_out_of_the_lines_does_not_hide_the_balance() {
        let amounts = [
            "-3.50", "-12.00", "1500.00", "-42.10", "-7.25", "-60.00", "-18.99", "25.00", "-4.40",
            "-310.00", "-9.75", "-55.20",
        ];
        let mut balance = dec!(204.15);
        let mut text = String::from("Date,Description,Amount,Balance\n");
        let mut lines = Vec::new();
        for (row, amount) in amounts.iter().enumerate() {
            balance += amount.parse::<Decimal>().unwrap();
            text.push_str(&format!("{:02}/01/2024,Shop,{amount},{balance}\n", row + 2));
            lines.push(line(row + 1, "acct", amount, None));
        }
        lines.remove(5);
        let output = ExtractionOutput {
            lines,
            ..Default::default()
        };

        assert!(matches!(
            check(&text::read(&text), &output),
            Some(Finding::BalanceColumnIgnored { field: 3, refs }) if refs.len() == 7
        ));
    }

    #[test]
    fn a_row_shared_by_both_sides_of_a_transfer_owes_its_balance_once() {
        let source = text::read(
            "Date,Description,Amount,Balance\n\
             02/01/2024,Coffee,-3.50,96.50\n\
             03/01/2024,To savings,-50.00,46.50\n\
             04/01/2024,From savings,20.00,66.50\n",
        );
        let mut output = output(
            &[("-3.50", "96.50"), ("-50.00", "46.50"), ("20.00", "66.50")],
            |_| true,
        );
        output.lines.push(line(2, "acct", "50.00", None));
        output.lines.push(line(3, "acct", "-20.00", None));
        for line in &mut output.lines[3..] {
            line.account_ref = "Savings".to_string();
        }
        output.lines.push(line(1, "Savings", "1.00", Some("1.00")));

        assert_eq!(check(&source, &output), None);
    }

    #[test]
    fn interleaved_accounts_and_rows_sorted_by_another_date_still_show_their_balances() {
        let source = text::read(
            "Product,Started,Completed,Amount,Balance\n\
             Savings,01/10,01/10,0.18,2500.18\n\
             Current,01/10,01/10,3120.44,4365.74\n\
             Current,01/10,01/10,-3.55,4362.19\n\
             Savings,02/10,02/10,0.21,2500.39\n\
             Current,02/10,02/10,-1400.00,2962.19\n\
             Current,02/10,03/10,-7.65,2602.95\n\
             Savings,03/10,03/10,0.19,2500.58\n\
             Current,03/10,03/10,-300.00,2662.19\n\
             Savings,03/10,03/10,300.00,2800.58\n\
             Current,03/10,03/10,-43.70,2618.49\n\
             Current,03/10,05/10,-17.90,2493.41\n\
             Current,03/10,03/10,-7.89,2610.60\n\
             Savings,04/10,04/10,0.22,2800.80\n\
             Current,04/10,04/10,-22.25,2580.70\n\
             Savings,05/10,05/10,0.20,2801.00\n\
             Current,05/10,05/10,-9.99,2570.71\n\
             Current,05/10,05/10,-21.94,2548.77\n\
             Current,05/10,05/10,-37.46,2511.31\n\
             Savings,06/10,06/10,0.23,2801.23\n",
        );
        let lines = |carrying: &dyn Fn(&str) -> bool| ExtractionOutput {
            lines: source.records[1..]
                .iter()
                .enumerate()
                .map(|(row, record)| {
                    let fields = record.fields.as_ref().unwrap();
                    line(
                        row + 1,
                        &fields[0],
                        &fields[3],
                        carrying(&fields[0]).then_some(fields[4].as_str()),
                    )
                })
                .collect(),
            ..Default::default()
        };

        assert!(matches!(
            check(&source, &lines(&|_| false)),
            Some(Finding::BalanceColumnIgnored { field: 4, refs }) if refs.len() == 10
        ));
        assert!(matches!(
            check(&source, &lines(&|account| account == "Current")),
            Some(Finding::BalanceColumnIgnored { field: 4, refs }) if refs == [4, 7, 9, 13, 15]
        ));
        assert_eq!(check(&source, &lines(&|_| true)), None);
    }

    #[test]
    fn a_row_number_is_not_a_running_balance() {
        let mut beside_ones = String::from("No,Date,Description,Qty,Amount\n");
        let mut numbered_units = String::from("No,Date,Description,Units\n");
        let (mut coffees, mut units) = (Vec::new(), Vec::new());
        for row in 1..=8 {
            beside_ones.push_str(&format!("{row},2024-01-{:02},Coffee,1,-3.50\n", row + 1));
            coffees.push(line(row, "acct", "-3.50", None));
            let bought = ["2", "3", "1", "2", "1", "3", "2", "1"][row - 1];
            numbered_units.push_str(&format!("{row},2024-01-{:02},Fund,{bought}\n", row + 1));
            units.push(line(row, "acct", bought, None));
        }
        let output = |lines| ExtractionOutput {
            lines,
            ..Default::default()
        };

        assert_eq!(check(&text::read(&beside_ones), &output(coffees)), None);
        assert_eq!(check(&text::read(&numbered_units), &output(units)), None);
    }

    #[test]
    fn rows_that_work_out_their_own_net_and_repeat_each_quarter_carry_no_balance() {
        let payments = [
            ["14 Feb 2025", "40", "0.30", "12.00", "1.80", "10.20"],
            ["03 Mar 2025", "15", "0.62", "9.30", "1.40", "7.90"],
            ["14 May 2025", "40", "0.30", "12.00", "1.80", "10.20"],
            ["03 Jun 2025", "15", "0.62", "9.30", "1.40", "7.90"],
            ["14 Aug 2025", "40", "0.30", "12.00", "1.80", "10.20"],
            ["03 Sep 2025", "15", "0.62", "9.30", "1.40", "7.90"],
            ["14 Nov 2025", "40", "0.30", "12.00", "1.80", "10.20"],
            ["03 Dec 2025", "15", "0.65", "9.75", "1.46", "8.29"],
        ];
        let reading = |legs: &dyn Fn(&[&str; 6]) -> Vec<String>| ExtractionOutput {
            lines: payments
                .iter()
                .enumerate()
                .map(|(row, payment)| {
                    let amounts = legs(payment);
                    let mut line = line(row, "acct", &amounts[0], None);
                    for amount in &amounts[1..] {
                        line.legs.push(ExtractedLeg {
                            asset_ref: "GBP".to_string(),
                            quantity: amount.parse().unwrap(),
                            amount_text: amount.trim_start_matches('-').to_string(),
                            fee_type: None,
                        });
                    }
                    line
                })
                .collect(),
            ..Default::default()
        };
        let net = reading(&|payment| vec![payment[5].to_string()]);
        let gross_less_tax =
            reading(&|payment| vec![payment[3].to_string(), format!("-{}", payment[4])]);
        let net_less_tax =
            reading(&|payment| vec![payment[5].to_string(), format!("-{}", payment[4])]);
        let statement: Vec<&[&str]> = payments.iter().map(|row| row.as_slice()).collect();

        for output in [&net, &gross_less_tax, &net_less_tax] {
            assert_eq!(check(&layout(SourceKind::Pdf, &statement), output), None);
        }

        let mut cash = dec!(250.00);
        let ledger: Vec<Vec<String>> = payments
            .iter()
            .map(|payment| {
                cash += payment[5].parse::<Decimal>().unwrap();
                payment
                    .iter()
                    .map(|field| field.to_string())
                    .chain([cash.to_string()])
                    .collect()
            })
            .collect();
        let ledger: Vec<Vec<&str>> = ledger
            .iter()
            .map(|row| row.iter().map(String::as_str).collect())
            .collect();
        let ledger: Vec<&[&str]> = ledger.iter().map(Vec::as_slice).collect();

        assert!(matches!(
            check(&layout(SourceKind::Pdf, &ledger), &net),
            Some(Finding::BalanceColumnIgnored { field: 6, .. })
        ));
    }

    #[test]
    fn a_balance_is_found_where_fields_shift_and_where_there_are_none() {
        let rows: [&[&str]; 8] = [
            &["02 Jan", "Coffee", "3.50", "996.50"],
            &["03 Jan", "Rent", "Flat 4", "400.00", "596.50"],
            &["04 Jan", "Salary", "1,200.00", "1,796.50"],
            &["Lunch", "12.00", "1,784.50"],
            &["05 Jan", "Books", "20.25", "1,764.25"],
            &["06 Jan", "Bus", "2.40", "1,761.85"],
            &["07 Jan", "Refund", "5.00", "1,766.85"],
            &["08 Jan", "Gym", "30.00", "1,736.85"],
        ];
        let amounts = [
            ("-3.50", "996.50"),
            ("-400.00", "596.50"),
            ("1200.00", "1796.50"),
            ("-12.00", "1784.50"),
            ("-20.25", "1764.25"),
            ("-2.40", "1761.85"),
            ("5.00", "1766.85"),
            ("-30.00", "1736.85"),
        ];
        let lines = |carrying: bool| {
            let mut output = output(&amounts, |_| carrying);
            for line in &mut output.lines {
                line.refs[0] -= 1;
            }
            output
        };

        assert_eq!(
            check(&layout(SourceKind::Pdf, &rows), &lines(false)),
            Some(Finding::BalanceColumnIgnored {
                field: 4,
                refs: vec![1, 2, 3, 4, 5, 6]
            })
        );
        assert_eq!(check(&layout(SourceKind::Pdf, &rows), &lines(true)), None);
        assert_eq!(
            check(&layout(SourceKind::Text, &rows), &lines(false)),
            Some(Finding::BalanceColumnIgnored {
                field: 6,
                refs: vec![1, 2, 3, 4, 5, 6]
            })
        );
    }

    #[test]
    fn a_balance_inside_structured_records_printed_in_minor_units_is_found() {
        let entries: Vec<String> = [
            (-350, 99650),
            (-1200, 98450),
            (250000, 348450),
            (-4599, 343851),
            (-807, 343044),
            (-2500, 340544),
            (-1999, 338545),
        ]
        .iter()
        .enumerate()
        .map(|(index, (amount, balance))| {
            let merchant = if index % 2 == 0 {
                r#""merchant":"Shop","#
            } else {
                ""
            };
            format!(
                r#"{{"id":"t{index}","date":"2024-01-{:02}",{merchant}"amount":{amount},"balance_after":{balance}}}"#,
                index + 2
            )
        })
        .collect();
        let source = text::read(&format!(r#"{{"transactions":[{}]}}"#, entries.join(",")));
        let lines = |carrying: bool| ExtractionOutput {
            lines: source
                .records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.text.contains("balance_after"))
                .map(|(index, record)| {
                    let fields = record.fields.as_ref().unwrap();
                    let (amount, balance) = (&fields[fields.len() - 2], &fields[fields.len() - 1]);
                    let mut line = line(index, "acct", "1", None);
                    line.legs[0].quantity = amount.parse::<Decimal>().unwrap() / dec!(100);
                    line.legs[0].amount_text = amount.clone();
                    if carrying {
                        line.balances.push(PrintedBalance {
                            asset_ref: "GBP".to_string(),
                            quantity: balance.parse::<Decimal>().unwrap() / dec!(100),
                            amount_text: balance.clone(),
                        });
                    }
                    line
                })
                .collect(),
            ..Default::default()
        };

        assert_eq!(lines(false).lines.len(), 7);
        assert!(matches!(
            check(&source, &lines(false)),
            Some(Finding::BalanceColumnIgnored { refs, .. }) if refs.len() == 5
        ));
        assert_eq!(check(&source, &lines(true)), None);
    }
}
