use std::collections::HashMap;

use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

const DELIMITER_CANDIDATES: [u8; 4] = *b",;\t|";
const MIN_MODAL_FIELDS: usize = 2;
const MIN_CONFORMING_RECORDS: usize = 2;
const MIN_SPLIT_RATIO: f64 = 0.5;
const TABLE_RATIO: f64 = 0.6;
const NOTE_PREVIEW: usize = 10;
const DECIMAL_COMMA_RATIO: f64 = 0.8;

struct RawRecord {
    first_line: u32,
    last_line: u32,
    fields: Vec<String>,
}

struct Shape {
    delimiter: u8,
    records: Vec<RawRecord>,
    modal: usize,
    conforming: usize,
}

pub fn tokenize(text: &str) -> Option<ImportSource> {
    let mut shape = DELIMITER_CANDIDATES
        .into_iter()
        .filter_map(|delimiter| shape(text, delimiter))
        .max_by_key(|shape| (shape.conforming, shape.modal))?;

    let lines: Vec<&str> = text.split('\n').collect();
    let records: Vec<SourceRecord> = std::mem::take(&mut shape.records)
        .into_iter()
        .filter(|record| record.fields.iter().any(|f| !f.trim().is_empty()))
        .map(|record| SourceRecord {
            text: lines[record.first_line as usize - 1..record.last_line as usize].join("\n"),
            location: SourceLocation::Lines {
                first: record.first_line,
                last: record.last_line,
            },
            fields: Some(record.fields),
        })
        .collect();

    let notes = structure_notes(
        shape.delimiter as char,
        shape.modal,
        shape.conforming,
        &records,
    );
    Some(ImportSource {
        kind: SourceKind::Delimited,
        records,
        notes,
        pages_without_text: Vec::new(),
    })
}

fn shape(text: &str, delimiter: u8) -> Option<Shape> {
    let records = read_records(text, delimiter)?;
    let split = |r: &RawRecord| {
        r.fields.iter().filter(|f| !f.trim().is_empty()).count() >= MIN_MODAL_FIELDS
    };
    let first = records.iter().position(split)?;
    let last = records.iter().rposition(split)?;
    let split_count = records[first..=last].iter().filter(|r| split(r)).count();
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for record in records.iter().filter(|r| split(r)) {
        *counts.entry(record.fields.len()).or_default() += 1;
    }
    let (modal, conforming) = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)))?;
    if delimiter == b','
        && mostly_decimal_commas(text)
        && !records.iter().any(|r| {
            r.fields.len() == modal
                && r.fields.windows(2).all(|pair| {
                    !(pair[0].ends_with(|c: char| c.is_ascii_digit())
                        && pair[1].starts_with(|c: char| c.is_ascii_digit()))
                })
        })
    {
        return None;
    }
    let structured = split_count as f64 / (last - first + 1) as f64;
    (split_count >= MIN_CONFORMING_RECORDS && structured >= MIN_SPLIT_RATIO).then_some(Shape {
        delimiter,
        records,
        modal,
        conforming,
    })
}

fn mostly_decimal_commas(text: &str) -> bool {
    let bytes = text.as_bytes();
    let commas: Vec<usize> = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b',')
        .map(|(index, _)| index)
        .collect();
    let between_digits = commas
        .iter()
        .filter(|&&index| {
            index > 0
                && bytes[index - 1].is_ascii_digit()
                && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
        })
        .count();
    !commas.is_empty() && between_digits as f64 >= DECIMAL_COMMA_RATIO * commas.len() as f64
}

fn structure_notes(
    delimiter: char,
    modal: usize,
    conforming: usize,
    records: &[SourceRecord],
) -> Vec<String> {
    let mut notes = vec![if conforming as f64 >= TABLE_RATIO * records.len() as f64 {
        format!(
            "delimiter {delimiter:?}, {modal} fields in most records; read `fields`, never split `text` yourself"
        )
    } else {
        format!(
            "delimiter {delimiter:?} gives {modal} fields in only {conforming} of {} records; the rest may not be a table, so check `text` where `fields` look wrong",
            records.len()
        )
    }];
    let wrapped: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(
            |(_, r)| matches!(r.location, SourceLocation::Lines { first, last } if last > first),
        )
        .map(|(index, _)| index)
        .collect();
    if !wrapped.is_empty() {
        notes.push(format!(
            "{} record(s) span several physical lines (line breaks inside quoted fields): {}",
            wrapped.len(),
            preview(&wrapped)
        ));
    }
    let outliers: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.fields.as_ref().is_some_and(|f| f.len() != modal))
        .map(|(index, _)| index)
        .collect();
    if !outliers.is_empty() {
        notes.push(format!(
            "{} record(s) do not have {modal} fields: {}",
            outliers.len(),
            preview(&outliers)
        ));
    }
    notes
}

fn preview(refs: &[usize]) -> String {
    let shown: Vec<String> = refs
        .iter()
        .take(NOTE_PREVIEW)
        .map(usize::to_string)
        .collect();
    if refs.len() > NOTE_PREVIEW {
        format!("{} …", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

fn read_records(text: &str, delimiter: u8) -> Option<Vec<RawRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut starts: Vec<(usize, Vec<String>)> = Vec::new();
    for record in reader.records() {
        let record = record.ok()?;
        let position = record.position().map_or(0, |p| p.byte() as usize);
        let skipped_blank_lines =
            text[position..].len() - text[position..].trim_start_matches('\n').len();
        starts.push((
            position + skipped_blank_lines,
            record.iter().map(str::to_string).collect(),
        ));
    }

    let line_starts = line_start_offsets(text);
    let mut out = Vec::with_capacity(starts.len());
    for index in 0..starts.len() {
        let (byte, fields) = starts[index].clone();
        let end = starts.get(index + 1).map_or(text.len(), |(next, _)| *next);
        let last_byte = text[..end]
            .trim_end_matches('\n')
            .len()
            .saturating_sub(1)
            .max(byte);
        out.push(RawRecord {
            first_line: line_of(&line_starts, byte),
            last_line: line_of(&line_starts, last_byte),
            fields,
        });
    }
    Some(out)
}

pub fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    starts.extend(
        text.char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i + 1),
    );
    starts
}

pub fn line_of(line_starts: &[usize], byte: usize) -> u32 {
    match line_starts.binary_search(&byte) {
        Ok(index) => index as u32 + 1,
        Err(index) => index as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::super::text;
    use super::*;

    fn spans(source: &ImportSource) -> Vec<(u32, u32)> {
        source
            .records
            .iter()
            .map(|r| match r.location {
                SourceLocation::Lines { first, last } => (first, last),
                _ => panic!("delimited records are located by lines"),
            })
            .collect()
    }

    #[test]
    fn a_quoted_newline_stays_one_record() {
        let source = tokenize(
            "\"acct\",\"20\",\"2019-09-01\",\"SEASIDE\nPARKING\",\"11.00\",\"D\"\n\
             \"acct\",\"20\",\"2019-09-02\",\"SHOP\",\"3.50\",\"D\"\n",
        )
        .expect("delimited");

        assert_eq!(source.records.len(), 2);
        let fields = source.records[0].fields.as_ref().unwrap();
        assert_eq!(fields[3], "SEASIDE\nPARKING");
        assert_eq!(fields[4], "11.00");
        assert_eq!(spans(&source), vec![(1, 2), (3, 3)]);
        assert!(source.records[0]
            .text
            .ends_with("PARKING\",\"11.00\",\"D\""));
        assert!(source.notes[1].ends_with("(line breaks inside quoted fields): 0"));
    }

    #[test]
    fn a_character_inside_every_description_does_not_beat_the_real_delimiter() {
        for (text, delimiter, fields) in [
            (
                "Account: 1234, Current Account\nDate,Description,Amount,Balance\n\
                 2024-01-02,CARD | TESCO,-3.50,96.50\n2024-01-03,CARD | AMAZON,-4.00,92.50\n",
                "','",
                vec!["2024-01-02", "CARD | TESCO", "-3.50", "96.50"],
            ),
            (
                "Date,Description,Amount\n2024-01-02,POS; TESCO,-3.50\n2024-01-03,POS; AMAZON,-4.00\n",
                "','",
                vec!["2024-01-02", "POS; TESCO", "-3.50"],
            ),
            (
                "Konto 123, Girokonto\nDatum;Text;Betrag\n\
                 02.01.2024, 14:30;Kaffee;-3,50\n03.01.2024, 09:10;Miete;-500,00\n",
                "';'",
                vec!["02.01.2024, 14:30", "Kaffee", "-3,50"],
            ),
            (
                "Date\tPayee\tAmount\n2024-01-02\tAustin, TX\t-3.50\n2024-01-03\tDallas, TX\t-4.00\n",
                "'\\t'",
                vec!["2024-01-02", "Austin, TX", "-3.50"],
            ),
        ] {
            let source = tokenize(text).unwrap();
            let row = source.records.len() - 2;

            assert!(
                source.notes[0].starts_with(&format!("delimiter {delimiter}")),
                "{}",
                source.notes[0]
            );
            assert_eq!(source.records[row].fields.as_ref().unwrap(), &fields);
        }
    }

    #[test]
    fn a_comma_table_of_dates_and_amounts_is_delimited() {
        let mut text = String::from("Date,Credit,Balance\n");
        for day in 1..=20 {
            text.push_str(&format!(
                "2024-01-{day:02},100.00,{}.00\n",
                1000 + day * 100
            ));
        }
        let source = tokenize(&text).expect("delimited");

        assert!(source.notes[0].starts_with("delimiter ',', 3 fields"));
        assert_eq!(
            source.records[1].fields.as_ref().unwrap(),
            &vec!["2024-01-01", "100.00", "1100.00"]
        );
    }

    #[test]
    fn a_long_preamble_does_not_hide_the_table() {
        let source = tokenize(
            "Bank of Somewhere\nAccount holder: A. Person\nAccount: 12345678\nPeriod: January\n\
             Generated: 2024-02-01\nCurrency: EUR\n\
             Date,Description,Amount,Balance\n\
             2024-01-02,Coffee,-3.50,96.50\n\
             2024-01-03,Salary,1000.00,1096.50\n",
        )
        .expect("delimited");

        assert_eq!(source.records.len(), 9);
        assert_eq!(
            source.records[8].fields.as_ref().unwrap(),
            &vec!["2024-01-03", "Salary", "1000.00", "1096.50"]
        );
    }

    #[test]
    fn continuation_rows_and_broken_quoting_keep_the_table() {
        let continued = tokenize(
            "Account details for:,ACME 666-666\nAvailable Balance,\"6,666.66\"\n\nTransaction History\n\
             Transaction date,Value date,Description,Withdrawals (SGD),Deposits (SGD)\n\
             18/04/2018,18/04/2018,DEBIT PURCHASE,6.66,\n,,15/04/18 UBER EATS\n\
             17/04/2018,17/04/2018,CASH WITHDRAWAL,66.66,\n,,ATM 123\n",
        )
        .expect("delimited");
        assert!(continued.notes[0].starts_with("delimiter ','"));
        assert!(
            continued.notes[0].contains("in only"),
            "{}",
            continued.notes[0]
        );

        let broken_quotes = tokenize(
            "\"Acct\",\"Code\",\"Date\",\"Payee\",\"Amount\"\n\
             \"LT01\",\"20\",\"2012-01-26\",\"UAB \"Naktida\"\",\"12.20\"\n\
             \"LT01\",\"20\",\"2012-01-27\",\"Shop\",\"3.00\"\n",
        )
        .expect("delimited");
        assert_eq!(broken_quotes.records.len(), 3);
    }

    #[test]
    fn amounts_with_decimal_commas_in_plain_text_are_not_a_table() {
        assert!(tokenize(
            "Kontoauszug Januar\n02.01. Kaffee 3,50\n03.01. Miete 500,00\n04.01. Strom 61,20\n",
        )
        .is_none());
    }

    #[test]
    fn a_wrapped_record_in_a_crlf_file_spans_its_lines() {
        let source = text::read("\"a\",\"one\"\r\n\"b\",\"two\r\nthree\"\r\n\"c\",\"four\"\r\n");

        assert_eq!(spans(&source), vec![(1, 1), (2, 3), (4, 4)]);
        assert_eq!(source.records[1].fields.as_ref().unwrap()[1], "two\nthree");
    }

    #[test]
    fn the_last_record_without_a_trailing_newline_is_not_wrapped() {
        let source = tokenize("\"a\",\"one\"\n\"b\",\"two\"").unwrap();

        assert_eq!(spans(&source), vec![(1, 1), (2, 2)]);
    }

    #[test]
    fn blank_and_empty_field_records_are_dropped() {
        let source = tokenize("a,b,c\n,,\n\nd,e,f\n").unwrap();

        assert_eq!(source.records.len(), 2);
        assert_eq!(spans(&source), vec![(1, 1), (4, 4)]);
    }
}
