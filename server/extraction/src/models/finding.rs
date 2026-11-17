use rust_decimal::Decimal;
use serde::Serialize;
use time::Date;

use super::output::{iso_date, BalanceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemRef {
    Line(usize),
    Balance(usize),
    ControlTotal(usize),
    Skipped(usize),
    Period,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BalancePosition {
    pub balance: Option<usize>,
    pub kind: BalanceKind,
    #[serde(with = "iso_date")]
    pub as_of_date: Date,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum Finding {
    InvalidOutput {
        message: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        refs: Vec<usize>,
    },
    InvalidItem {
        item: ItemRef,
        message: String,
    },
    UncoveredRecords {
        refs: Vec<usize>,
    },
    RecordClaimedTwice {
        record: usize,
        items: Vec<ItemRef>,
    },
    SuspectSkipped {
        refs: Vec<usize>,
    },
    NothingExtracted {
        refs: Vec<usize>,
    },
    BalancesSkipped {
        account_ref: String,
        asset_ref: String,
        refs: Vec<usize>,
        lines_net: Decimal,
    },
    AmountNotFound {
        item: ItemRef,
        amount_text: String,
        quantity: Decimal,
    },
    DateNotFound {
        item: ItemRef,
        date_text: String,
        #[serde(with = "iso_date")]
        date: Date,
    },
    TimeNotFound {
        item: ItemRef,
        time: String,
    },
    ExternalIdNotFound {
        item: ItemRef,
        external_id: String,
    },
    BalanceChainBroken {
        account_ref: String,
        asset_ref: String,
        line: usize,
        expected: Decimal,
        printed: Decimal,
    },
    BalancesDisagree {
        account_ref: String,
        asset_ref: String,
        earlier: BalancePosition,
        later: BalancePosition,
        expected: Decimal,
    },
    BalanceColumnIgnored {
        field: usize,
        refs: Vec<usize>,
    },
    ControlTotalMismatch {
        item: ItemRef,
        computed: Decimal,
        blocking: bool,
    },
}

impl Finding {
    pub fn is_blocking(&self) -> bool {
        !matches!(
            self,
            Finding::ControlTotalMismatch {
                blocking: false,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractionReport {
    pub findings: Vec<Finding>,
}
