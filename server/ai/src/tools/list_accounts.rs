use std::sync::Arc;

use super::{ToolError, ToolMode};
use crate::data_provider::AiDataProvider;
use crate::models::tool_output::ListAccountsArgs;
use rig::tool::{Tool, ToolContext, ToolExecutionError};
use serde_json::{json, Value};

pub struct ListAccountsTool<D: AiDataProvider> {
    data: Arc<D>,
}

impl<D: AiDataProvider> ListAccountsTool<D> {
    pub fn new(data: Arc<D>) -> Self {
        Self { data }
    }

    pub fn with_mode(data: Arc<D>, _mode: ToolMode) -> Self {
        Self { data }
    }
}

impl<D: AiDataProvider> Tool for ListAccountsTool<D> {
    const NAME: &'static str = "list_accounts";

    type Error = ToolError;
    type Args = ListAccountsArgs;
    type Output = Value;

    fn description(&self) -> String {
        "List the user's accounts. Returns each account's name, type, liquidity type, ownership share (the user's fraction of the account, 1 = sole owner), and identifiers (typed: card_last4, account_number, iban). Each row: {account_id, account_name, account_type, liquidity_type, ownership_share (number), identifiers: [{kind, value}]}.".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn map_error(&self, error: Self::Error) -> ToolExecutionError {
        error.into()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(tool = Self::NAME))]
    async fn call(
        &self,
        _context: &mut ToolContext,
        _args: Self::Args,
    ) -> std::result::Result<Self::Output, Self::Error> {
        let accounts = self
            .data
            .list_accounts()
            .await
            .map_err(|e| ToolError(e.to_string()))?;

        serde_json::to_value(&accounts).map_err(Into::into)
    }
}
