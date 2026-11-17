use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use rust_decimal::Decimal;

use super::amounts::{contains_amount_shape, normalize_digits, numeric_value};
use super::dates::{contains_date_shape, strip_date_shapes};
use crate::models::finding::{Finding, ItemRef};
use crate::models::output::{BalanceKind, ExtractedLine, ExtractionOutput, SkipClass};
use crate::models::source::{ImportSource, SourceLocation, SourceRecord};

static NUMBER_RUNS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9][0-9.,'’\s\u{a0}\u{202f}\u{2009}]*").expect("valid regex"));

pub fn check(source: &ImportSource, output: &ExtractionOutput) -> Vec<Finding> {
    let mut claims: BTreeMap<usize, Vec<ItemRef>> = BTreeMap::new();
    let mut claim = |refs: &[usize], item: ItemRef| {
        for record in refs {
            let items = claims.entry(*record).or_default();
            if !items.contains(&item) {
                items.push(item);
            }
        }
    };
    for (index, line) in output.lines.iter().enumerate() {
        claim(&line.refs, ItemRef::Line(index));
    }
    for (index, balance) in output.balances.iter().enumerate() {
        claim(&balance.refs, ItemRef::Balance(index));
    }
    for (index, total) in output.control_totals.iter().enumerate() {
        claim(&total.refs, ItemRef::ControlTotal(index));
    }
    for (index, skipped) in output.skipped.iter().enumerate() {
        claim(&skipped.refs, ItemRef::Skipped(index));
    }
    if let Some(period) = &output.period {
        claim(&period.refs, ItemRef::Period);
    }
    let date_only: BTreeSet<usize> = output
        .lines
        .iter()
        .flat_map(|line| line.date_refs.iter().copied())
        .collect();

    let mut findings = Vec::new();
    let uncovered: Vec<usize> = (0..source.records.len())
        .filter(|record| !claims.contains_key(record) && !date_only.contains(record))
        .collect();
    if !uncovered.is_empty() {
        findings.push(Finding::UncoveredRecords { refs: uncovered });
    }

    for (record, items) in &claims {
        let keys: Vec<Option<&str>> = items
            .iter()
            .filter_map(|item| match item {
                ItemRef::Line(index) => Some(output.lines[*index].transaction_key.as_deref()),
                _ => None,
            })
            .collect();
        let one_transaction = match keys.as_slice() {
            [] | [_] => true,
            [first, rest @ ..] => first.is_some() && rest.iter().all(|key| key == first),
        };
        let others = items
            .iter()
            .filter(|item| !matches!(item, ItemRef::Line(_) | ItemRef::Period))
            .count();
        if !keys.is_empty() && (others > 0 || !one_transaction) {
            findings.push(Finding::RecordClaimedTwice {
                record: *record,
                items: items.clone(),
            });
        }
    }

    let suspect = suspect_skipped(source, output);
    if output.lines.is_empty() && output.balances.is_empty() {
        let rows: Vec<usize> = (0..source.records.len())
            .filter(|record| !suspect.contains(record))
            .filter(|record| carries_date_and_amount(&source.records[*record]))
            .collect();
        if !rows.is_empty() {
            findings.push(Finding::NothingExtracted { refs: rows });
        }
    }
    findings.extend(skipped_balances(source, output, &suspect));
    if !suspect.is_empty() {
        findings.push(Finding::SuspectSkipped { refs: suspect });
    }
    findings
}

fn skipped_balances(
    source: &ImportSource,
    output: &ExtractionOutput,
    suspect: &[usize],
) -> Vec<Finding> {
    let groups: BTreeSet<(&str, &str)> = output
        .lines
        .iter()
        .flat_map(|line| {
            line.legs
                .iter()
                .map(|leg| (line.account_ref.as_str(), leg.asset_ref.as_str()))
        })
        .collect();
    let mut findings = Vec::new();
    for (account, asset) in groups {
        let lines: Vec<&ExtractedLine> = output
            .lines
            .iter()
            .filter(|line| line.account_ref == account)
            .collect();
        let stated = lines
            .iter()
            .any(|line| line.balances.iter().any(|b| b.asset_ref == asset))
            || output.balances.iter().any(|b| {
                b.account_ref == account
                    && b.asset_ref == asset
                    && b.kind != BalanceKind::ValueTotal
            });
        let rows = lines.iter().flat_map(|line| line.refs.iter().copied());
        let (false, Some(first), Some(last)) = (stated, rows.clone().min(), rows.max()) else {
            continue;
        };
        let amounts: Vec<(usize, Decimal)> = output
            .skipped
            .iter()
            .filter(|skipped| skipped.class != SkipClass::Excluded)
            .flat_map(|skipped| skipped.refs.iter().copied())
            .filter(|record| (*record < first || *record > last) && !suspect.contains(record))
            .filter_map(|record| {
                let pieces = match &source.records[record].fields {
                    Some(fields) => fields.iter().map(String::as_str).collect(),
                    None => vec![source.records[record].text.as_str()],
                };
                let printed: Vec<Decimal> = pieces
                    .into_iter()
                    .flat_map(|piece| {
                        match numeric_value(piece).filter(|_| contains_amount_shape(piece)) {
                            Some(amount) => vec![amount],
                            None => piece
                                .split_whitespace()
                                .filter(|token| contains_amount_shape(token))
                                .filter_map(numeric_value)
                                .collect(),
                        }
                    })
                    .collect();
                let [amount] = printed[..] else {
                    return None;
                };
                Some((record, amount))
            })
            .collect();
        let (debits, credits): (Vec<Decimal>, Vec<Decimal>) = lines
            .iter()
            .flat_map(|line| &line.legs)
            .filter(|leg| leg.asset_ref == asset && !leg.quantity.is_zero())
            .map(|leg| leg.quantity)
            .partition(Decimal::is_sign_negative);
        let debit_total: Decimal = debits.iter().map(Decimal::abs).sum();
        let credit_total: Decimal = credits.iter().sum();
        let lines_net = credit_total - debit_total;
        let a_total = |amount: Decimal| {
            (debits.len() > 1 && amount.abs() == debit_total)
                || (credits.len() > 1 && amount.abs() == credit_total)
        };

        let mut refs = BTreeSet::new();
        for (index, (record, amount)) in amounts.iter().enumerate() {
            for (other_record, other_amount) in &amounts[index + 1..] {
                if !lines_net.is_zero()
                    && (other_amount - amount).abs() == lines_net.abs()
                    && !(a_total(*amount) && a_total(*other_amount))
                {
                    refs.extend([*record, *other_record]);
                }
            }
        }
        if !refs.is_empty() {
            findings.push(Finding::BalancesSkipped {
                account_ref: account.to_string(),
                asset_ref: asset.to_string(),
                refs: refs.into_iter().collect(),
                lines_net,
            });
        }
    }
    findings
}

fn suspect_skipped(source: &ImportSource, output: &ExtractionOutput) -> Vec<usize> {
    let mut table_rows: HashMap<u32, (usize, usize)> = HashMap::new();
    for record in output
        .lines
        .iter()
        .flat_map(|line| line.refs.iter().copied())
    {
        if let Some(page) = source.records[record].page() {
            let range = table_rows.entry(page).or_insert((record, record));
            range.0 = range.0.min(record);
            range.1 = range.1.max(record);
        }
    }
    let running = running_headers_and_footers(source);

    let mut suspect: Vec<usize> = output
        .skipped
        .iter()
        .filter(|skipped| skipped.class != SkipClass::Excluded)
        .flat_map(|skipped| skipped.refs.iter().map(move |r| (skipped.class, *r)))
        .filter(|(class, record)| {
            let outside_table = source.records[*record].page().is_some_and(|page| {
                table_rows
                    .get(&page)
                    .is_none_or(|(first, last)| record < first || record > last)
            });
            let exempt = matches!(class, SkipClass::Header | SkipClass::Footer)
                && outside_table
                && running.contains(record);
            !exempt
        })
        .map(|(_, record)| record)
        .filter(|record| carries_date_and_amount(&source.records[*record]))
        .collect();
    suspect.sort_unstable();
    suspect.dedup();
    suspect
}

fn carries_date_and_amount(record: &SourceRecord) -> bool {
    let Some(fields) = &record.fields else {
        return contains_date_shape(&record.text)
            && contains_amount_shape(&strip_date_shapes(&record.text));
    };
    let packed_dates = record.page().is_none() && !fields.iter().any(|field| is_date(field, false));
    let whole_number = |field: &str| {
        !is_date(field, packed_dates)
            && !field.chars().any(char::is_alphabetic)
            && !field.trim_end().ends_with(['%', '‰'])
            && numeric_value(field).is_some()
    };
    fields.iter().any(|field| is_date(field, packed_dates))
        && fields
            .iter()
            .any(|field| contains_amount_shape(&strip_date_shapes(field)) || whole_number(field))
}

fn is_date(field: &str, packed_dates: bool) -> bool {
    let field = normalize_digits(field.trim());
    if packed_dates && field.len() == 6 && field.chars().all(|c| c.is_ascii_digit()) {
        contains_date_shape(&format!("{}-{}-{}", &field[..2], &field[2..4], &field[4..]))
    } else {
        contains_date_shape(&field)
    }
}

fn running_headers_and_footers(source: &ImportSource) -> BTreeSet<usize> {
    let mut page_lengths: HashMap<u32, u32> = HashMap::new();
    for record in &source.records {
        if let SourceLocation::Page { page, line } = record.location {
            let length = page_lengths.entry(page).or_default();
            *length = (*length).max(line);
        }
    }
    let mut places: HashMap<(String, i64), BTreeSet<u32>> = HashMap::new();
    let mut keys: Vec<(usize, [(String, i64); 2])> = Vec::new();
    for (index, record) in source.records.iter().enumerate() {
        if let SourceLocation::Page { page, line } = record.location {
            let shape = shape(record);
            let from_top = (shape.clone(), i64::from(line));
            let from_bottom = (shape, i64::from(line) - i64::from(page_lengths[&page]) - 1);
            for key in [&from_top, &from_bottom] {
                places.entry(key.clone()).or_default().insert(page);
            }
            keys.push((index, [from_top, from_bottom]));
        }
    }
    keys.into_iter()
        .filter(|(_, keys)| keys.iter().any(|key| places[key].len() > 1))
        .map(|(index, _)| index)
        .collect()
}

fn shape(record: &SourceRecord) -> String {
    let text = normalize_digits(&record.text);
    NUMBER_RUNS
        .replace_all(&text, "#")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::{json, Value};
    use time::macros::date;

    use super::*;
    use crate::models::output::{ExtractedLeg, SkippedRecords, StatedBalance, StatedPeriod};
    use crate::models::source::SourceKind;
    use crate::source::text;

    const SHOPS: [&str; 7] = ["Tesco", "Lidl", "Pret", "Boots", "Greggs", "Aldi", "Asda"];

    fn source() -> ImportSource {
        text::read(
            "Date,Description,Amount\n\
             2024-01-02,Coffee,-3.50\n\
             2024-01-03,Salary,1000.00\n\
             Total 2024-01-31,,996.50\n",
        )
    }

    fn line(refs: Vec<usize>) -> ExtractedLine {
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
                quantity: dec!(-3.50),
                amount_text: "-3.50".to_string(),
                fee_type: None,
            }],
            balances: Vec::new(),
        }
    }

    fn skipped(refs: Vec<usize>, class: SkipClass) -> SkippedRecords {
        SkippedRecords {
            refs,
            class,
            reason: (class == SkipClass::Excluded).then(|| "reverted".to_string()),
        }
    }

    fn statement_pages() -> ImportSource {
        ImportSource {
            kind: SourceKind::Pdf,
            records: (1..=2u32)
                .flat_map(|page| {
                    (1..=12u32).map(move |line| SourceRecord {
                        location: SourceLocation::Page { page, line },
                        text: match line {
                            1 => format!("Balance brought forward 0{page}/12/2023  1,2{page}4.56"),
                            12 => format!("Balance carried forward 1{page}/12/2023  9,8{page}6.54"),
                            _ => format!(
                                "{line:02}/12/2023  {}  1{line}.{page}0",
                                SHOPS[((page * 12 + line) as usize) % SHOPS.len()]
                            ),
                        },
                        fields: None,
                    })
                })
                .collect(),
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        }
    }

    #[test]
    fn every_record_must_be_claimed() {
        let output = ExtractionOutput {
            lines: vec![line(vec![1])],
            skipped: vec![skipped(vec![0], SkipClass::Header)],
            ..Default::default()
        };

        assert_eq!(
            check(&source(), &output),
            vec![Finding::UncoveredRecords { refs: vec![2, 3] }]
        );
    }

    #[test]
    fn a_line_record_cannot_also_be_claimed_by_another_item() {
        let output = ExtractionOutput {
            lines: vec![line(vec![1]), line(vec![1, 2])],
            balances: vec![StatedBalance {
                refs: vec![2],
                kind: BalanceKind::Snapshot,
                account_ref: "Current".to_string(),
                asset_ref: "EUR".to_string(),
                as_of_date: date!(2024 - 01 - 03),
                date_text: "2024-01-03".to_string(),
                quantity: dec!(996.50),
                amount_text: "996.50".to_string(),
            }],
            skipped: vec![skipped(vec![0, 3], SkipClass::Excluded)],
            ..Default::default()
        };

        assert_eq!(
            check(&source(), &output),
            vec![
                Finding::RecordClaimedTwice {
                    record: 1,
                    items: vec![ItemRef::Line(0), ItemRef::Line(1)]
                },
                Finding::RecordClaimedTwice {
                    record: 2,
                    items: vec![ItemRef::Line(1), ItemRef::Balance(0)]
                },
            ]
        );
    }

    #[test]
    fn both_sides_of_a_transfer_may_share_their_record() {
        let mut from = line(vec![1]);
        from.transaction_key = Some("t1".to_string());
        let mut to = line(vec![1]);
        to.transaction_key = Some("t1".to_string());
        to.account_ref = "Savings".to_string();
        let output = ExtractionOutput {
            lines: vec![from, to, line(vec![2])],
            skipped: vec![
                skipped(vec![0], SkipClass::Header),
                skipped(vec![3], SkipClass::Excluded),
            ],
            ..Default::default()
        };

        assert!(check(&source(), &output).is_empty());
    }

    #[test]
    fn any_skipped_record_with_a_date_and_an_amount_is_suspect() {
        let output = ExtractionOutput {
            lines: vec![line(vec![1])],
            skipped: vec![
                skipped(vec![0], SkipClass::Header),
                skipped(vec![2], SkipClass::Noise),
                skipped(vec![3], SkipClass::Footer),
            ],
            ..Default::default()
        };

        assert_eq!(
            check(&source(), &output),
            vec![Finding::SuspectSkipped { refs: vec![2, 3] }]
        );
    }

    #[test]
    fn a_whole_number_inside_running_text_is_not_an_amount() {
        let record = |line: u32, fields: &[&str]| SourceRecord {
            location: SourceLocation::Page { page: 1, line },
            text: fields.join("    "),
            fields: Some(fields.iter().map(|field| field.to_string()).collect()),
        };
        let source = ImportSource {
            kind: SourceKind::Pdf,
            records: vec![
                record(
                    1,
                    &[
                        "27 ORCHARD CLOSE",
                        "Statement period",
                        "13 Sep - 12 Oct 2026",
                    ],
                ),
                record(
                    2,
                    &["Sort code 04-00-75", "Account 31926819", "12 October 2026"],
                ),
                record(3, &["12 Oct", "ATM 2841 HIGH STREET", "2500"]),
                record(4, &["Reference", "250131", "Credit limit", "£6,500.00"]),
            ],
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        };
        let output = ExtractionOutput {
            skipped: vec![skipped(vec![0, 1, 2, 3], SkipClass::Noise)],
            ..Default::default()
        };

        assert_eq!(
            check(&source, &output),
            vec![Finding::SuspectSkipped { refs: vec![2] }]
        );
    }

    #[test]
    fn a_date_packed_into_six_digits_counts_as_a_date() {
        let source = text::read(
            ":20:STMT260131\n\
             :25:NL91ABNA0417164300\n\
             :28C:00001/001\n\
             :60F:C260101EUR1000,00\n\
             :61:2601050105D12,50NTRFNONREF\n\
             :86:Coffee shop\n\
             :61:2601070107C250,NTRFNONREF\n\
             :86:Refund\n\
             :62F:C260131EUR1237,50\n",
        );
        let output = ExtractionOutput {
            skipped: vec![skipped(
                (0..source.records.len()).collect(),
                SkipClass::Noise,
            )],
            ..Default::default()
        };

        assert_eq!(
            check(&source, &output),
            vec![Finding::SuspectSkipped {
                refs: vec![3, 4, 5, 6]
            }]
        );
    }

    #[test]
    fn six_digits_beside_a_written_date_are_an_amount_not_a_second_date() {
        let source = text::read(
            "Date;Text;Amount\n\
             2024-01-05;Coffee;-350\n\
             2024-01-31;Closing balance;151130\n",
        );
        let output = ExtractionOutput {
            lines: vec![line(vec![1])],
            skipped: vec![
                skipped(vec![0], SkipClass::Header),
                skipped(vec![2], SkipClass::Noise),
            ],
            ..Default::default()
        };

        assert_eq!(
            check(&source, &output),
            vec![Finding::SuspectSkipped { refs: vec![2] }]
        );
    }

    #[test]
    fn an_output_that_holds_nothing_is_refused_while_records_look_like_rows() {
        let nothing = |skipped| ExtractionOutput {
            skipped,
            ..Default::default()
        };

        assert_eq!(
            check(
                &source(),
                &nothing(vec![
                    skipped(vec![0], SkipClass::Header),
                    skipped(vec![1, 2, 3], SkipClass::Excluded),
                ])
            ),
            vec![Finding::NothingExtracted {
                refs: vec![1, 2, 3]
            }]
        );
        assert_eq!(
            check(
                &source(),
                &nothing(vec![
                    skipped(vec![0, 1], SkipClass::Header),
                    skipped(vec![2, 3], SkipClass::Excluded),
                ])
            ),
            vec![
                Finding::NothingExtracted { refs: vec![2, 3] },
                Finding::SuspectSkipped { refs: vec![1] }
            ]
        );
        let rows_cited_as_the_period = ExtractionOutput {
            skipped: vec![skipped(vec![0], SkipClass::Header)],
            period: Some(StatedPeriod {
                refs: vec![1, 2, 3],
                from: date!(2024 - 01 - 02),
                from_text: "2024-01-02".to_string(),
                to: date!(2024 - 01 - 31),
                to_text: "2024-01-31".to_string(),
            }),
            ..Default::default()
        };
        assert_eq!(
            check(&source(), &rows_cited_as_the_period),
            vec![Finding::NothingExtracted {
                refs: vec![1, 2, 3]
            }]
        );

        let letter = text::read("Dear customer\nYour new card is on its way.\nPage 1 of 1\n");
        assert!(check(
            &letter,
            &nothing(vec![skipped(vec![0, 1, 2], SkipClass::Noise)])
        )
        .is_empty());
    }

    #[test]
    fn a_rate_or_a_date_packed_with_its_time_is_read_for_what_it_is() {
        let record = |line: u32, fields: &[&str]| SourceRecord {
            location: SourceLocation::Lines {
                first: line,
                last: line,
            },
            text: fields.join(" "),
            fields: Some(fields.iter().map(|field| field.to_string()).collect()),
        };
        let source = ImportSource {
            kind: SourceKind::Text,
            records: vec![
                record(1, &["2024-01-02", "Coffee", "-3.50"]),
                record(2, &["Product to 30 June 2025", "2 Year Fixed Rate 4.29%"]),
                record(3, &["30 June 2025", "4.29%"]),
                record(4, &["20260304120000[0:GMT]", "Tea", "-2.00"]),
                record(5, &["20260304120000[0:GMT]", "Tea", "12"]),
            ],
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        };
        let output = ExtractionOutput {
            lines: vec![line(vec![0])],
            skipped: vec![skipped(vec![1, 2, 3, 4], SkipClass::Noise)],
            ..Default::default()
        };

        assert_eq!(
            check(&source, &output),
            vec![Finding::SuspectSkipped { refs: vec![3, 4] }]
        );
        let plain = text::read("Product to 30 June 2025    2 Year Fixed Rate 4.29%\n");
        assert!(!carries_date_and_amount(&plain.records[0]));
    }

    #[test]
    fn running_page_headers_and_footers_are_exempt() {
        let output = ExtractionOutput {
            lines: (1..=10).chain(13..=22).map(|r| line(vec![r])).collect(),
            skipped: vec![
                skipped(vec![0, 12], SkipClass::Header),
                skipped(vec![11, 23], SkipClass::Footer),
            ],
            ..Default::default()
        };

        assert!(check(&statement_pages(), &output).is_empty());
    }

    #[test]
    fn a_hidden_first_row_of_a_page_is_suspect() {
        let output = ExtractionOutput {
            lines: (2..=10).chain(14..=22).map(|r| line(vec![r])).collect(),
            skipped: vec![
                skipped(vec![0, 12], SkipClass::Header),
                skipped(vec![11, 23], SkipClass::Footer),
                skipped(vec![1, 13], SkipClass::Header),
            ],
            ..Default::default()
        };

        assert_eq!(
            check(&statement_pages(), &output),
            vec![Finding::SuspectSkipped { refs: vec![1, 13] }]
        );
    }

    fn card_output(
        lines: &[(usize, &str, &str, &str)],
        balances: Value,
        skipped: Value,
    ) -> ExtractionOutput {
        let lines: Vec<Value> = lines
            .iter()
            .map(|(record, date, quantity, amount_text)| {
                json!({"refs": [record], "date": format!("2024-04-{}", &date[..2]),
                       "date_text": date, "account_ref": "Card",
                       "legs": [{"asset_ref": "GBP", "quantity": quantity, "amount_text": amount_text}]})
            })
            .collect();
        serde_json::from_value(json!({"lines": lines, "balances": balances, "skipped": skipped}))
            .expect("valid extraction output")
    }

    #[test]
    fn two_skipped_amounts_that_differ_by_the_lines_net_are_the_accounts_opening_and_closing() {
        let source = text::read(
            "Previous balance,\"1,000.00\"\n\
             Payments and credits,\"1,050.00\"\n\
             Purchases and charges,820.00\n\
             New balance,770.00\n\
             Date,Description,Amount\n\
             02/04/2024,Shop,300.00\n\
             05/04/2024,Payment,\"-1,000.00\"\n\
             10/04/2024,Shop,500.00\n\
             12/04/2024,Refund,-50.00\n\
             20/04/2024,Fee,20.00\n",
        );
        let lines = [
            (5, "02/04/2024", "-300.00", "300.00"),
            (6, "05/04/2024", "1000.00", "-1,000.00"),
            (7, "10/04/2024", "-500.00", "500.00"),
            (8, "12/04/2024", "50.00", "-50.00"),
            (9, "20/04/2024", "-20.00", "20.00"),
        ];
        let output = |balances: Value, skipped: Value| card_output(&lines, balances, skipped);

        assert_eq!(
            check(
                &source,
                &output(
                    json!([]),
                    json!([{"refs": [0, 1, 2, 3, 4], "class": "header"}])
                )
            ),
            vec![Finding::BalancesSkipped {
                account_ref: "Card".to_string(),
                asset_ref: "GBP".to_string(),
                refs: vec![0, 3],
                lines_net: dec!(230.00),
            }]
        );

        let balance = |record: usize, kind: &str, date: &str, quantity: &str, text: &str| {
            json!({"refs": [record], "kind": kind, "account_ref": "Card", "asset_ref": "GBP",
                   "as_of_date": date, "date_text": date, "quantity": quantity, "amount_text": text})
        };
        let stated = json!([
            balance(0, "opening", "2024-04-02", "-1000.00", "1,000.00"),
            balance(3, "closing", "2024-04-20", "-770.00", "770.00"),
        ]);
        assert!(check(
            &source,
            &output(stated, json!([{"refs": [1, 2, 4], "class": "header"}]))
        )
        .is_empty());

        let left_out = json!([{"refs": [4], "class": "header"},
                              {"refs": [0, 1, 2, 3], "class": "excluded", "reason": "summary"}]);
        assert!(check(&source, &output(json!([]), left_out)).is_empty());
    }

    #[test]
    fn a_rows_own_details_and_figures_printed_several_to_a_record_are_no_balance() {
        let source = text::read(
            "Date,Description,Amount\n\
             02/04/2024,Card payment,-300.00\n\
             ,Original amount USD,380.00\n\
             05/04/2024,Salary,\"1,000.00\"\n\
             ,Original amount USD,\"1,060.00\"\n\
             20/04/2024,Fee,-20.00\n\
             Fees,20.00,20.00\n\
             Interest,700.00,700.00\n",
        );
        let output = card_output(
            &[
                (1, "02/04/2024", "-300.00", "-300.00"),
                (3, "05/04/2024", "1000.00", "1,000.00"),
                (5, "20/04/2024", "-20.00", "-20.00"),
            ],
            json!([]),
            json!([{"refs": [0, 6, 7], "class": "header"}, {"refs": [2, 4], "class": "noise"}]),
        );

        assert!(check(&source, &output).is_empty());
    }
}
