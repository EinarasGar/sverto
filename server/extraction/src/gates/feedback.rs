use serde_json::{json, Map, Value};

use super::amounts::numeric_value;
use super::dates::contains_date_shape;
use crate::models::finding::{ExtractionReport, Finding, ItemRef};
use crate::models::output::ExtractionOutput;
use crate::models::source::{ImportSource, SourceRecord};

const MAX_PROBLEMS: usize = 8;
const MAX_REFS: usize = 20;
const MAX_EXCERPTS: usize = 3;
const EXCERPT_CHARS: usize = 160;
const MAX_EXCERPT_FIELDS: usize = 16;
const MAX_EXCERPT_FIELD_CHARS: usize = 40;

impl ExtractionReport {
    pub fn feedback(&self, source: &ImportSource, output: &ExtractionOutput) -> Value {
        let blocking: Vec<&Finding> = self.findings.iter().filter(|f| f.is_blocking()).collect();
        let mut hinted = Vec::new();
        let problems: Vec<Value> = blocking
            .iter()
            .take(MAX_PROBLEMS)
            .map(|finding| {
                let mut value = problem(finding, source, output);
                let kind = value["problem"].clone();
                if hinted.contains(&kind) {
                    value.as_object_mut().map(|object| object.remove("hint"));
                } else {
                    hinted.push(kind);
                }
                value
            })
            .collect();
        json!({
            "accepted": false,
            "problems": problems,
            "more_problems": blocking.len().saturating_sub(MAX_PROBLEMS),
        })
    }
}

fn problem(finding: &Finding, source: &ImportSource, output: &ExtractionOutput) -> Value {
    let mut value = serde_json::to_value(finding).unwrap_or_default();
    let records = match finding {
        Finding::UncoveredRecords { refs }
        | Finding::SuspectSkipped { refs }
        | Finding::NothingExtracted { refs }
        | Finding::BalancesSkipped { refs, .. }
        | Finding::BalanceColumnIgnored { refs, .. } => {
            if refs.len() > MAX_REFS {
                value["refs"] = json!(refs[..MAX_REFS]);
                value["more_refs"] = json!(refs.len() - MAX_REFS);
            }
            refs.clone()
        }
        Finding::RecordClaimedTwice { record, .. } => vec![*record],
        Finding::InvalidItem { item, .. }
        | Finding::AmountNotFound { item, .. }
        | Finding::DateNotFound { item, .. }
        | Finding::TimeNotFound { item, .. }
        | Finding::ExternalIdNotFound { item, .. }
        | Finding::ControlTotalMismatch { item, .. } => item_refs(*item, output),
        Finding::BalanceChainBroken { line, .. } => item_refs(ItemRef::Line(*line), output),
        Finding::BalancesDisagree { earlier, later, .. } => [earlier.balance, later.balance]
            .into_iter()
            .flatten()
            .flat_map(|index| item_refs(ItemRef::Balance(index), output))
            .collect(),
        Finding::InvalidOutput { refs, .. } => refs.clone(),
    };
    value["hint"] = json!(hint(finding));
    value["records"] = records
        .iter()
        .filter(|r| **r < source.records.len())
        .take(MAX_EXCERPTS)
        .map(|r| excerpt(*r, &source.records[*r]))
        .collect();
    value
}

fn item_refs(item: ItemRef, output: &ExtractionOutput) -> Vec<usize> {
    let refs = match item {
        ItemRef::Line(index) => output.lines.get(index).map(|l| {
            l.refs
                .iter()
                .chain(&l.date_refs)
                .copied()
                .collect::<Vec<_>>()
        }),
        ItemRef::Balance(index) => output.balances.get(index).map(|b| b.refs.clone()),
        ItemRef::ControlTotal(index) => output.control_totals.get(index).map(|t| t.refs.clone()),
        ItemRef::Skipped(index) => output.skipped.get(index).map(|s| s.refs.clone()),
        ItemRef::Period => output.period.as_ref().map(|p| p.refs.clone()),
    };
    refs.unwrap_or_default()
}

fn excerpt(index: usize, record: &SourceRecord) -> Value {
    let text = record.text.trim();
    if text.chars().count() <= EXCERPT_CHARS {
        return json!({ "ref": index, "text": text });
    }
    let values: Map<String, Value> = record
        .fields
        .iter()
        .flatten()
        .enumerate()
        .filter(|(_, field)| {
            field.chars().count() <= MAX_EXCERPT_FIELD_CHARS
                && (contains_date_shape(field) || numeric_value(field).is_some())
        })
        .take(MAX_EXCERPT_FIELDS)
        .map(|(position, field)| (position.to_string(), json!(field)))
        .collect();
    let mut excerpt = json!({
        "ref": index,
        "text": format!("{}…", text.chars().take(EXCERPT_CHARS).collect::<String>()),
    });
    if !values.is_empty() {
        excerpt["fields"] = Value::Object(values);
    }
    excerpt
}

fn hint(finding: &Finding) -> &'static str {
    match finding {
        Finding::InvalidOutput { .. } | Finding::InvalidItem { .. } => {
            "Fix the output as the message says."
        }
        Finding::UncoveredRecords { .. } => {
            "Every record must belong to a line, a balance, a control total, the period, or `skipped`."
        }
        Finding::RecordClaimedTwice { .. } => {
            "A record that belongs to a line belongs to that line only, unless the lines sharing it are one transaction (same transaction_key) or the other item is the period. A running balance goes in the line's `balances`, not in `balances` at the top level."
        }
        Finding::SuspectSkipped { .. } => {
            "These skipped records carry a date and an amount. Make them lines, balances or control totals; if they never moved money, skip them as \"excluded\" with a reason."
        }
        Finding::NothingExtracted { .. } => {
            "The output has no lines and no balances, yet these records carry a date and an amount. Extract them; where a gate refused them, fix what its feedback named instead of leaving them out. Only if not one of them changed what an account holds (every entry pending, declined, reverted or failed) and the document states no balance, return this same output again: the file is then reported as holding nothing to import."
        }
        Finding::BalancesSkipped { .. } => {
            "The account states no balance in this asset and its lines carry none, yet these skipped records each print one amount, and two of them differ by exactly lines_net, what the lines move the account by: they are the statement's own opening and closing balance. State them in balances as what their records say they are. A balance needs no date on its own record: an opening is dated the first day covered and a closing the last, with date_text cited from the period or wherever else the document prints that day. On a statement of money owed the balance owed is negative. A record that is no balance is excluded with what it is."
        }
        Finding::AmountNotFound { .. } => {
            "amount_text must be copied exactly from the line's own records, and the quantity must be that number: give toDecimal the decimal separator the document uses. Whole numbers of minor units (350 for 3.50) are read with fromMinorUnits, every amount and balance of that currency alike. Never compute an amount. The one exception is a printed amount that already includes the line's fee or tax (not one the row also prints with the fee added): the line's only main leg in that asset cites it and takes minus(amount, fee), with the fee leg's own quantity read as printed."
        }
        Finding::DateNotFound { .. } => {
            "date_text must be copied exactly from the cited records and must read as the date: the day and time as printed, never shifted by a UTC offset. When a row's date is printed elsewhere (once per day, in a heading), cite that record in date_refs."
        }
        Finding::TimeNotFound { .. } => {
            "The time must appear in date_text or in the cited records, on the 24-hour clock (2:30 PM is 14:30) and with its seconds when they are printed; leave it out when the document gives none."
        }
        Finding::ExternalIdNotFound { .. } => {
            "external_id must be copied exactly from the line's own records; leave it out when the document gives none."
        }
        Finding::BalanceChainBroken { .. } => {
            "Each printed balance must equal the previous balance plus this line's amount. Every order was tried (the document's, its reverse, and the lines' dates, re-ordering rows within a day) and so was a balance printed before each row, so re-sorting the lines changes nothing. Check this line's sign and amount, whether a row before it is missing or wrongly excluded (a row whose balance moves is a line even when its amount is printed only inside its text: cite it from there), and, where rows print two dates, that `date` is the one the balance follows (posting or completion). On a statement of money owed the printed balance is a negative balance: negate the balance, not the lines."
        }
        Finding::BalancesDisagree { .. } => {
            "The lines between these two balances do not add up to their difference, and the opening plus every line of the account does not give the closing either; a snapshot was also tried where its record is printed, between the rows around it. A row may be missing or misread, a balance misdated (an opening is dated the first day it covers, a closing the last), or a running balance given as a separate balance."
        }
        Finding::BalanceColumnIgnored { .. } => {
            "These lines print a running balance they do not carry. `field` is its position in the first record listed: in `fields`, or among the words of `text` when the record has none. Within one account it moves from line to line by exactly the line's amount, or it sits where the account's other lines read their balance. Put it in `balances` on every line that prints one."
        }
        Finding::ControlTotalMismatch { .. } => {
            "The lines do not add up to this stated total. Totals are computed over the account's lines in the total's asset, fees included; if the document totals on another basis, leave the total out."
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;
    use crate::source::text;

    #[test]
    fn feedback_names_the_problem_with_a_hint_and_the_records_involved() {
        let source = text::read("date,amount\n2024-01-02,-3.50\n2024-01-03,10.00\n");
        let output = ExtractionOutput::default();
        let report = ExtractionReport {
            findings: vec![
                Finding::UncoveredRecords {
                    refs: vec![0, 1, 2],
                },
                Finding::ControlTotalMismatch {
                    item: ItemRef::ControlTotal(0),
                    computed: dec!(1),
                    blocking: false,
                },
            ],
        };

        let feedback = report.feedback(&source, &output);

        assert_eq!(feedback["accepted"], json!(false));
        assert_eq!(feedback["more_problems"], json!(0));
        let problems = feedback["problems"].as_array().unwrap();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0]["problem"], json!("uncovered_records"));
        assert_eq!(problems[0]["refs"], json!([0, 1, 2]));
        assert_eq!(
            problems[0]["records"][1],
            json!({ "ref": 1, "text": "2024-01-02,-3.50" })
        );
        assert!(problems[0]["hint"]
            .as_str()
            .unwrap()
            .starts_with("Every record"));
    }
}
