use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::{Map, Value};

use crate::ibkr::models::FlexReply;
use crate::Result;

/// Names the Flex section a row came from; rows of all sections share one page.
pub(crate) const SECTION_KEY: &str = "_section";

const ROW_ELEMENTS: [&str; 5] = [
    "AccountInformation",
    "Trade",
    "CashTransaction",
    "OpenPosition",
    "CashReportCurrency",
];

pub(crate) fn parse_reply(xml: &str) -> Result<FlexReply> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut is_statement = false;
    let mut rows = Vec::new();
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;

    loop {
        match reader.read_event()? {
            Event::Start(element) => {
                let name = element_name(&element)?;
                if name == "FlexQueryResponse" {
                    is_statement = true;
                }
                if let Some(row) = row_from(&name, &element)? {
                    rows.push(row);
                }
                current = Some(name);
            }
            Event::Empty(element) => {
                let name = element_name(&element)?;
                if let Some(row) = row_from(&name, &element)? {
                    rows.push(row);
                }
            }
            Event::Text(text) => {
                if let Some(name) = &current {
                    fields
                        .entry(name.clone())
                        .or_default()
                        .push_str(&text.decode()?);
                }
            }
            Event::End(_) => current = None,
            Event::Eof => break,
            _ => {}
        }
    }

    if is_statement {
        return Ok(FlexReply::Statement(rows));
    }

    let status = fields.get("Status").cloned().unwrap_or_default();
    if status == "Success" {
        let reference_code = fields
            .remove("ReferenceCode")
            .ok_or_else(|| anyhow::anyhow!("IBKR flex response has no ReferenceCode"))?;
        return Ok(FlexReply::Accepted { reference_code });
    }

    Ok(FlexReply::Failed {
        code: fields
            .get("ErrorCode")
            .and_then(|code| code.trim().parse().ok()),
        message: fields
            .remove("ErrorMessage")
            .unwrap_or_else(|| format!("unexpected flex response status: {status:?}")),
    })
}

fn element_name(element: &BytesStart) -> Result<String> {
    Ok(std::str::from_utf8(element.name().as_ref())?.to_string())
}

fn row_from(name: &str, element: &BytesStart) -> Result<Option<Value>> {
    if !ROW_ELEMENTS.contains(&name) {
        return Ok(None);
    }
    let mut row = Map::new();
    row.insert(SECTION_KEY.to_string(), Value::String(name.to_string()));
    for attribute in element.attributes() {
        let attribute = attribute?;
        let key = std::str::from_utf8(attribute.key.as_ref())?.to_string();
        row.insert(key, Value::String(attribute.unescape_value()?.into_owned()));
    }
    Ok(Some(Value::Object(row)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accepted_envelope() {
        let xml = r#"<FlexStatementResponse timestamp="06 October, 2026 09:00 AM EDT">
<Status>Success</Status>
<ReferenceCode>1234567890</ReferenceCode>
<Url>https://ndcdyn.interactivebrokers.com/AccountManagement/FlexWebService/GetStatement</Url>
</FlexStatementResponse>"#;
        match parse_reply(xml).unwrap() {
            FlexReply::Accepted { reference_code } => {
                assert_eq!(reference_code, "1234567890");
            }
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn parses_failed_envelope() {
        let xml = r#"<FlexStatementResponse timestamp="06 October, 2026 09:00 AM EDT">
<Status>Warn</Status>
<ErrorCode>1019</ErrorCode>
<ErrorMessage>Statement generation in progress. Please try again shortly.</ErrorMessage>
</FlexStatementResponse>"#;
        match parse_reply(xml).unwrap() {
            FlexReply::Failed { code, message } => {
                assert_eq!(code, Some(1019));
                assert!(message.starts_with("Statement generation"));
            }
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn parses_statement_rows_with_section() {
        let xml = r#"<FlexQueryResponse queryName="sverto" type="AF">
<FlexStatements count="1">
<FlexStatement accountId="U1234567" fromDate="20260101" toDate="20261005">
<Trades>
<Trade accountId="U1234567" currency="USD" assetCategory="STK" symbol="AAPL" description="APPLE INC" listingExchange="NASDAQ" tradeID="111" dateTime="20260105;093512" quantity="10" netCash="-1901.25" buySell="BUY" levelOfDetail="EXECUTION" />
</Trades>
<CashTransactions>
<CashTransaction currency="EUR" type="Deposits/Withdrawals" amount="500" description="CASH RECEIPTS &amp; DEPOSITS" transactionID="222" dateTime="20260102" levelOfDetail="DETAIL" />
</CashTransactions>
</FlexStatement>
</FlexStatements>
</FlexQueryResponse>"#;
        let FlexReply::Statement(rows) = parse_reply(xml).unwrap() else {
            panic!("expected statement");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][SECTION_KEY], "Trade");
        assert_eq!(rows[0]["tradeID"], "111");
        assert_eq!(rows[1][SECTION_KEY], "CashTransaction");
        assert_eq!(rows[1]["description"], "CASH RECEIPTS & DEPOSITS");
    }
}
