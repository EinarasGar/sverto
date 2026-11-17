use std::sync::LazyLock;

use regex::Regex;

use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

static SWIFT_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^:(\d{2}[A-Z]?):").expect("valid regex"));
static SWIFT_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{6})(\d{4})?(R?[CD])([A-Z])?(\d+,\d*)([A-Z][A-Z0-9]{3})?(.*)$")
        .expect("valid regex")
});
static SWIFT_BALANCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([CD])(\d{6})([A-Z]{3})(\d+,\d*)$").expect("valid regex"));
const SWIFT_ENTRY_TAG: &str = "61";
const SWIFT_ENTRY_DETAILS_TAG: &str = "86";
const MIN_SWIFT_FIELDS: usize = 2;
const QIF_RECORD_END: &str = "^";

pub fn swift_fields(text: &str) -> Option<ImportSource> {
    let grammar = |tag: &str| {
        if tag == SWIFT_ENTRY_TAG {
            &SWIFT_ENTRY
        } else {
            &SWIFT_BALANCE
        }
    };
    let states_money = text.lines().any(|line| {
        let line = line.trim_end();
        SWIFT_TAG
            .captures(line)
            .is_some_and(|tag| grammar(&tag[1]).is_match(&line[tag[0].len()..]))
    });
    if !states_money
        || text.lines().filter(|line| SWIFT_TAG.is_match(line)).count() < MIN_SWIFT_FIELDS
    {
        return None;
    }
    let mut records: Vec<SourceRecord> = Vec::new();
    let mut open_tag: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index as u32 + 1;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let tag = SWIFT_TAG.captures(line);
        let continues = match &tag {
            Some(tag) => {
                &tag[1] == SWIFT_ENTRY_DETAILS_TAG && open_tag.as_deref() == Some(SWIFT_ENTRY_TAG)
            }
            None => open_tag.is_some() && line != "-" && !line.starts_with("-}"),
        };
        let fields = match &tag {
            Some(tag) => {
                let content = &line[tag[0].len()..];
                match grammar(&tag[1]).captures(content) {
                    Some(parts) => parts
                        .iter()
                        .skip(1)
                        .map(|part| part.map_or("", |found| found.as_str()).to_string())
                        .collect(),
                    None => vec![content.to_string()],
                }
            }
            None => vec![line.to_string()],
        };
        match records.last_mut().filter(|_| continues) {
            Some(record) => {
                record.text.push('\n');
                record.text.push_str(line);
                if let SourceLocation::Lines { last, .. } = &mut record.location {
                    *last = number;
                }
                record.fields.get_or_insert_default().extend(fields);
            }
            None => records.push(SourceRecord {
                location: SourceLocation::Lines {
                    first: number,
                    last: number,
                },
                text: line.to_string(),
                fields: tag.is_some().then_some(fields),
            }),
        }
        match tag {
            Some(tag) if !continues => open_tag = Some(tag[1].to_string()),
            Some(_) => {}
            None if !continues => open_tag = None,
            None => {}
        }
    }
    Some(ImportSource {
        kind: SourceKind::Text,
        records,
        notes: vec![
            "SWIFT MT fields: one record per `:tag:` field with the lines that continue it, and a `:61:` entry together with the `:86:` details after it; `fields` split a `:61:` into value date, entry date (the booking date: four digits, month then day, cited as printed; the value date before it gives the year), debit/credit mark, funds code, amount, type code and reference, a balance field into mark, date, currency and amount, and hold everything else line by line"
                .to_string(),
        ],
        pages_without_text: Vec::new(),
    })
}

pub fn caret_terminated(text: &str) -> Option<ImportSource> {
    let starts_with_heading = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.starts_with('!'));
    if !starts_with_heading || !text.lines().any(|line| line.trim() == QIF_RECORD_END) {
        return None;
    }
    let mut records: Vec<SourceRecord> = Vec::new();
    let mut open = false;
    for (index, line) in text.lines().enumerate() {
        let number = index as u32 + 1;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if line == QIF_RECORD_END {
            open = false;
            continue;
        }
        let heading = line.starts_with('!');
        match records.last_mut().filter(|_| open && !heading) {
            Some(record) => {
                record.text.push('\n');
                record.text.push_str(line);
                if let SourceLocation::Lines { last, .. } = &mut record.location {
                    *last = number;
                }
                record
                    .fields
                    .get_or_insert_default()
                    .push(line.chars().skip(1).collect());
            }
            None => records.push(SourceRecord {
                location: SourceLocation::Lines {
                    first: number,
                    last: number,
                },
                text: line.to_string(),
                fields: (!heading).then(|| vec![line.chars().skip(1).collect()]),
            }),
        }
        open = !heading;
    }
    Some(ImportSource {
        kind: SourceKind::Text,
        records,
        notes: vec![
            "QIF: one record per block of lines ending at `^`; each line of `text` starts with its one-letter code and `fields` hold the lines without that letter; `!` lines are section headings"
                .to_string(),
        ],
        pages_without_text: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(source: &ImportSource, index: usize) -> Vec<&str> {
        source.records[index]
            .fields
            .iter()
            .flatten()
            .map(String::as_str)
            .collect()
    }

    const MT940: &str = "{1:F01BANKDEFFXXXX0000000000}{2:I940BANKDEFFXXXXN}{4:\n\
:20:STARTUMSE\n:25:DE43700543060012345678\n:28C:00010/001\n:60F:C240101EUR2345,67\n\
:61:2401020103D1240,00NSTONONREF//2934703340\n\
:86:152?00DAUERAUFTRAG?20SVWZ+Miete 01/2024 Whg 4 OG li\nnks?32Hausverwaltung Berger GmbH\n\
:61:240105C3200,5NTRFSALARY-JAN\nsupplementary details\n:86:Gehalt Januar\n\
:62F:C240131EUR4306,17\n:86:Kontoabschluss\n-}\n";

    #[test]
    fn a_swift_entry_and_its_details_are_one_record() {
        let source = swift_fields(MT940).unwrap();

        assert_eq!(source.kind, SourceKind::Text);
        assert_eq!(source.records.len(), 10);
        assert_eq!(
            source.records[5].text,
            ":61:2401020103D1240,00NSTONONREF//2934703340\n:86:152?00DAUERAUFTRAG?20SVWZ+Miete 01/2024 Whg 4 OG li\nnks?32Hausverwaltung Berger GmbH"
        );
        assert_eq!(
            source.records[5].location,
            SourceLocation::Lines { first: 6, last: 8 }
        );
        assert_eq!(
            fields(&source, 5),
            vec![
                "240102",
                "0103",
                "D",
                "",
                "1240,00",
                "NSTO",
                "NONREF//2934703340",
                "152?00DAUERAUFTRAG?20SVWZ+Miete 01/2024 Whg 4 OG li",
                "nks?32Hausverwaltung Berger GmbH"
            ]
        );
        assert_eq!(
            fields(&source, 6),
            vec![
                "240105",
                "",
                "C",
                "",
                "3200,5",
                "NTRF",
                "SALARY-JAN",
                "supplementary details",
                "Gehalt Januar"
            ]
        );
    }

    #[test]
    fn swift_balances_and_other_fields_are_their_own_records() {
        let source = swift_fields(MT940).unwrap();

        assert_eq!(source.records[0].fields, None);
        assert_eq!(fields(&source, 2), vec!["DE43700543060012345678"]);
        assert_eq!(fields(&source, 4), vec!["C", "240101", "EUR", "2345,67"]);
        assert_eq!(fields(&source, 7), vec!["C", "240131", "EUR", "4306,17"]);
        assert_eq!(fields(&source, 8), vec!["Kontoabschluss"]);
        assert_eq!(source.records[9].text, "-}");
        assert_eq!(source.records[9].fields, None);
    }

    const QIF: &str = "!Account\nNEveryday Checking\nTBank\n^\n!Type:Bank\n\
D1/2'24\nT-400.00\nPTransfer to Savings\nMOnline transfer\n^\n\
D1/3'24\nT-1,875.00\nN2041\nPMarina Vista Apartments\nSHousing:Rent\n$-1,800.00\nSUtilities\n$-75.00\n^\n\
D1/5'24\nT0.13\nPInterest Earned\n";

    #[test]
    fn qif_blocks_ending_at_a_caret_are_records() {
        let source = caret_terminated(QIF).unwrap();

        assert_eq!(source.kind, SourceKind::Text);
        assert_eq!(
            source
                .records
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "!Account",
                "NEveryday Checking\nTBank",
                "!Type:Bank",
                "D1/2'24\nT-400.00\nPTransfer to Savings\nMOnline transfer",
                "D1/3'24\nT-1,875.00\nN2041\nPMarina Vista Apartments\nSHousing:Rent\n$-1,800.00\nSUtilities\n$-75.00",
                "D1/5'24\nT0.13\nPInterest Earned",
            ]
        );
        assert_eq!(
            fields(&source, 3),
            vec![
                "1/2'24",
                "-400.00",
                "Transfer to Savings",
                "Online transfer"
            ]
        );
        assert_eq!(
            source.records[4].location,
            SourceLocation::Lines {
                first: 11,
                last: 18
            }
        );
        assert_eq!(source.records[0].fields, None);
    }

    #[test]
    fn other_text_is_not_taken_for_a_block_format() {
        assert!(swift_fields("Date,Amount\n2024-01-02,-3.50\n").is_none());
        assert!(swift_fields(":20:only one tagged line\nand prose\n").is_none());
        assert!(swift_fields(
            "Notes\n:10: first thing 3.50\n:20: second thing 4.00\nDate,Amount\n2024-01-02,-3.50\n"
        )
        .is_none());
        assert!(caret_terminated("Date,Amount\n2024-01-02,-3.50\n").is_none());
        assert!(caret_terminated("rate 2^10\n^\n").is_none());
    }
}
