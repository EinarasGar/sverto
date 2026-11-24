use std::sync::Arc;

use super::{ToolError, ToolMode};
use crate::data_provider::AiDataProvider;
use crate::embedding::{embed_query, EmbeddingModel};
use crate::models::reference::AssetSearchParams;
use crate::models::tool_output::SearchAssetsArgs;
use rig::tool::{Tool, ToolContext, ToolExecutionError};
use serde_json::{json, Value};

const MAX_MATCHES: u64 = 15;
const MAX_SIMILAR: u64 = 5;
const CODE_MODE_MAX_MATCHES: u64 = 100;

pub struct SearchAssetsTool<D: AiDataProvider> {
    data: Arc<D>,
    embedding_model: EmbeddingModel,
    mode: ToolMode,
}

impl<D: AiDataProvider> SearchAssetsTool<D> {
    pub fn new(data: Arc<D>, embedding_model: EmbeddingModel) -> Self {
        Self::with_mode(data, embedding_model, ToolMode::Normal)
    }

    pub fn with_mode(data: Arc<D>, embedding_model: EmbeddingModel, mode: ToolMode) -> Self {
        Self {
            data,
            embedding_model,
            mode,
        }
    }
}

impl<D: AiDataProvider> Tool for SearchAssetsTool<D> {
    const NAME: &'static str = "search_assets";

    type Error = ToolError;
    type Args = SearchAssetsArgs;
    type Output = Value;

    fn description(&self) -> String {
        match self.mode {
            ToolMode::Normal => "Find asset IDs (currencies, stocks, ETFs, funds, bonds, ETCs, crypto) by ticker or name, e.g. 'EUR', 'Apple', 'VWRL'. Put the kind of asset in asset_type instead of adding words like 'currency' to the query. Returns `matches` (text matches, best first; assets the user holds rank higher) and `similar` (closest by meaning among currencies, crypto and the user's own assets; this catches typos and other names such as 'British pound', but may be unrelated). Each row: {id, asset_name, ticker, asset_type, held}, where held means the user currently holds the asset.",
            ToolMode::CodeMode => "Assets whose ticker or name matches the query, as a flat array ranked best first (assets the user holds rank higher). args {query, asset_type? (currency|stock|bond|mutual_fund|etf|etc|crypto|real_estate|vehicle)}. Each row: {id, asset_name, ticker, asset_type, held}.",
        }
        .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Ticker or name, e.g. 'EUR', 'Euro', 'AAPL', 'S&P 500'."
                },
                "asset_type": {
                    "type": "string",
                    "enum": ["currency", "stock", "bond", "mutual_fund", "etf", "etc", "crypto", "real_estate", "vehicle"],
                    "description": "Optional. Only return assets of this type."
                }
            },
            "required": ["query"]
        })
    }

    fn map_error(&self, error: Self::Error) -> ToolExecutionError {
        error.into()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(tool = Self::NAME))]
    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> std::result::Result<Self::Output, Self::Error> {
        let (query_vector, max_matches) = match self.mode {
            ToolMode::CodeMode => (None, CODE_MODE_MAX_MATCHES),
            ToolMode::Normal => (
                Some(
                    embed_query(&self.embedding_model, &args.query)
                        .await
                        .map_err(|e| ToolError(e.to_string()))?,
                ),
                MAX_MATCHES,
            ),
        };

        let result = self
            .data
            .search_assets(AssetSearchParams {
                query: args.query,
                asset_type: args.asset_type,
                query_vector,
                max_matches,
                max_similar: MAX_SIMILAR,
            })
            .await
            .map_err(|e| ToolError(e.to_string()))?;

        match self.mode {
            ToolMode::CodeMode => serde_json::to_value(&result.matches).map_err(Into::into),
            ToolMode::Normal => serde_json::to_value(&result).map_err(Into::into),
        }
    }
}
