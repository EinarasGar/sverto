use std::collections::BTreeMap;

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::chat::TokenUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifiedKind {
    Expense,
    Refund,
    TransferOut,
    TransferIn,
    BankFee,
    CurrencyExchange,
    Interest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum MoneyDirection {
    #[serde(rename = "money out")]
    Out,
    #[serde(rename = "money in")]
    In,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationLine {
    pub description: Option<String>,
    pub amount: Decimal,
    pub currency: String,
    pub account: String,
    pub has_fee: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassificationGroup {
    pub index: u32,
    pub description: String,
    pub direction: MoneyDirection,
    pub typical_amount: String,
    pub times_seen: u32,
    pub account: String,
    pub includes_a_fee: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationCategory {
    pub id: i32,
    pub type_name: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupClassification {
    pub index: u32,
    pub classification: LineClassification,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineClassification {
    pub kind: ClassifiedKind,
    pub category_id: Option<i32>,
    pub confidence: f64,
    pub confident: bool,
}

#[derive(Debug)]
pub enum ClassificationOutcome {
    Classified(ClassificationRun),
    Stopped,
}

#[derive(Debug, Default)]
pub struct ClassificationRun {
    pub lines: Vec<Option<LineClassification>>,
    pub groups: usize,
    pub usage: TokenUsage,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompactClassification {
    #[schemars(description = "One row per item: [index, kind number, category_id, confidence].")]
    pub rows: Vec<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
pub struct SystemOneAnswers {
    pub kind: SystemOneChoice,
    pub category: SystemOneChoice,
}

#[derive(Debug, Deserialize)]
pub struct SystemOneChoice {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
}
