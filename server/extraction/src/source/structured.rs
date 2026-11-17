use std::collections::{HashMap, HashSet};
use std::ops::Range;

use super::delimited::{line_of, line_start_offsets};
use super::{blocks, json, markup};
use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

const MIN_UNIT_LEAVES: usize = 2;
pub const MAX_DEPTH: usize = 128;

pub struct Node {
    pub name: String,
    pub span: Range<usize>,
    pub head_end: usize,
    pub attributes: Vec<String>,
    pub text: Option<String>,
    pub children: Vec<Node>,
    pub is_list: bool,
    repeats: bool,
    leaves: usize,
    leaves_in_units: usize,
}

impl Node {
    pub fn new(name: String, start: usize) -> Self {
        Self {
            name,
            span: start..start,
            head_end: start,
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
            is_list: false,
            repeats: false,
            leaves: 0,
            leaves_in_units: 0,
        }
    }

    fn is_unit(&self) -> bool {
        self.leaves >= MIN_UNIT_LEAVES
    }

    fn values(&self, out: &mut Vec<String>) {
        out.extend(self.attributes.iter().cloned());
        if self.children.is_empty() && (self.text.is_some() || self.attributes.is_empty()) {
            out.push(self.text.clone().unwrap_or_default());
        }
        for child in &self.children {
            child.values(out);
        }
    }
}

struct RepeatedUnit {
    name: String,
    count: usize,
    first: usize,
    last: usize,
}

pub fn read(text: &str) -> Option<ImportSource> {
    if let Some(root) = json::parse(text) {
        return Some(tree_source(
            text,
            Vec::new(),
            root,
            "JSON",
            "`text` is the record's own JSON, so read a repeated element by key with JSON.parse(text); `fields` are its values in document order, null as an empty field",
        ));
    }
    if let Some((preamble, root)) = markup::parse(text) {
        return Some(tree_source(
            text,
            preamble,
            root,
            "tagged markup",
            "`text` is the record's own markup and `fields` are its attribute and element values in document order, so an optional element shifts the fields after it: find a value by its tag in `text`",
        ));
    }
    blocks::swift_fields(text).or_else(|| blocks::caret_terminated(text))
}

fn tree_source(
    text: &str,
    preamble: Vec<SourceRecord>,
    mut root: Node,
    format: &str,
    reading: &str,
) -> ImportSource {
    let mut unit_names = HashSet::new();
    measure(&mut root, &mut unit_names);
    let line_starts = line_start_offsets(text);
    let mut records = preamble;
    let mut units = Vec::new();
    let mut flow = Flow {
        text,
        line_starts: &line_starts,
        unit_names: &unit_names,
        records: &mut records,
        units: &mut units,
        unit_positions: HashMap::new(),
    };
    flow.through(&root);

    let repeated = if units.is_empty() {
        "nothing in it repeats, so each group of values is one record, in document order"
            .to_string()
    } else {
        format!(
            "one record per repeated element ({}), then one per group of values that does not repeat, in document order",
            units
                .iter()
                .map(|unit| format!(
                    "{} ×{} at refs {}–{}",
                    unit.name, unit.count, unit.first, unit.last
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    ImportSource {
        kind: SourceKind::Text,
        records,
        notes: vec![format!("{format}: {repeated}; {reading}")],
        pages_without_text: Vec::new(),
    }
}

fn measure(node: &mut Node, unit_names: &mut HashSet<String>) {
    if node.children.is_empty() {
        let only_attributes = node.text.is_none() && node.attributes.len() >= MIN_UNIT_LEAVES;
        node.leaves = if only_attributes {
            node.attributes.len()
        } else {
            1
        };
        return;
    }
    for child in &mut node.children {
        measure(child, unit_names);
    }
    let mut unit_counts: HashMap<&str, usize> = HashMap::new();
    for child in node.children.iter().filter(|child| child.is_unit()) {
        *unit_counts.entry(&child.name).or_default() += 1;
    }
    let repeats: Vec<bool> = node
        .children
        .iter()
        .map(|child| child.is_unit() && (node.is_list || unit_counts[child.name.as_str()] > 1))
        .collect();
    let (mut leaves, mut leaves_in_units) = (0, 0);
    for (child, repeats) in node.children.iter_mut().zip(repeats) {
        child.repeats = repeats;
        leaves += child.leaves;
        if repeats {
            unit_names.insert(child.name.clone());
            leaves_in_units += child.leaves;
        } else {
            leaves_in_units += child.leaves_in_units;
        }
    }
    (node.leaves, node.leaves_in_units) = (leaves, leaves_in_units);
}

struct Flow<'a> {
    text: &'a str,
    line_starts: &'a [usize],
    unit_names: &'a HashSet<String>,
    records: &'a mut Vec<SourceRecord>,
    units: &'a mut Vec<RepeatedUnit>,
    unit_positions: HashMap<String, usize>,
}

impl Flow<'_> {
    fn through(&mut self, node: &Node) {
        if !node.attributes.is_empty() {
            self.push(node.span.start..node.head_end, node.attributes.clone());
        }
        let mut repeated: HashMap<&str, (usize, usize)> = HashMap::new();
        for child in node.children.iter().filter(|child| child.repeats) {
            let (leaves, leaves_in_units) = repeated.entry(&child.name).or_default();
            *leaves += child.leaves;
            *leaves_in_units += child.leaves_in_units;
        }

        for child in &node.children {
            let is_unit =
                child.repeats || (child.is_unit() && self.unit_names.contains(&child.name));
            let whole = match repeated.get(child.name.as_str()).filter(|_| child.repeats) {
                Some((leaves, leaves_in_units)) => leaves_in_units * 2 <= *leaves,
                None if is_unit => child.leaves_in_units * 2 <= child.leaves,
                None => {
                    child.leaves_in_units == 0
                        && child.children.iter().all(|part| part.children.is_empty())
                }
            };
            if !whole {
                self.through(child);
                continue;
            }
            if is_unit {
                let index = self.records.len();
                match self.unit_positions.get(child.name.as_str()) {
                    Some(position) => {
                        let unit = &mut self.units[*position];
                        unit.count += 1;
                        unit.last = index;
                    }
                    None => {
                        self.unit_positions
                            .insert(child.name.clone(), self.units.len());
                        self.units.push(RepeatedUnit {
                            name: child.name.clone(),
                            count: 1,
                            first: index,
                            last: index,
                        });
                    }
                }
            }
            let mut fields = Vec::new();
            child.values(&mut fields);
            self.push(child.span.clone(), fields);
        }
    }

    fn push(&mut self, span: Range<usize>, fields: Vec<String>) {
        self.records.push(SourceRecord {
            location: SourceLocation::Lines {
                first: line_of(self.line_starts, span.start),
                last: line_of(self.line_starts, span.end.saturating_sub(1).max(span.start)),
            },
            text: self.text[span]
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            fields: Some(fields),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &ImportSource) -> Vec<&str> {
        source.records.iter().map(|r| r.text.as_str()).collect()
    }

    fn fields(source: &ImportSource, index: usize) -> Vec<&str> {
        source.records[index]
            .fields
            .iter()
            .flatten()
            .map(String::as_str)
            .collect()
    }

    fn lines(source: &ImportSource, index: usize) -> (u32, u32) {
        match source.records[index].location {
            SourceLocation::Lines { first, last } => (first, last),
            _ => panic!("structured records are located by lines"),
        }
    }

    const EXPORT: &str = r#"{
  "account": {"iban": "GB69KITE04007518273645", "currency": "GBP"},
  "from": "2024-01-01",
  "to": "2024-01-31",
  "transactions": [
    {
      "id": "tx_1",
      "booked_at": "2024-01-02T10:00:00Z",
      "amount": {"value": "-3.50", "currency": "GBP"},
      "description": "Café \"Nero\"",
      "reference": null
    },
    {
      "id": "tx_2",
      "booked_at": "2024-01-03T09:30:00Z",
      "amount": {"value": "1000.00", "currency": "GBP"},
      "description": "Salary",
      "reference": "JAN"
    }
  ],
  "closing_balance": "996.50"
}"#;

    #[test]
    fn json_array_elements_are_records_and_the_rest_keeps_its_own() {
        let source = read(EXPORT).unwrap();

        assert_eq!(source.kind, SourceKind::Text);
        assert_eq!(source.records.len(), 6, "{:#?}", texts(&source));
        assert_eq!(fields(&source, 0), vec!["GB69KITE04007518273645", "GBP"]);
        assert_eq!(fields(&source, 1), vec!["2024-01-01"]);
        assert_eq!(fields(&source, 2), vec!["2024-01-31"]);
        assert_eq!(
            fields(&source, 3),
            vec![
                "tx_1",
                "2024-01-02T10:00:00Z",
                "-3.50",
                "GBP",
                "Café \"Nero\"",
                ""
            ]
        );
        assert_eq!(lines(&source, 3), (6, 12));
        assert_eq!(lines(&source, 4), (13, 19));
        assert_eq!(fields(&source, 5), vec!["996.50"]);
        assert!(source.notes[0].contains("transactions ×2 at refs 3–4"));
    }

    #[test]
    fn a_json_record_is_its_own_json() {
        let source = read(EXPORT).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&source.records[4].text).unwrap();

        assert_eq!(parsed["amount"]["value"], "1000.00");
        assert_eq!(source.records[1].text, "\"from\": \"2024-01-01\"");
    }

    #[test]
    fn json_numbers_keep_the_digits_the_file_wrote() {
        let source =
            read(r#"[{"amount": -3.50, "units": 1e-8}, {"amount": 10.00, "units": 2}]"#).unwrap();

        assert_eq!(fields(&source, 0), vec!["-3.50", "1e-8"]);
        assert_eq!(fields(&source, 1), vec!["10.00", "2"]);
    }

    #[test]
    fn nested_arrays_are_split_down_to_the_transactions() {
        let source = read(
            r#"{"accounts": [
                {"iban": "LT01", "currency": "EUR", "transactions": [
                    {"date": "2024-01-02", "amount": "-3.50"},
                    {"date": "2024-01-03", "amount": "-4.00"}]},
                {"iban": "LT02", "currency": "USD", "transactions": [
                    {"date": "2024-01-05", "amount": "12.00"}]}]}"#,
        )
        .unwrap();

        assert_eq!(
            (0..source.records.len())
                .map(|index| fields(&source, index))
                .collect::<Vec<_>>(),
            vec![
                vec!["LT01"],
                vec!["EUR"],
                vec!["2024-01-02", "-3.50"],
                vec!["2024-01-03", "-4.00"],
                vec!["LT02"],
                vec!["USD"],
                vec!["2024-01-05", "12.00"],
            ]
        );
    }

    #[test]
    fn json_lines_are_read_as_the_elements_of_a_list() {
        let source = read(
            "{\"date\":\"2024-01-02\",\"amount\":\"-3.50\",\"name\":\"a, b\"}\n\n\
             {\"date\":\"2024-01-03\",\"amount\":\"4.00\",\"name\":\"c\"}\n",
        )
        .unwrap();

        assert_eq!(source.records.len(), 2);
        assert_eq!(fields(&source, 0), vec!["2024-01-02", "-3.50", "a, b"]);
        assert_eq!(lines(&source, 1), (3, 3));
        assert_eq!(
            source.records[1].text,
            "{\"date\":\"2024-01-03\",\"amount\":\"4.00\",\"name\":\"c\"}"
        );
        assert!(read("{\"date\":\"2024-01-02\"}\nnot json\n").is_none());
    }

    #[test]
    fn a_string_serde_cannot_decode_keeps_what_the_file_wrote() {
        let source = read(r#"[{"id": "a", "name": "\ud800"}, {"id": "b", "name": "x"}]"#).unwrap();

        assert_eq!(fields(&source, 0), vec!["a", "\\ud800"]);
    }

    #[test]
    fn text_that_is_not_a_json_document_is_left_to_other_readers() {
        assert!(read("[2024-01-02] coffee 3.50\n[2024-01-03] rent 500.00\n").is_none());
        assert!(read("\"just a string\"").is_none());
        assert!(read("{\"date\": \"2024-01-02\"").is_none());
    }

    const STATEMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">
  <BkToCstmrStmt>
    <Stmt>
      <Id>STMT-1</Id>
      <Acct><Id><IBAN>CH3708390037154000120</IBAN></Id><Ccy>CHF</Ccy></Acct>
      <Bal>
        <Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp>
        <Amt Ccy="CHF">100.00</Amt>
        <Dt><Dt>2024-01-01</Dt></Dt>
      </Bal>
      <Bal>
        <Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp>
        <Amt Ccy="CHF">52.50</Amt>
        <Dt><Dt>2024-01-31</Dt></Dt>
      </Bal>
      <Ntry>
        <Amt Ccy="CHF">40.00</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <BookgDt><Dt>2024-01-02</Dt></BookgDt>
        <NtryDtls>
          <TxDtls><Amt Ccy="CHF">15.00</Amt><RmtInf><Ustrd>Parking</Ustrd></RmtInf></TxDtls>
          <TxDtls><Amt Ccy="CHF">25.00</Amt><RmtInf><Ustrd>Rent &amp; water</Ustrd></RmtInf></TxDtls>
        </NtryDtls>
      </Ntry>
      <Ntry>
        <Amt Ccy="CHF">7.50</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <BookgDt><Dt>2024-01-03</Dt></BookgDt>
        <NtryDtls>
          <TxDtls><Amt Ccy="CHF">7.50</Amt><RmtInf><Ustrd><![CDATA[Migros <Limmatplatz>]]></Ustrd></RmtInf></TxDtls>
        </NtryDtls>
      </Ntry>
      <Ntry>
        <Amt Ccy="CHF">0.00</Amt>
        <CdtDbtInd>CRDT</CdtDbtInd>
        <BookgDt><Dt>2024-01-04</Dt></BookgDt>
        <NtryDtls>
          <TxDtls><Amt Ccy="CHF">0.00</Amt><RmtInf><Ustrd>Interest</Ustrd></RmtInf></TxDtls>
        </NtryDtls>
      </Ntry>
    </Stmt>
  </BkToCstmrStmt>
</Document>
"#;

    #[test]
    fn repeated_xml_elements_are_records_with_their_values_as_fields() {
        let source = read(STATEMENT).unwrap();

        assert_eq!(source.records.len(), 9, "{:#?}", texts(&source));
        assert_eq!(
            fields(&source, 0),
            vec!["urn:iso:std:iso:20022:tech:xsd:camt.053.001.08"]
        );
        assert_eq!(fields(&source, 1), vec!["STMT-1"]);
        assert_eq!(fields(&source, 2), vec!["CH3708390037154000120"]);
        assert_eq!(fields(&source, 3), vec!["CHF"]);
        assert_eq!(
            fields(&source, 4),
            vec!["OPBD", "CHF", "100.00", "2024-01-01"]
        );
        assert_eq!(
            fields(&source, 7),
            vec![
                "CHF",
                "7.50",
                "DBIT",
                "2024-01-03",
                "CHF",
                "7.50",
                "Migros <Limmatplatz>"
            ]
        );
        assert_eq!(lines(&source, 7), (26, 33));
        assert!(source.records[7]
            .text
            .starts_with("<Ntry>\n<Amt Ccy=\"CHF\">7.50</Amt>"));
        assert!(source.records[7].text.ends_with("</Ntry>"));
        assert!(source.notes[0].contains("Bal ×2 at refs 4–5, Ntry ×3 at refs 6–8"));
    }

    #[test]
    fn one_entry_that_itemises_its_payments_stays_one_record() {
        let source = read(STATEMENT).unwrap();

        assert_eq!(
            fields(&source, 6),
            vec![
                "CHF",
                "40.00",
                "DBIT",
                "2024-01-02",
                "CHF",
                "15.00",
                "Parking",
                "CHF",
                "25.00",
                "Rent & water"
            ]
        );
    }

    #[test]
    fn a_doctype_with_an_internal_subset_is_skipped_whole() {
        let source = read(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE Stmt [\n<!ENTITY bank \"Kite\">\n]>\n\
             <Stmt><Ntry><Amt>3.50</Amt><Dt>2024-01-02</Dt></Ntry>\
             <Ntry><Amt>4.00</Amt><Dt>2024-01-03</Dt></Ntry></Stmt>",
        )
        .unwrap();

        assert_eq!(source.records.len(), 2);
        assert_eq!(fields(&source, 1), vec!["4.00", "2024-01-03"]);
    }

    #[test]
    fn a_text_of_ampersands_or_of_many_repeated_names_is_read_in_linear_time() {
        let ampersands = format!("<a><b>{}</b><b>x</b></a>", "&".repeat(1_000_000));
        let mut names = String::from("<r>");
        for name in 0..40_000 {
            names.push_str(&format!(
                "<n{name}><a>1</a><b>2</b></n{name}><n{name}><a>1</a><b>2</b></n{name}>"
            ));
        }
        names.push_str("</r>");
        let started = std::time::Instant::now();

        assert!(read(&ampersands).is_some());
        assert_eq!(read(&names).unwrap().records.len(), 80_000);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn a_statement_with_a_single_entry_keeps_it_whole_beside_a_longer_one() {
        let entry = |day: u8| {
            format!(
                "<Ntry><Amt Ccy=\"EUR\">{day}.00</Amt><BookgDt><Dt>2024-01-{day:02}</Dt></BookgDt></Ntry>"
            )
        };
        let source = read(&format!(
            "<Document><Stmt><Id>A</Id>{}{}{}</Stmt><Stmt><Id>B</Id>{}</Stmt></Document>",
            entry(2),
            entry(3),
            entry(4),
            entry(9)
        ))
        .unwrap();

        assert_eq!(
            (0..source.records.len())
                .map(|index| fields(&source, index))
                .collect::<Vec<_>>(),
            vec![
                vec!["A"],
                vec!["EUR", "2.00", "2024-01-02"],
                vec!["EUR", "3.00", "2024-01-03"],
                vec!["EUR", "4.00", "2024-01-04"],
                vec!["B"],
                vec!["EUR", "9.00", "2024-01-09"],
            ]
        );
    }

    #[test]
    fn elements_that_hold_only_attributes_are_records() {
        let source = read(
            "<Report account=\"U123\" from=\"20240101\" to=\"20240131\">\n<Trades>\n\
             <Trade date=\"20240102\" symbol=\"VWRL\" quantity=\"2\" cost=\"-200.00\" />\n\
             <Trade date=\"20240105\" symbol=\"AAPL\" quantity=\"1\" cost=\"-180.00\" />\n\
             </Trades>\n</Report>\n",
        )
        .unwrap();

        assert_eq!(fields(&source, 0), vec!["U123", "20240101", "20240131"]);
        assert_eq!(fields(&source, 1), vec!["20240102", "VWRL", "2", "-200.00"]);
        assert_eq!(fields(&source, 2), vec!["20240105", "AAPL", "1", "-180.00"]);
        assert_eq!(source.records.len(), 3);
    }

    const OFX: &str = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\n\n<OFX>\n<BANKMSGSRSV1>\n<STMTTRNRS>\n<STMTRS>\n\
<CURDEF>GBP\n<BANKACCTFROM>\n<BANKID>089250\n<ACCTID>40017725\n</BANKACCTFROM>\n<BANKTRANLIST>\n\
<DTSTART>20240101\n<DTEND>20240131\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20240102\n<TRNAMT>-12.50\n<FITID>1001\n<NAME>TESCO\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20240103\n<TRNAMT>1000.00\n<FITID>1002\n<NAME>SALARY\n<MEMO>JAN\n</STMTTRN>\n\
</BANKTRANLIST>\n<LEDGERBAL>\n<BALAMT>987.50\n<DTASOF>20240131\n</LEDGERBAL>\n</STMTRS>\n</STMTTRNRS>\n</BANKMSGSRSV1>\n</OFX>\n";

    #[test]
    fn ofx_transactions_without_closing_tags_are_records() {
        let source = read(OFX).unwrap();

        assert_eq!(
            texts(&source)[..3],
            ["OFXHEADER:100", "DATA:OFXSGML", "VERSION:102"]
        );
        assert_eq!(source.records[0].fields, None);
        assert_eq!(fields(&source, 3), vec!["GBP"]);
        assert_eq!(fields(&source, 4), vec!["089250", "40017725"]);
        assert_eq!(fields(&source, 5), vec!["20240101"]);
        assert_eq!(fields(&source, 6), vec!["20240131"]);
        assert_eq!(
            fields(&source, 7),
            vec!["DEBIT", "20240102", "-12.50", "1001", "TESCO"]
        );
        assert_eq!(lines(&source, 7), (17, 23));
        assert_eq!(
            source.records[8].text,
            "<STMTTRN>\n<TRNTYPE>CREDIT\n<DTPOSTED>20240103\n<TRNAMT>1000.00\n<FITID>1002\n<NAME>SALARY\n<MEMO>JAN\n</STMTTRN>"
        );
        assert_eq!(fields(&source, 9), vec!["987.50", "20240131"]);
        assert_eq!(source.records.len(), 10);
    }

    #[test]
    fn a_lone_transaction_does_not_share_a_record_with_the_balance() {
        let source = read(
            "<OFX><STMTRS><BANKTRANLIST><DTSTART>20240101</DTSTART><DTEND>20240131</DTEND>\
             <STMTTRN><DTPOSTED>20240102</DTPOSTED><TRNAMT>-12.50</TRNAMT><NAME>TESCO</NAME></STMTTRN>\
             </BANKTRANLIST><LEDGERBAL><BALAMT>987.50</BALAMT><DTASOF>20240131</DTASOF></LEDGERBAL>\
             </STMTRS></OFX>",
        )
        .unwrap();

        assert_eq!(
            (0..source.records.len())
                .map(|index| fields(&source, index))
                .collect::<Vec<_>>(),
            vec![
                vec!["20240101"],
                vec!["20240131"],
                vec!["20240102", "-12.50", "TESCO"],
                vec!["987.50", "20240131"],
            ]
        );
        assert!(source.notes[0].contains("nothing in it repeats"));
    }

    #[test]
    fn text_that_only_mentions_a_tag_is_left_to_other_readers() {
        assert!(read("Date,Description,Amount\n2024-01-02,<none>,-3.50\n").is_none());
        assert!(read("coffee <b>3.50</b> yesterday\n").is_none());
        assert!(read("<Document><Stmt><Id>A</Id></Stmt>").is_none());
        assert!(read("<a>one</a><b>two</b>").is_none());
        assert!(read("<Document><Id>A</Id></Document> and more").is_none());
    }
}
