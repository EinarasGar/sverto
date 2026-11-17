use std::str::FromStr;

use rust_decimal::Decimal;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use time::macros::format_description;
use time::{Date, Time};

time::serde::format_description!(pub iso_date, Date, "[year]-[month]-[day]");

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionOutput {
    #[serde(default)]
    pub lines: Vec<ExtractedLine>,
    #[serde(default)]
    pub balances: Vec<StatedBalance>,
    #[serde(default)]
    pub control_totals: Vec<StatedControlTotal>,
    #[serde(default)]
    pub skipped: Vec<SkippedRecords>,
    #[serde(default)]
    pub period: Option<StatedPeriod>,
    #[serde(default)]
    pub minor_units: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedLine {
    #[serde(default)]
    pub refs: Vec<usize>,
    #[serde(default)]
    pub date_refs: Vec<usize>,
    #[serde(with = "iso_date")]
    pub date: Date,
    pub date_text: String,
    #[serde(default, deserialize_with = "optional_time")]
    pub time: Option<Time>,
    #[serde(default)]
    pub description: Option<String>,
    pub account_ref: String,
    #[serde(default)]
    pub transaction_key: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub related_asset_ref: Option<String>,
    pub legs: Vec<ExtractedLeg>,
    #[serde(default)]
    pub balances: Vec<PrintedBalance>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedLeg {
    pub asset_ref: String,
    #[serde(deserialize_with = "decimal_string")]
    pub quantity: Decimal,
    pub amount_text: String,
    #[serde(default)]
    pub fee_type: Option<ExtractedFeeType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractedFeeType {
    Transaction,
    Exchange,
    WithholdingTax,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintedBalance {
    pub asset_ref: String,
    #[serde(deserialize_with = "decimal_string")]
    pub quantity: Decimal,
    pub amount_text: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatedBalance {
    #[serde(default)]
    pub refs: Vec<usize>,
    pub kind: BalanceKind,
    pub account_ref: String,
    pub asset_ref: String,
    #[serde(with = "iso_date")]
    pub as_of_date: Date,
    pub date_text: String,
    #[serde(deserialize_with = "decimal_string")]
    pub quantity: Decimal,
    pub amount_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceKind {
    Opening,
    Closing,
    Snapshot,
    ValueTotal,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatedControlTotal {
    #[serde(default)]
    pub refs: Vec<usize>,
    pub kind: ControlTotalKind,
    pub account_ref: String,
    #[serde(default)]
    pub asset_ref: Option<String>,
    #[serde(deserialize_with = "decimal_string")]
    pub quantity: Decimal,
    pub amount_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlTotalKind {
    DebitTotal,
    CreditTotal,
    NetTotal,
    RowCount,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkippedRecords {
    pub refs: Vec<usize>,
    pub class: SkipClass,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipClass {
    Header,
    Footer,
    Noise,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatedPeriod {
    #[serde(default)]
    pub refs: Vec<usize>,
    #[serde(with = "iso_date")]
    pub from: Date,
    pub from_text: String,
    #[serde(with = "iso_date")]
    pub to: Date,
    pub to_text: String,
}

fn decimal_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Decimal, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    value
        .as_str()
        .and_then(|text| Decimal::from_str(text.trim()).ok())
        .ok_or_else(|| {
            D::Error::custom(format!(
                "{value} is not a decimal string such as \"-12.34\". toDecimal returns one; arithmetic on it gives a number, NaN or joined text, so subtract or negate with minus(a, b)"
            ))
        })
}

fn optional_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Time>, D::Error> {
    let Some(text) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    Time::parse(&text, format_description!("[hour]:[minute]:[second]"))
        .or_else(|_| Time::parse(&text, format_description!("[hour]:[minute]")))
        .map(Some)
        .map_err(|_| {
            D::Error::custom(format!(
                "{text:?} is not a time like \"14:05:09\" or \"14:05\""
            ))
        })
}
