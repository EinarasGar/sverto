use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub struct ReceiptProcessorOutput {
    pub proposal_type: String,
    pub proposal: serde_json::Value,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ReceiptAnswer {
    pub proposal_type: ProposalKind,
    pub proposal: ProposalFields,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProposalKind {
    Transaction,
    TransactionGroup,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct ProposalFields {
    pub description: Option<String>,
    #[schemars(description = "YYYY-MM-DD")]
    pub date: Option<String>,
    pub category_id: Option<i32>,
    #[schemars(
        description = "For a single transaction: the id of the account the money was spent from."
    )]
    pub account_id: Option<String>,
    #[schemars(
        description = "For a single transaction: negative for money going out, e.g. \"-73.59\"."
    )]
    pub amount: Option<String>,
    #[schemars(description = "For a single transaction: the id of the currency asset.")]
    pub asset_id: Option<i32>,
    #[schemars(description = "For a transaction group: one transaction per item.")]
    pub transactions: Option<Vec<ProposedTransaction>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct ProposedTransaction {
    pub description: Option<String>,
    #[schemars(description = "YYYY-MM-DD")]
    pub date: Option<String>,
    #[schemars(description = "The id of the account the money was spent from.")]
    pub account_id: Option<String>,
    #[schemars(description = "Negative for money going out, e.g. \"-73.59\".")]
    pub amount: Option<String>,
    #[schemars(description = "The id of the currency asset.")]
    pub asset_id: Option<i32>,
    pub category_id: Option<i32>,
}

#[derive(Serialize)]
pub(crate) struct ProposedTransactionGroup {
    pub description: Option<String>,
    pub date: Option<String>,
    pub category_id: Option<i32>,
    pub transactions: Vec<ProposedTransaction>,
}
