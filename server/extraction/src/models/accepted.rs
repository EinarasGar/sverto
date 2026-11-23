use rust_decimal::Decimal;
use time::Date;

use super::output::{BalanceKind, ExtractedLine};

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedExtraction {
    pub lines: Vec<ExtractedLine>,
    pub assertions: Vec<ExtractedAssertion>,
    pub end_of_day_balances: Vec<ExtractedAssertion>,
    pub excluded: Vec<ExcludedRecords>,
    pub unsettled_date_order: Option<DateOrder>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedAssertion {
    pub kind: BalanceKind,
    pub account_ref: String,
    pub asset_ref: String,
    pub as_of_date: Date,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExcludedRecords {
    pub refs: Vec<usize>,
    pub reason: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    DayFirst,
    MonthFirst,
}
