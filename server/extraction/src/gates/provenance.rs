use std::collections::{BTreeMap, HashSet};

use rust_decimal::Decimal;
use time::{Date, Duration, Month};

use super::amounts::{
    amount_matches, appears_as_compact_date, appears_as_token, decimal_separator, normalize_digits,
    numeric_value, reads_in_minor_units,
};
use super::dates::{
    date_matches, day_month_swapped, nearest_readings, prints_day_first, prints_full_year,
    prints_no_date, prints_no_year, runs_one_way, time_appears,
};
use crate::models::finding::{Finding, ItemRef};
use crate::models::output::{BalanceKind, ExtractionOutput, StatedBalance};
use crate::models::source::{ImportSource, SourceKind, SourceRecord};

const HALF_A_YEAR: Duration = Duration::days(183);
const DAY_NAMES_REACH: Duration = Duration::days(6);

pub fn check(source: &ImportSource, output: &ExtractionOutput, uploaded_on: Date) -> Vec<Finding> {
    let records: Vec<NormalizedRecord> = source.records.iter().map(NormalizedRecord::new).collect();
    let from_image = source.kind == SourceKind::Image;
    let cited = |refs: &[usize], needle: &str| {
        from_image || refs.iter().any(|r| records[*r].contains(needle))
    };
    let anywhere = |needle: &str| from_image || records.iter().any(|r| r.contains_date(needle));
    let cited_date = |refs: &[usize], needle: &str| {
        from_image || refs.iter().any(|r| records[*r].contains_date(needle))
    };
    let named_day = |date_text: &str, date: Date| {
        from_image
            && prints_no_date(date_text)
            && (uploaded_on - DAY_NAMES_REACH..=uploaded_on).contains(&date)
    };

    let stated: Vec<(Option<&str>, &str, Decimal)> = output
        .lines
        .iter()
        .flat_map(|line| {
            let legs = line
                .legs
                .iter()
                .map(|leg| (&leg.asset_ref, &leg.amount_text, leg.quantity));
            let balances = line
                .balances
                .iter()
                .map(|balance| (&balance.asset_ref, &balance.amount_text, balance.quantity));
            legs.chain(balances)
        })
        .chain(
            output
                .balances
                .iter()
                .map(|balance| (&balance.asset_ref, &balance.amount_text, balance.quantity)),
        )
        .map(|(asset, text, quantity)| (Some(asset.as_str()), text.as_str(), quantity))
        .chain(output.control_totals.iter().map(|total| {
            (
                total.asset_ref.as_deref(),
                total.amount_text.as_str(),
                total.quantity,
            )
        }))
        .collect();
    let separators: HashSet<char> = stated
        .iter()
        .filter_map(|(_, text, _)| decimal_separator(text))
        .collect();
    let separator = match separators.len() {
        1 => separators.into_iter().next(),
        _ => None,
    };
    let read_as_printed: HashSet<&str> = stated
        .iter()
        .filter(|(_, text, quantity)| {
            !quantity.is_zero() && amount_matches(text, *quantity, separator)
        })
        .filter_map(|(asset, _, _)| *asset)
        .collect();
    let amount_found = |refs: &[usize], asset: Option<&str>, text: &str, quantity: Decimal| {
        let in_minor_units = || {
            output.minor_units
                && asset.is_some_and(|asset| {
                    !read_as_printed.contains(asset) && reads_in_minor_units(text, quantity, asset)
                })
        };
        cited(refs, text) && (amount_matches(text, quantity, separator) || in_minor_units())
    };

    let mut amounts = Vec::new();
    let mut dates = Vec::new();
    let mut fees_counted_twice = Vec::new();
    let mut read_dates: Vec<(ItemRef, &str, Date)> = Vec::new();
    let mut missing_amount = |item: ItemRef, amount_text: &str, quantity: Decimal| {
        amounts.push(Finding::AmountNotFound {
            item,
            amount_text: amount_text.to_string(),
            quantity,
        })
    };

    for (index, line) in output.lines.iter().enumerate() {
        let item = ItemRef::Line(index);
        let dated: Vec<usize> = line.refs.iter().chain(&line.date_refs).copied().collect();
        if cited_date(&dated, &line.date_text) && date_matches(&line.date_text, line.date) {
            read_dates.push((item, &line.date_text, line.date));
        } else if !named_day(&line.date_text, line.date)
            && !dated
                .iter()
                .any(|r| records[*r].dated_after_full_date(&line.date_text, line.date))
        {
            dates.push(Finding::DateNotFound {
                item,
                date_text: line.date_text.clone(),
                date: line.date,
            });
        }
        if let Some(time) = line.time {
            let shown = time_appears(&line.date_text, time)
                || dated.iter().any(|r| time_appears(&records[*r].text, time));
            if !shown {
                dates.push(Finding::TimeNotFound {
                    item,
                    time: match time.second() {
                        0 => format!("{:02}:{:02}", time.hour(), time.minute()),
                        second => {
                            format!("{:02}:{:02}:{second:02}", time.hour(), time.minute())
                        }
                    },
                });
            }
        }
        if let Some(external_id) = &line.external_id {
            if !from_image
                && !line
                    .refs
                    .iter()
                    .any(|r| records[*r].contains_id(external_id))
            {
                dates.push(Finding::ExternalIdNotFound {
                    item,
                    external_id: external_id.clone(),
                });
            }
        }
        let prints = |magnitude| {
            line.refs
                .iter()
                .any(|r| records[*r].prints_number(magnitude))
        };
        for leg in &line.legs {
            let found = |quantity| {
                amount_found(&line.refs, Some(&leg.asset_ref), &leg.amount_text, quantity)
            };
            let in_asset = |fee: bool| {
                line.legs
                    .iter()
                    .filter(move |other| other.asset_ref == leg.asset_ref)
                    .filter(move |other| other.fee_type.is_some() == fee)
            };
            let fees: Decimal = in_asset(true).map(|fee| fee.quantity).sum();
            let with_fees_on_top = (leg.quantity + fees + fees).abs();
            let net_of_fees = leg.fee_type.is_none()
                && !fees.is_zero()
                && in_asset(false).count() == 1
                && found(leg.quantity + fees)
                && !prints(with_fees_on_top);
            if !found(leg.quantity) && !net_of_fees {
                missing_amount(item, &leg.amount_text, leg.quantity);
            }
            let combined = if leg.quantity.is_sign_negative() == fees.is_sign_negative() {
                (leg.quantity.abs() - fees.abs()).abs()
            } else {
                leg.quantity.abs() + fees.abs()
            };
            let net = leg.quantity + fees;
            if leg.fee_type.is_none()
                && in_asset(false).count() == 1
                && !line.balances.iter().any(|b| b.asset_ref == leg.asset_ref)
                && !line
                    .legs
                    .iter()
                    .any(|other| other.quantity.abs() == combined)
                && [leg.quantity.abs(), fees.abs(), combined]
                    .into_iter()
                    .all(prints)
                && !prints(net.abs())
            {
                fees_counted_twice.push(Finding::InvalidItem {
                    item,
                    message: format!(
                        "its legs in {asset} add up to {net}, which its records do not print, but they print {combined}: the main leg's {main} and the fee's {fee} combined. One printed amount already has the fee in it, so these legs count the fee twice: the main leg is {combined}, cited where it is printed, and the legs then add up to the printed {main}",
                        asset = leg.asset_ref,
                        main = leg.quantity.abs(),
                        fee = fees.abs(),
                    ),
                });
            }
        }
        for balance in &line.balances {
            if !amount_found(
                &line.refs,
                Some(&balance.asset_ref),
                &balance.amount_text,
                balance.quantity,
            ) {
                missing_amount(item, &balance.amount_text, balance.quantity);
            }
        }
    }

    for (index, balance) in output.balances.iter().enumerate() {
        let item = ItemRef::Balance(index);
        match printed_date(balance).filter(|_| anywhere(&balance.date_text)) {
            Some(printed) => read_dates.push((item, &balance.date_text, printed)),
            None if named_day(&balance.date_text, balance.as_of_date) => {}
            None => dates.push(Finding::DateNotFound {
                item,
                date_text: balance.date_text.clone(),
                date: balance.as_of_date,
            }),
        }
        if !amount_found(
            &balance.refs,
            Some(&balance.asset_ref),
            &balance.amount_text,
            balance.quantity,
        ) {
            missing_amount(item, &balance.amount_text, balance.quantity);
        }
    }

    for (index, total) in output.control_totals.iter().enumerate() {
        if !amount_found(
            &total.refs,
            total.asset_ref.as_deref(),
            &total.amount_text,
            total.quantity,
        ) {
            missing_amount(
                ItemRef::ControlTotal(index),
                &total.amount_text,
                total.quantity,
            );
        }
    }

    let mut period_span = None;
    if let Some(period) = &output.period {
        let ends = [
            (&period.from_text, period.from),
            (&period.to_text, period.to),
        ];
        for (text, date) in ends {
            if anywhere(text) && date_matches(text, date) {
                read_dates.push((ItemRef::Period, text, date));
            } else {
                dates.push(Finding::DateNotFound {
                    item: ItemRef::Period,
                    date_text: text.clone(),
                    date,
                });
            }
        }
        if ends
            .iter()
            .all(|(text, date)| anywhere(text) && prints_full_year(text, *date))
            && period.from <= period.to
        {
            period_span = Some((period.from, period.to));
        }
    }

    amounts.extend(fees_counted_twice);
    amounts.extend(dates);
    let last_day = output
        .balances
        .iter()
        .filter(|balance| {
            balance.kind == BalanceKind::Closing
                && prints_full_year(&balance.date_text, balance.as_of_date)
        })
        .map(|balance| balance.as_of_date)
        .max();
    let undated = from_image.then_some((uploaded_on, uploaded_on));
    amounts.extend(misread_years(
        output,
        &read_dates,
        period_span,
        last_day,
        undated,
    ));
    amounts.extend(mixed_day_month_orders(&read_dates));
    amounts
}

pub fn printed_date(balance: &StatedBalance) -> Option<Date> {
    let day_before = balance
        .as_of_date
        .previous_day()
        .filter(|_| balance.kind == BalanceKind::Opening);
    [Some(balance.as_of_date), day_before]
        .into_iter()
        .flatten()
        .find(|date| date_matches(&balance.date_text, *date))
}

fn misread_years(
    output: &ExtractionOutput,
    read_dates: &[(ItemRef, &str, Date)],
    period_span: Option<(Date, Date)>,
    last_day: Option<Date>,
    undated: Option<(Date, Date)>,
) -> Vec<Finding> {
    let range = |printed: fn(&str, Date) -> bool| {
        let dates = || {
            read_dates
                .iter()
                .filter(|(_, text, date)| printed(text, *date))
                .map(|(_, _, date)| *date)
        };
        dates().min().zip(dates().max())
    };
    let span = period_span
        .or_else(|| range(prints_full_year))
        .or_else(|| range(|text, date| !prints_no_year(text, date)));
    let (span, last_day, dated_by) = match (span, undated) {
        (Some(span), _) => (
            span,
            last_day,
            format!("the document's dates run from {} to {}", span.0, span.1),
        ),
        (None, Some(undated)) => (
            undated,
            Some(undated.1),
            format!(
                "nothing in the document prints a year and it was uploaded on {}",
                undated.1
            ),
        ),
        (None, None) => {
            return read_dates
                .iter()
                .find(|(_, text, date)| prints_no_year(text, *date))
                .map(|(item, text, _)| Finding::InvalidItem {
                    item: *item,
                    message: format!(
                        "date_text {text:?} prints no year and nothing you cite prints one, so the year cannot be checked; state the period, or a balance, from a record of the document that prints a full date"
                    ),
                })
                .into_iter()
                .collect();
        }
    };
    let misread: Vec<(ItemRef, &str, Date, Date)> = read_dates
        .iter()
        .filter_map(|(item, text, date)| {
            let supported = nearest_readings(text, *date, span, last_day);
            let nearest = supported.first().filter(|_| !supported.contains(date))?;
            Some((*item, *text, *date, *nearest))
        })
        .collect();

    let mut past_a_year_end: HashSet<usize> = HashSet::new();
    let mut rows: BTreeMap<&str, Vec<(usize, Date, Date)>> = BTreeMap::new();
    for (item, text, date) in read_dates {
        let ItemRef::Line(index) = item else { continue };
        if period_span.is_none() && prints_no_year(text, *date) {
            let nearest = misread
                .iter()
                .find(|(misread, ..)| misread == item)
                .map_or(*date, |(.., nearest)| *nearest);
            rows.entry(output.lines[*index].account_ref.as_str())
                .or_default()
                .push((*index, *date, nearest));
        }
    }
    for mut rows in rows.into_values() {
        rows.sort_by_key(|(index, ..)| output.lines[*index].refs.iter().min().copied());
        let given: Vec<Date> = rows.iter().map(|(_, date, _)| *date).collect();
        let nearest: Vec<Date> = rows.iter().map(|(.., nearest)| *nearest).collect();
        if runs_one_way(&given)
            && !runs_one_way(&nearest)
            && given
                .iter()
                .zip(&nearest)
                .any(|(given, nearest)| given == nearest)
            && given
                .iter()
                .all(|date| last_day.is_none_or(|last| *date <= last))
            && given
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < Duration::days(365))
        {
            past_a_year_end.extend(rows.iter().map(|(index, ..)| *index));
        }
    }

    misread
        .into_iter()
        .filter(|(item, ..)| {
            !matches!(item, ItemRef::Line(index) if past_a_year_end.contains(index))
        })
        .map(|(item, text, date, nearest)| Finding::InvalidItem {
            item,
            message: format!(
                "date_text {text:?} does not print the whole year; {dated_by}, so it is {nearest}, not {date}"
            ),
        })
        .collect()
}

fn mixed_day_month_orders(read_dates: &[(ItemRef, &str, Date)]) -> Vec<Finding> {
    let other_reading = |text: &str, date: Date| {
        day_month_swapped(date).filter(|swapped| date_matches(text, *swapped))
    };
    let settled: HashSet<bool> = read_dates
        .iter()
        .filter(|(_, text, date)| other_reading(text, *date).is_none())
        .filter_map(|(_, text, date)| prints_day_first(text, *date))
        .collect();
    let (Some(day_first), 1) = (settled.iter().next().copied(), settled.len()) else {
        return Vec::new();
    };
    read_dates
        .iter()
        .filter(|(_, text, date)| prints_day_first(text, *date) == Some(!day_first))
        .filter_map(|(item, text, date)| {
            let consistent = other_reading(text, *date)?;
            Some(Finding::InvalidItem {
                item: *item,
                message: format!(
                    "date_text {text:?} is read {} first, but dates in this document that read only one way are {} first; in that order it is {consistent}, not {date}",
                    if day_first { "month" } else { "day" },
                    if day_first { "day" } else { "month" },
                ),
            })
        })
        .collect()
}

struct NormalizedRecord {
    text: String,
    fields: Vec<String>,
}

impl NormalizedRecord {
    fn new(record: &SourceRecord) -> Self {
        Self {
            text: normalize_digits(&record.text),
            fields: record
                .fields
                .iter()
                .flatten()
                .map(|f| normalize_digits(f))
                .collect(),
        }
    }

    fn contains(&self, needle: &str) -> bool {
        let needle = normalize_digits(needle.trim());
        !needle.is_empty()
            && (appears_as_token(&self.text, &needle)
                || self.fields.iter().any(|f| appears_as_token(f, &needle)))
    }

    fn contains_date(&self, needle: &str) -> bool {
        let needle = normalize_digits(needle.trim());
        let compact = matches!(needle.len(), 6 | 8) && needle.chars().all(|c| c.is_ascii_digit());
        if !compact {
            return self.contains(&needle);
        }
        appears_as_compact_date(&self.text, &needle)
            || self
                .fields
                .iter()
                .any(|f| appears_as_compact_date(f, &needle))
    }

    fn prints_number(&self, magnitude: Decimal) -> bool {
        let prints = |piece: &str| numeric_value(piece).is_some_and(|n| n.abs() == magnitude);
        if self.fields.is_empty() {
            self.text.split_whitespace().any(prints)
        } else {
            self.fields.iter().any(|field| prints(field))
        }
    }

    fn dated_after_full_date(&self, date_text: &str, date: Date) -> bool {
        let short = normalize_digits(date_text.trim());
        if short.len() != 4 || !short.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        let in_text = self
            .text
            .split(|c: char| !c.is_ascii_digit())
            .filter_map(|digits| digits.strip_suffix(short.as_str()));
        let in_fields = self
            .fields
            .windows(2)
            .filter(|pair| pair[1].trim() == short)
            .map(|pair| pair[0].trim());
        let number = |digits: &str| digits.parse::<u8>().ok();
        let reads =
            |year: &str, (month, day): (&str, &str), (short_month, short_day): (&str, &str)| {
                number(short_month) == Some(u8::from(date.month()))
                    && number(short_day) == Some(date.day())
                    && (date.year() - 1..=date.year() + 1).any(|full_year| {
                        let full = number(month)
                            .and_then(|month| Month::try_from(month).ok())
                            .zip(number(day))
                            .and_then(|(month, day)| {
                                Date::from_calendar_date(full_year, month, day).ok()
                            });
                        year.parse() == Ok(full_year.rem_euclid(10_i32.pow(year.len() as u32)))
                            && full.is_some_and(|full| (date - full).abs() < HALF_A_YEAR)
                    })
            };
        let short_parts = short.split_at(2);
        in_text
            .chain(in_fields)
            .filter(|full| matches!(full.len(), 6 | 8) && full.chars().all(|c| c.is_ascii_digit()))
            .any(|full| {
                let (year_first, month_day) = full.split_at(full.len() - 4);
                let (head, year_last) = full.split_at(4);
                let (first, second) = head.split_at(2);
                reads(year_first, month_day.split_at(2), short_parts)
                    || reads(year_last, (first, second), short_parts)
                    || reads(year_last, (second, first), (short_parts.1, short_parts.0))
            })
    }

    fn contains_id(&self, id: &str) -> bool {
        let id = id.trim();
        let bounded = |haystack: &str| {
            haystack.match_indices(id).any(|(start, _)| {
                let before = haystack[..start].chars().next_back();
                let after = haystack[start + id.len()..].chars().next();
                !before.is_some_and(char::is_alphanumeric)
                    && !after.is_some_and(char::is_alphanumeric)
            })
        };
        !id.is_empty() && (bounded(&self.text) || self.fields.iter().any(|f| bounded(f)))
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::{json, Value};
    use time::macros::{date, time};

    use super::*;
    use crate::models::output::{BalanceKind, ExtractedLeg, ExtractedLine, StatedBalance};
    use crate::models::source::SourceLocation;
    use crate::source::text;

    const UPLOADED: Date = date!(2026 - 11 - 17);

    fn check(source: &ImportSource, output: &ExtractionOutput) -> Vec<Finding> {
        super::check(source, output, UPLOADED)
    }

    fn source() -> ImportSource {
        text::read(
            "Date,Description,Amount,Balance,Id\n\
             2024-01-02 09:15,Coffee,-11.00,989.00,TX-1001\n\
             2024-01-03 17:40,Salary,\"2,346.75\",\"3,335.75\",TX-1002\n",
        )
    }

    fn line(
        refs: Vec<usize>,
        date_text: &str,
        amount_text: &str,
        quantity: Decimal,
    ) -> ExtractedLine {
        ExtractedLine {
            refs,
            date_refs: Vec::new(),
            date: date!(2024 - 01 - 02),
            date_text: date_text.to_string(),
            time: None,
            description: None,
            account_ref: "Current".to_string(),
            transaction_key: None,
            external_id: None,
            related_asset_ref: None,
            legs: vec![ExtractedLeg {
                asset_ref: "EUR".to_string(),
                quantity,
                amount_text: amount_text.to_string(),
                fee_type: None,
            }],
            balances: Vec::new(),
        }
    }

    fn output(value: Value) -> ExtractionOutput {
        serde_json::from_value(value).expect("valid extraction output")
    }

    fn row(record: usize, date: &str, date_text: &str, leg: (&str, &str, &str)) -> Value {
        json!({"refs": [record], "date": date, "date_text": date_text, "account_ref": "Current",
               "legs": [{"asset_ref": leg.0, "quantity": leg.1, "amount_text": leg.2}]})
    }

    fn messages(findings: &[Finding]) -> Vec<&str> {
        findings
            .iter()
            .map(|finding| match finding {
                Finding::InvalidItem { message, .. } => message.as_str(),
                other => panic!("unexpected finding {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_decimal_comma_file_cannot_pass_a_hundred_times_too_large() {
        let source = text::read(
            "Data;Aprasymas;Suma;Likutis\n\
             2024-01-02;Kava;-3,50;96,50\n\
             2024-01-03;Alga;1000,00;1096,50\n",
        );
        let lines = |coffee: &str, salary: &str| {
            output(json!({"lines": [
                row(1, "2024-01-02", "2024-01-02", ("EUR", coffee, "-3,50")),
                row(2, "2024-01-03", "2024-01-03", ("EUR", salary, "1000,00")),
            ]}))
        };

        assert!(check(&source, &lines("-3.50", "1000.00")).is_empty());
        assert_eq!(check(&source, &lines("-350", "100000")).len(), 2);
    }

    #[test]
    fn a_number_that_reads_both_ways_follows_the_other_amounts_of_the_file() {
        let source = text::read(
            "Date,Description,Amount\n\
             2024-01-02,Laptop,\"1,234\"\n\
             2024-01-03,Coffee,3.50\n",
        );
        let lines = |laptop: &str| {
            output(json!({"lines": [
                row(1, "2024-01-02", "2024-01-02", ("USD", laptop, "1,234")),
                row(2, "2024-01-03", "2024-01-03", ("USD", "-3.50", "3.50")),
            ]}))
        };

        assert!(check(&source, &lines("-1234")).is_empty());
        assert_eq!(
            check(&source, &lines("-1.234")),
            vec![Finding::AmountNotFound {
                item: ItemRef::Line(0),
                amount_text: "1,234".to_string(),
                quantity: dec!(-1.234),
            }]
        );
    }

    #[test]
    fn amounts_printed_in_minor_units_are_read_at_the_currency_scale_throughout() {
        let source = text::read(
            "date,description,amount,balance,currency\n\
             2024-01-02,Coffee,-350,99650,GBP\n\
             2024-01-03,Salary,100000,199650,GBP\n",
        );
        let stated = |minor_units: bool, asset: &str, coffee: &str, salary: &str, balance: &str| {
            let mut second = row(2, "2024-01-03", "2024-01-03", (asset, salary, "100000"));
            second["balances"] =
                json!([{"asset_ref": asset, "quantity": balance, "amount_text": "199650"}]);
            output(json!({"minor_units": minor_units, "lines": [
                row(1, "2024-01-02", "2024-01-02", (asset, coffee, "-350")),
                second,
            ]}))
        };
        let lines = |asset: &str, coffee: &str, salary: &str, balance: &str| {
            stated(true, asset, coffee, salary, balance)
        };
        let refused = |output: &ExtractionOutput| {
            check(&source, output)
                .iter()
                .map(|finding| match finding {
                    Finding::AmountNotFound { quantity, .. } => *quantity,
                    other => panic!("unexpected finding {other:?}"),
                })
                .collect::<Vec<_>>()
        };

        assert!(refused(&lines("GBP", "-3.50", "1000.00", "1996.50")).is_empty());
        assert!(refused(&lines("GBP", "-350", "100000", "199650")).is_empty());
        assert!(refused(&lines("KWD", "-0.350", "100.000", "199.650")).is_empty());
        assert_eq!(
            refused(&stated(false, "GBP", "-3.50", "1000.00", "1996.50")),
            vec![dec!(-3.50), dec!(1000.00), dec!(1996.50)]
        );
        assert_eq!(
            refused(&lines("GBP", "-3.50", "100000", "199650")),
            vec![dec!(-3.50)]
        );
        assert_eq!(
            refused(&lines("GBP", "-3.50", "1000.00", "199650")),
            vec![dec!(-3.50), dec!(1000.00)]
        );
        assert_eq!(
            refused(&lines("GBP", "-0.350", "100.000", "199.650")),
            vec![dec!(-0.350), dec!(100.000), dec!(199.650)]
        );
        assert_eq!(
            refused(&lines("GBP", "-35000", "10000000", "19965000")),
            vec![dec!(-35000), dec!(10000000), dec!(19965000)]
        );
        for not_a_currency_with_cents in ["JPY", "IBM", "VWRL", "US0378331005"] {
            assert_eq!(
                refused(&lines(
                    not_a_currency_with_cents,
                    "-3.50",
                    "1000.00",
                    "1996.50"
                )),
                vec![dec!(-3.50), dec!(1000.00), dec!(1996.50)],
                "{not_a_currency_with_cents}"
            );
        }
    }

    #[test]
    fn a_printed_fraction_is_never_read_in_minor_units() {
        let source = text::read("date,description,amount\n2024-01-02,Coffee,3.50\n");
        let coffee = |quantity: &str| {
            output(json!({"minor_units": true, "lines": [
                row(1, "2024-01-02", "2024-01-02", ("GBP", quantity, "3.50")),
            ]}))
        };

        assert!(check(&source, &coffee("-3.50")).is_empty());
        assert_eq!(check(&source, &coffee("-0.035")).len(), 1);
    }

    const TOTALS_WITH_FEES: &str = "Date,Action,Units,Total,Fee\n\
         2024-03-04,Buy,4.2135,779.41,1.17\n\
         2024-03-05,Sell,3.25,389.05,0.58\n";

    fn with_fee(record: usize, legs: &[(&str, &str, &str)], fee: (&str, &str, &str)) -> Value {
        let mut legs: Vec<Value> = legs
            .iter()
            .chain([&fee])
            .map(|(asset, quantity, text)| {
                json!({"asset_ref": asset, "quantity": quantity, "amount_text": text})
            })
            .collect();
        if let Some(fee) = legs.last_mut() {
            fee["fee_type"] = json!("exchange");
        }
        json!({"refs": [record], "date": "2024-03-04", "date_text": "2024-03-04",
               "account_ref": "Broker", "legs": legs})
    }

    fn amounts_not_found(source: &ImportSource, lines: Value) -> Vec<Decimal> {
        check(source, &output(json!({ "lines": lines })))
            .iter()
            .filter_map(|finding| match finding {
                Finding::AmountNotFound { quantity, .. } => Some(*quantity),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_fee_is_not_taken_out_of_an_amount_the_row_also_prints_with_the_fee_added() {
        let source = text::read(
            "Date,Action,Amount,Fee,Total\n\
             2024-03-04,Buy,250.00,1.50,251.50\n\
             2024-03-05,Sell,120.00,0.75,119.25\n",
        );
        let taken_out_of_the_amount = json!([
            with_fee(1, &[("GBP", "-248.50", "250.00")], ("GBP", "-1.50", "1.50")),
            with_fee(2, &[("GBP", "120.75", "120.00")], ("GBP", "-0.75", "0.75")),
        ]);
        let taken_out_of_the_total = json!([
            with_fee(1, &[("GBP", "-250.00", "251.50")], ("GBP", "-1.50", "1.50")),
            with_fee(2, &[("GBP", "120.00", "119.25")], ("GBP", "-0.75", "0.75")),
        ]);

        assert_eq!(
            amounts_not_found(&source, taken_out_of_the_amount),
            vec![dec!(-248.50), dec!(120.75)]
        );
        assert!(amounts_not_found(&source, taken_out_of_the_total).is_empty());
    }

    #[test]
    fn a_leg_net_of_fees_is_still_made_of_printed_amounts() {
        let source = text::read(TOTALS_WITH_FEES);
        let refused = |legs: &[(&str, &str, &str)], fee| {
            amounts_not_found(&source, json!([with_fee(1, legs, fee)]))
        };
        let fee = ("GBP", "-1.17", "1.17");

        assert_eq!(
            refused(&[("GBP", "-700.00", "779.41")], fee),
            vec![dec!(-700.00)]
        );
        assert_eq!(
            refused(&[("GBP", "-777.41", "779.41")], ("GBP", "-2.00", "1.17")),
            vec![dec!(-2.00)]
        );
        assert_eq!(
            refused(&[("GBP", "-778.24", "779.41")], ("USD", "-1.17", "1.17")),
            vec![dec!(-778.24)]
        );
        assert_eq!(
            refused(
                &[("GBP", "-778.24", "779.41"), ("GBP", "-778.24", "779.41")],
                fee
            ),
            vec![dec!(-778.24), dec!(-778.24)]
        );
        assert_eq!(
            amounts_not_found(
                &source,
                json!([row(
                    1,
                    "2024-03-04",
                    "2024-03-04",
                    ("GBP", "-778.24", "779.41")
                )])
            ),
            vec![dec!(-778.24)]
        );
    }

    #[test]
    fn a_line_moves_the_account_by_a_printed_amount_when_its_row_relates_amount_fee_and_total() {
        let source = text::read(
            "Date,Description,Amount,Fee,Total,Balance\n\
             2024-03-04,ACME dividend,8.40,1.26,7.14,\n\
             2024-03-05,Card payment,60.00,0.90,60.90,\n\
             2024-03-06,Transfer,25.00,0.30,,\n\
             2024-03-07,ACME dividend,,1.26,7.14,\n\
             2024-03-08,ACME dividend,8.40,1.26,7.14,107.14\n",
        );
        let line = |record: usize, main: (&str, &str), fee: (&str, &str)| {
            let date = format!("2024-03-0{}", record + 3);
            json!({"refs": [record], "date": date, "date_text": date, "account_ref": "Broker",
                   "legs": [{"asset_ref": "USD", "quantity": main.0, "amount_text": main.1},
                            {"asset_ref": "USD", "quantity": fee.0, "amount_text": fee.1,
                             "fee_type": "transaction"}]})
        };
        let refused = |record: usize, main: (&str, &str), fee: (&str, &str)| {
            check(
                &source,
                &output(json!({ "lines": [line(record, main, fee)] })),
            )
        };

        assert_eq!(
            messages(&refused(1, ("7.14", "7.14"), ("-1.26", "1.26"))),
            vec!["its legs in USD add up to 5.88, which its records do not print, but they print 8.40: the main leg's 7.14 and the fee's 1.26 combined. One printed amount already has the fee in it, so these legs count the fee twice: the main leg is 8.40, cited where it is printed, and the legs then add up to the printed 7.14"]
        );
        assert!(refused(1, ("8.40", "8.40"), ("-1.26", "1.26")).is_empty());
        assert!(refused(1, ("8.40", "7.14"), ("-1.26", "1.26")).is_empty());

        assert!(refused(2, ("-60.00", "60.00"), ("-0.90", "0.90")).is_empty());
        assert_eq!(refused(2, ("-60.90", "60.90"), ("-0.90", "0.90")).len(), 1);

        assert!(refused(3, ("-25.00", "25.00"), ("-0.30", "0.30")).is_empty());
        assert!(refused(3, ("-24.70", "25.00"), ("-0.30", "0.30")).is_empty());

        assert!(refused(4, ("7.14", "7.14"), ("-1.26", "1.26")).is_empty());
        assert!(refused(4, ("8.40", "7.14"), ("-1.26", "1.26")).is_empty());

        let mut with_balance = line(5, ("7.14", "7.14"), ("-1.26", "1.26"));
        with_balance["balances"] =
            json!([{"asset_ref": "USD", "quantity": "107.14", "amount_text": "107.14"}]);
        assert!(check(&source, &output(json!({ "lines": [with_balance] }))).is_empty());
    }

    const ACROSS_NEW_YEAR: &str = "Statement period 01/12/2023 to 31/01/2024\n\
         Date,Description,Amount\n\
         28/12,Shop,-3.50\n\
         02/01,Cafe,-4.00\n";

    fn across_new_year(december: &str, january: &str, period: bool) -> ExtractionOutput {
        let mut value = json!({"lines": [
            row(2, december, "28/12", ("GBP", "-3.50", "-3.50")),
            row(3, january, "02/01", ("GBP", "-4.00", "-4.00")),
        ]});
        if period {
            value["period"] = json!({"refs": [0], "from": "2023-12-01", "from_text": "01/12/2023",
                                     "to": "2024-01-31", "to_text": "31/01/2024"});
        }
        output(value)
    }

    #[test]
    fn a_date_without_a_year_takes_the_year_the_period_gives_it() {
        let source = text::read(ACROSS_NEW_YEAR);

        assert!(check(&source, &across_new_year("2023-12-28", "2024-01-02", true)).is_empty());
        assert_eq!(
            messages(&check(&source, &across_new_year("2023-12-28", "2023-01-02", true))),
            vec!["date_text \"02/01\" does not print the whole year; the document's dates run from 2023-12-01 to 2024-01-31, so it is 2024-01-02, not 2023-01-02"]
        );
        assert_eq!(
            check(&source, &across_new_year("2024-12-28", "2019-01-02", true)).len(),
            2
        );
    }

    #[test]
    fn a_date_without_a_year_takes_it_from_the_fully_dated_dates_when_no_period_is_stated() {
        let source = text::read(
            "Date,Description,Amount\n\
             28/12,Shop,-3.50\n\
             02/01,Cafe,-4.00\n\
             Closing balance on 31 January 2024,92.50\n",
        );
        let lines = |december: &str| {
            output(json!({
                "lines": [
                    row(1, december, "28/12", ("GBP", "-3.50", "-3.50")),
                    row(2, "2024-01-02", "02/01", ("GBP", "-4.00", "-4.00")),
                ],
                "balances": [{"refs": [3], "kind": "closing", "account_ref": "Current", "asset_ref": "GBP",
                    "as_of_date": "2024-01-31", "date_text": "31 January 2024", "quantity": "92.50", "amount_text": "92.50"}]
            }))
        };

        assert!(check(&source, &lines("2023-12-28")).is_empty());
        assert_eq!(check(&source, &lines("2024-12-28")).len(), 1);
    }

    #[test]
    fn a_date_without_a_year_is_not_after_the_closing_balance() {
        let source = text::read(
            "Date,Description,Amount\n\
             02/01,Shop,-3.50\n\
             30/12,Cafe,-4.00\n\
             Closing balance on 31 December 2024,92.50\n",
        );
        let lines = |january: &str| {
            output(json!({
                "lines": [
                    row(1, january, "02/01", ("GBP", "-3.50", "-3.50")),
                    row(2, "2024-12-30", "30/12", ("GBP", "-4.00", "-4.00")),
                ],
                "balances": [{"refs": [3], "kind": "closing", "account_ref": "Current", "asset_ref": "GBP",
                    "as_of_date": "2024-12-31", "date_text": "31 December 2024", "quantity": "92.50", "amount_text": "92.50"}]
            }))
        };

        assert!(check(&source, &lines("2024-01-02")).is_empty());
        assert_eq!(check(&source, &lines("2025-01-02")).len(), 1);
    }

    #[test]
    fn rows_that_run_past_a_year_end_keep_the_years_that_keep_them_in_order() {
        let source = text::read(
            "Date,Description,Amount\n\
             05/03,Insurance,-100.00\n\
             05/09,Insurance,-100.00\n\
             05/03,Insurance,-110.00\n\
             05/09,Insurance,-110.00\n\
             Balance at 31 December 2024,580.00\n",
        );
        let lines = |years: [&str; 4]| {
            let rows: Vec<Value> = years
                .iter()
                .zip(["03", "09", "03", "09"])
                .enumerate()
                .map(|(index, (year, month))| {
                    row(
                        index + 1,
                        &format!("{year}-{month}-05"),
                        &format!("05/{month}"),
                        ("EUR", "-100.00", "-100.00"),
                    )
                })
                .collect();
            output(json!({
                "lines": rows,
                "balances": [{"refs": [5], "kind": "closing", "account_ref": "Current", "asset_ref": "EUR",
                    "as_of_date": "2024-12-31", "date_text": "31 December 2024", "quantity": "580.00", "amount_text": "580.00"}]
            }))
        };
        let misread = |years| {
            check(&source, &lines(years))
                .iter()
                .filter(|finding| matches!(finding, Finding::InvalidItem { .. }))
                .count()
        };

        assert_eq!(misread(["2023", "2023", "2024", "2024"]), 0);
        assert_eq!(misread(["2024", "2024", "2024", "2024"]), 0);
        assert_eq!(misread(["2013", "2013", "2014", "2014"]), 4);
        assert_eq!(misread(["2022", "2023", "2024", "2024"]), 2);
        assert_eq!(misread(["2023", "2024", "2024", "2024"]), 1);
    }

    #[test]
    fn dates_without_a_year_are_refused_when_nothing_cited_prints_one() {
        let source = text::read(ACROSS_NEW_YEAR);

        assert_eq!(
            check(&source, &across_new_year("2023-12-28", "2024-01-02", false)),
            vec![Finding::InvalidItem {
                item: ItemRef::Line(0),
                message: "date_text \"28/12\" prints no year and nothing you cite prints one, so the year cannot be checked; state the period, or a balance, from a record of the document that prints a full date".to_string(),
            }]
        );
    }

    #[test]
    fn a_two_digit_year_is_read_inside_the_documents_dates() {
        let source = text::read(
            "Statement date: 30 November 2024\n\
             Date,Description,Amount\n\
             23/11/24,Shop,-3.50\n",
        );
        let lines = |date: &str, dated: bool| {
            let mut value = json!({"lines": [row(2, date, "23/11/24", ("EUR", "-3.50", "-3.50"))]});
            if dated {
                value["balances"] = json!([{"refs": [2], "kind": "snapshot", "account_ref": "Current",
                    "asset_ref": "EUR", "as_of_date": "2024-11-30", "date_text": "30 November 2024",
                    "quantity": "-3.50", "amount_text": "-3.50"}]);
            }
            output(value)
        };

        assert!(check(&source, &lines("2024-11-23", true)).is_empty());
        assert_eq!(
            messages(&check(&source, &lines("2023-11-24", true))),
            vec!["date_text \"23/11/24\" does not print the whole year; the document's dates run from 2024-11-30 to 2024-11-30, so it is 2024-11-23, not 2023-11-24"]
        );
        assert!(check(&source, &lines("2023-11-24", false)).is_empty());
    }

    #[test]
    fn one_date_that_reads_only_one_way_sets_the_day_month_order_of_the_file() {
        let source = text::read(
            "Date,Description,Amount\n\
             03/04/2024,A,-1.00\n\
             05/06/2024,B,-2.00\n\
             25/06/2024,C,-3.00\n",
        );
        let lines = |second: &str| {
            output(json!({"lines": [
                row(1, "2024-04-03", "03/04/2024", ("EUR", "-1.00", "-1.00")),
                row(2, second, "05/06/2024", ("EUR", "-2.00", "-2.00")),
                row(3, "2024-06-25", "25/06/2024", ("EUR", "-3.00", "-3.00")),
            ]}))
        };

        assert!(check(&source, &lines("2024-06-05")).is_empty());
        assert_eq!(
            messages(&check(&source, &lines("2024-05-06"))),
            vec!["date_text \"05/06/2024\" is read month first, but dates in this document that read only one way are day first; in that order it is 2024-06-05, not 2024-05-06"]
        );
    }

    #[test]
    fn values_that_are_not_in_the_cited_record_fail() {
        let mut wrong_id = line(vec![1], "2024-01-02", "-11.00", dec!(-11));
        wrong_id.external_id = Some("TX-100".to_string());
        wrong_id.time = Some(time!(09:16));
        let output = ExtractionOutput {
            lines: vec![
                line(vec![1], "2024-01-02", "1.00", dec!(-1)),
                line(vec![2], "2024-01-02", "346.75", dec!(346.75)),
                wrong_id,
            ],
            ..Default::default()
        };

        assert_eq!(
            check(&source(), &output),
            vec![
                Finding::AmountNotFound {
                    item: ItemRef::Line(0),
                    amount_text: "1.00".to_string(),
                    quantity: dec!(-1),
                },
                Finding::AmountNotFound {
                    item: ItemRef::Line(1),
                    amount_text: "346.75".to_string(),
                    quantity: dec!(346.75),
                },
                Finding::DateNotFound {
                    item: ItemRef::Line(1),
                    date_text: "2024-01-02".to_string(),
                    date: date!(2024 - 01 - 02),
                },
                Finding::TimeNotFound {
                    item: ItemRef::Line(2),
                    time: "09:16".to_string(),
                },
                Finding::ExternalIdNotFound {
                    item: ItemRef::Line(2),
                    external_id: "TX-100".to_string(),
                },
            ]
        );
    }

    #[test]
    fn packed_machine_formats_are_readable() {
        let source = text::read(":20:STMT\n:61:1701190119CN0,01NTRFNONREF//MB170119\n");
        let mut line = line(vec![1], "170119", "0,01", dec!(0.01));
        line.date = date!(2017 - 01 - 19);
        let output = ExtractionOutput {
            lines: vec![line],
            ..Default::default()
        };

        assert!(check(&source, &output).is_empty());
    }

    #[test]
    fn an_entry_date_without_its_year_takes_it_from_the_value_date_before_it() {
        let source = text::read(
            ":20:STMT\n\
             :25:DE00123\n\
             :61:2612310102D7,99NMSCNONREF\n\
             :86:Card payment\n\
             :61:2701020102C5,00NTRFNONREF\n",
        );
        let entry = |record: usize, date: Date, amount: &str| {
            let mut line = line(
                vec![record],
                "0102",
                amount,
                amount.replace(',', ".").parse().unwrap(),
            );
            line.date = date;
            line
        };
        let lines = |first: Date, second: Date| ExtractionOutput {
            lines: vec![entry(2, first, "7,99"), entry(3, second, "5,00")],
            ..Default::default()
        };

        assert!(check(
            &source,
            &lines(date!(2027 - 01 - 02), date!(2027 - 01 - 02))
        )
        .is_empty());
        assert_eq!(
            check(
                &source,
                &lines(date!(2026 - 01 - 02), date!(2028 - 01 - 02))
            ),
            vec![
                Finding::DateNotFound {
                    item: ItemRef::Line(0),
                    date_text: "0102".to_string(),
                    date: date!(2026 - 01 - 02),
                },
                Finding::DateNotFound {
                    item: ItemRef::Line(1),
                    date_text: "0102".to_string(),
                    date: date!(2028 - 01 - 02),
                },
            ]
        );

        let unsplit = ImportSource {
            kind: SourceKind::Text,
            records: vec![SourceRecord {
                location: SourceLocation::Lines { first: 1, last: 1 },
                text: "2612310102D7,99 Card payment 0102".to_string(),
                fields: None,
            }],
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        };
        let cited = |date: Date| ExtractionOutput {
            lines: vec![entry(0, date, "7,99")],
            ..Default::default()
        };
        assert!(check(&unsplit, &cited(date!(2027 - 01 - 02))).is_empty());
        assert_eq!(check(&unsplit, &cited(date!(2026 - 01 - 02))).len(), 1);

        let follows = |full: &str, short: &str, day: Date| {
            NormalizedRecord {
                text: format!("{full}{short}"),
                fields: Vec::new(),
            }
            .dated_after_full_date(short, day)
        };
        for (full, short, day) in [
            ("260902", "0903", date!(2026 - 09 - 03)),
            ("260902", "0902", date!(2026 - 09 - 02)),
            ("270101", "1231", date!(2026 - 12 - 31)),
            ("20260902", "0903", date!(2026 - 09 - 03)),
            ("02092026", "0309", date!(2026 - 09 - 03)),
            ("090226", "0903", date!(2026 - 09 - 03)),
        ] {
            assert!(follows(full, short, day), "{short} after {full} is {day}");
        }
        for (full, short, day) in [
            ("260902", "0903", date!(2025 - 09 - 03)),
            ("260902", "0903", date!(2026 - 03 - 09)),
            ("260902", "0904", date!(2026 - 09 - 03)),
            ("260902", "903", date!(2026 - 09 - 03)),
            ("2609", "0903", date!(2026 - 09 - 03)),
            ("26090A", "0903", date!(2026 - 09 - 03)),
        ] {
            assert!(
                !follows(full, short, day),
                "{short} after {full} is not {day}"
            );
        }
    }

    #[test]
    fn an_opening_may_be_dated_the_day_after_the_day_its_record_prints() {
        let source = text::read(
            "Date;Text;Amount\n\
             Old balance per 31.08.2026;;2.104,18\n\
             New balance per 30.09.2026;;2.000,00\n",
        );
        let balance = |kind: &str,
                       date: &str,
                       date_text: &str,
                       record: usize,
                       amount: (&str, &str)| {
            json!({"refs": [record], "kind": kind, "account_ref": "Current", "asset_ref": "EUR",
                   "as_of_date": date, "date_text": date_text, "quantity": amount.0, "amount_text": amount.1})
        };
        let stated = |opening: (&str, &str), closing: (&str, &str)| {
            output(json!({"balances": [
                balance(opening.0, opening.1, "31.08.2026", 1, ("2104.18", "2.104,18")),
                balance(closing.0, closing.1, "30.09.2026", 2, ("2000.00", "2.000,00")),
            ]}))
        };

        for opening in ["2026-08-31", "2026-09-01"] {
            assert!(
                check(
                    &source,
                    &stated(("opening", opening), ("closing", "2026-09-30"))
                )
                .is_empty(),
                "{opening}"
            );
        }
        assert_eq!(
            check(
                &source,
                &stated(("opening", "2026-09-02"), ("closing", "2026-10-01"))
            )
            .len(),
            2
        );
        assert_eq!(
            check(
                &source,
                &stated(("snapshot", "2026-09-01"), ("closing", "2026-09-30"))
            )
            .len(),
            1
        );
    }

    #[test]
    fn values_in_other_scripts_are_read() {
        let source = text::read("٢٠٢٤-٠١-٠٢ قهوة ١٢٫٥٠\n");
        let output = ExtractionOutput {
            lines: vec![line(vec![0], "٢٠٢٤-٠١-٠٢", "١٢٫٥٠", dec!(-12.5))],
            ..Default::default()
        };

        assert!(check(&source, &output).is_empty());
    }

    #[test]
    fn a_stated_balance_may_take_its_date_from_elsewhere_in_the_document() {
        let source = ImportSource {
            kind: SourceKind::Pdf,
            records: [
                "Statement period 1 December 2023 to 31 December 2023",
                "Opening balance                      £1,234.56",
            ]
            .into_iter()
            .enumerate()
            .map(|(line, text)| SourceRecord {
                location: SourceLocation::Page {
                    page: 1,
                    line: line as u32 + 1,
                },
                text: text.to_string(),
                fields: None,
            })
            .collect(),
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        };
        let output = ExtractionOutput {
            balances: vec![StatedBalance {
                refs: vec![1],
                kind: BalanceKind::Opening,
                account_ref: "Current".to_string(),
                asset_ref: "GBP".to_string(),
                as_of_date: date!(2023 - 12 - 01),
                date_text: "1 December 2023".to_string(),
                quantity: dec!(1234.56),
                amount_text: "£1,234.56".to_string(),
            }],
            ..Default::default()
        };

        assert!(check(&source, &output).is_empty());
    }
}
