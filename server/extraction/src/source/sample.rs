use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range;

use serde_json::{json, Value};

use crate::gates::amounts::{contains_amount_shape, numeric_value};
use crate::gates::dates::contains_date_shape;
use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

const HEAD_RECORDS: usize = 30;
const TAIL_RECORDS: usize = 15;
const MIDDLE_WINDOWS: usize = 3;
const WINDOW_RECORDS: usize = 10;
const MAX_KINDS: usize = 40;

struct RecordKind {
    count: usize,
    first: usize,
    last: usize,
    shown: bool,
    unseen_example: Option<usize>,
}

impl ImportSource {
    pub fn script_records(&self) -> Value {
        self.records
            .iter()
            .map(|record| {
                let mut value = json!({ "text": record.text });
                if let Some(fields) = &record.fields {
                    value["fields"] = json!(fields);
                }
                match &record.location {
                    SourceLocation::Lines { .. } => {}
                    SourceLocation::Row { sheet, row } => {
                        value["sheet"] = json!(sheet);
                        value["row"] = json!(row);
                    }
                    SourceLocation::Page { page, .. } => {
                        value["page"] = json!(page);
                        if let Some(fields) = &record.fields {
                            value["columns"] = json!(field_columns(&record.text, fields));
                        }
                    }
                }
                value
            })
            .collect()
    }

    pub fn sample(&self) -> String {
        let total = self.records.len();
        let mut out = format!(
            "{} source, {total} records (refs 0–{})\n",
            self.kind.label(),
            total.saturating_sub(1)
        );
        for note in &self.notes {
            let _ = writeln!(out, "note: {note}");
        }

        let ranges = sample_ranges(total);
        for (title, range) in &ranges {
            let _ = writeln!(out, "-- {title} --");
            for index in range.clone() {
                let _ = writeln!(out, "{}", render(index, &self.records[index]));
            }
        }

        let kinds = self.unseen_kinds(&ranges);
        if !kinds.is_empty() {
            let _ = writeln!(
                out,
                "-- other kinds of record: one example of each row shape not shown above --"
            );
            for kind in kinds.iter().take(MAX_KINDS) {
                if let Some(example) = kind.unseen_example {
                    let _ = writeln!(
                        out,
                        "{} (×{}, refs {}–{})",
                        render(example, &self.records[example]),
                        kind.count,
                        kind.first,
                        kind.last
                    );
                }
            }
            if kinds.len() > MAX_KINDS {
                let _ = writeln!(out, "… {} more kinds", kinds.len() - MAX_KINDS);
            }
        }
        out
    }

    fn unseen_kinds(&self, ranges: &[(String, Range<usize>)]) -> Vec<RecordKind> {
        let shown = |index: usize| ranges.iter().any(|(_, range)| range.contains(&index));
        let mut order: Vec<String> = Vec::new();
        let mut kinds: HashMap<String, RecordKind> = HashMap::new();
        for (index, record) in self.records.iter().enumerate() {
            let signature = kind_signature(self.kind, record);
            let kind = kinds.entry(signature.clone()).or_insert_with(|| {
                order.push(signature);
                RecordKind {
                    count: 0,
                    first: index,
                    last: index,
                    shown: false,
                    unseen_example: None,
                }
            });
            kind.count += 1;
            kind.last = index;
            if shown(index) {
                kind.shown = true;
            } else if kind.unseen_example.is_none() {
                kind.unseen_example = Some(index);
            }
        }
        order
            .into_iter()
            .filter_map(|signature| kinds.remove(&signature))
            .filter(|kind| !kind.shown)
            .collect()
    }
}

fn kind_signature(kind: SourceKind, record: &SourceRecord) -> String {
    let leading_tag = || {
        let mut named = false;
        let mut closed = false;
        let tag: String = record
            .text
            .trim_start()
            .chars()
            .take_while(|c| {
                let within = !closed && !c.is_ascii_digit() && !c.is_whitespace();
                closed = named && !c.is_alphanumeric();
                named |= c.is_alphanumeric();
                within
            })
            .collect();
        tag.chars().any(|c| !c.is_alphanumeric()).then_some(tag)
    };
    match record.fields.as_deref() {
        Some(fields) if kind == SourceKind::Text => {
            leading_tag().unwrap_or_else(|| fields.iter().map(|field| field_class(field)).collect())
        }
        Some([single]) => first_word_signature(single),
        Some(fields) => fields.iter().map(|field| field_class(field)).collect(),
        None => leading_tag().unwrap_or_else(|| text_shape(&record.text)),
    }
}

fn first_word_signature(text: &str) -> String {
    let first_word = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let class = field_class(text);
    if first_word.chars().any(char::is_alphabetic) {
        format!("{class}:{first_word}")
    } else {
        format!("{class}:{}", text_shape(text))
    }
}

fn field_class(field: &str) -> char {
    let field = field.trim();
    if field.is_empty() {
        '_'
    } else if contains_date_shape(field) {
        'D'
    } else if numeric_value(field).is_some() {
        if contains_amount_shape(field) {
            'A'
        } else {
            'N'
        }
    } else {
        'T'
    }
}

fn text_shape(text: &str) -> String {
    let mut shape = String::new();
    for c in text.chars() {
        let class = if c.is_ascii_digit() {
            '#'
        } else if c.is_alphabetic() {
            'a'
        } else if c.is_whitespace() {
            ' '
        } else {
            c
        };
        let repeat = shape.ends_with(class) && matches!(class, '#' | 'a' | ' ');
        let word_run = class == 'a' && shape.ends_with("a ");
        if word_run {
            shape.pop();
        } else if !repeat {
            shape.push(class);
        }
    }
    shape.trim().to_string()
}

fn sample_ranges(total: usize) -> Vec<(String, Range<usize>)> {
    if total <= HEAD_RECORDS + TAIL_RECORDS + MIDDLE_WINDOWS * WINDOW_RECORDS {
        return vec![("all records".to_string(), 0..total)];
    }

    let mut ranges = vec![("head".to_string(), 0..HEAD_RECORDS)];
    let middle_start = HEAD_RECORDS;
    let middle_end = total - TAIL_RECORDS;
    let step = (middle_end - middle_start) / (MIDDLE_WINDOWS + 1);
    for window in 1..=MIDDLE_WINDOWS {
        let start = middle_start + step * window - WINDOW_RECORDS / 2;
        ranges.push((
            format!("refs {start}–{}", start + WINDOW_RECORDS - 1),
            start..start + WINDOW_RECORDS,
        ));
    }
    ranges.push(("tail".to_string(), middle_end..total));
    ranges
}

fn render(index: usize, record: &SourceRecord) -> String {
    let location = match &record.location {
        SourceLocation::Lines { first, last } if first == last => String::new(),
        SourceLocation::Lines { first, last } => format!(" (lines {first}–{last})"),
        SourceLocation::Row { sheet, row } => format!(" ({sheet}!{row})"),
        SourceLocation::Page { page, .. } => format!(" (p{page})"),
    };
    let content = match (&record.location, &record.fields) {
        (SourceLocation::Page { .. }, Some(fields)) => positioned_fields(&record.text, fields),
        _ => record.text.replace('\n', "⏎"),
    };
    format!("[{index}]{location} {content}")
}

fn positioned_fields(text: &str, fields: &[String]) -> String {
    fields
        .iter()
        .zip(field_columns(text, fields))
        .map(|(field, column)| format!("@{column} {field}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn field_columns(text: &str, fields: &[String]) -> Vec<usize> {
    let mut cursor = 0;
    fields
        .iter()
        .map(|field| {
            let start = text[cursor..]
                .find(field.as_str())
                .map_or(cursor, |at| cursor + at);
            cursor = start + field.len();
            text[..start].chars().count()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn rare_kinds_of_record_outside_the_windows_are_shown_once() {
        let mut text = String::from("Date,Description,Amount\n");
        for day in 0..200 {
            text.push_str(&format!(
                "2024-01-{:02},Shop {day},-{day}.50\n",
                day % 28 + 1
            ));
        }
        text.push_str("Total,,-19950.00\n");
        text.push_str("2024-02-01,Monthly fee,-1.00,EXTRA\n");
        for day in 0..30 {
            text.push_str(&format!(
                "2024-03-{:02},Shop {day},-{day}.50\n",
                day % 28 + 1
            ));
        }
        let sample = text::read(&text).sample();

        assert!(sample.contains("-- other kinds of record"), "{sample}");
        assert!(
            sample.contains("Total,,-19950.00 (×1, refs 201–201)"),
            "{sample}"
        );
        assert!(
            sample.contains("Monthly fee,-1.00,EXTRA (×1, refs 202–202)"),
            "{sample}"
        );
        assert_eq!(sample.matches("(×").count(), 2, "{sample}");
    }

    #[test]
    fn a_structured_record_is_shown_whole_and_kinds_go_by_its_tag() {
        let mut text = String::from("<Stmt>\n<Id>A</Id>\n");
        for entry in 0..300 {
            let memo = if entry % 7 == 0 {
                "<Memo>card</Memo>\n"
            } else {
                ""
            };
            text.push_str(&format!(
                "<Ntry>\n<Amt>{entry}.50</Amt>\n<Dt>2024-01-{:02}</Dt>\n{memo}</Ntry>\n",
                entry % 28 + 1
            ));
            if entry == 60 || entry == 200 {
                text.push_str("<Fee>\n<Amt>1.00</Amt>\n<Dt>2024-01-31</Dt>\n</Fee>\n");
            }
        }
        text.push_str("</Stmt>\n");
        let sample = text::read(&text).sample();

        assert!(
            sample
                .contains("[2] (lines 8–11) <Ntry>⏎<Amt>1.50</Amt>⏎<Dt>2024-01-02</Dt>⏎</Ntry>\n"),
            "{sample}"
        );
        assert!(
            sample.contains("<Fee>⏎<Amt>1.00</Amt>⏎<Dt>2024-01-31</Dt>⏎</Fee> (×2, refs 62–203)"),
            "{sample}"
        );
        assert_eq!(sample.matches("(×").count(), 1, "{sample}");
    }

    #[test]
    fn layout_lines_are_shown_as_positioned_fields() {
        let record = SourceRecord {
            location: SourceLocation::Page { page: 2, line: 7 },
            text: "     14 Dec 2023    Corner Bakery Co          £47.18          £3,418.27"
                .to_string(),
            fields: Some(vec![
                "14 Dec 2023".to_string(),
                "Corner Bakery Co".to_string(),
                "£47.18".to_string(),
                "£3,418.27".to_string(),
            ]),
        };

        assert_eq!(
            render(12, &record),
            "[12] (p2) @5 14 Dec 2023 | @20 Corner Bakery Co | @46 £47.18 | @62 £3,418.27"
        );
    }

    use super::super::text;
    use super::*;

    #[test]
    fn a_long_source_shows_head_three_middle_windows_and_tail() {
        let source = ImportSource {
            kind: SourceKind::Text,
            records: (0..1000)
                .map(|i| SourceRecord {
                    location: SourceLocation::Lines {
                        first: i + 1,
                        last: i + 1,
                    },
                    text: format!("row {i}"),
                    fields: None,
                })
                .collect(),
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        };
        let sample = source.sample();

        assert!(sample.contains("-- head --\n[0] row 0\n"));
        assert!(sample.contains("[29] row 29\n-- refs 263–272 --"));
        assert!(sample.contains("-- refs 501–510 --"));
        assert!(sample.contains("-- refs 739–748 --"));
        assert!(sample.contains("-- tail --\n[985] row 985\n"));
        assert!(sample.ends_with("[999] row 999\n"));
        assert_eq!(sample.lines().filter(|l| l.starts_with('[')).count(), 75);
    }
}
