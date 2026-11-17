//! Receipt-processing workflow. A thin facade over `Conversation::prompt`:
//! configures the rig agent with receipt-specific prompt + tools, calls the
//! wrapper, then parses the agent's final assistant message into a
//! `Proposal`. All conversation persistence and image fetching is handled
//! by the wrapper — this file is only the workflow's unique parts.

use std::sync::Arc;

use crate::config::AiConfig;
use crate::conversation::Conversation;
use crate::conversation_provider::ConversationProvider;
use crate::data_provider::AiDataProvider;
use crate::models::agent::StructuredAgent;
use crate::models::chat::{ChatTurn, Persistence};
pub use crate::models::receipt::ReceiptProcessorOutput;
use crate::models::receipt::{
    ProposalKind, ProposedTransaction, ProposedTransactionGroup, ReceiptAnswer,
};
use crate::provider::{completion_model, embedding_model};
use crate::rate_limit_provider::RateLimitProvider;
use crate::tools::list_accounts::ListAccountsTool;
use crate::tools::search_assets::SearchAssetsTool;
use crate::tools::search_categories::SearchCategoriesTool;
use rig::agent::AgentBuilder;
use rig::completion::CacheRetention;
use uuid::Uuid;

const SYSTEM_PROMPT: &str = r#"You are a receipt processing assistant. Your job is to extract transaction details from receipt images.

Analyze the receipt image and extract:
1. The merchant/store name (use as description)
2. The date of the transaction
3. The total amount
4. Individual line items (if visible and distinct)

Use the available tools to:
- Search for matching accounts (the account the money was spent from)
- Search for appropriate categories for the transaction
- Search for the correct currency/asset
- If the receipt shows a card ending, account number, or IBAN, match it to the account whose `identifiers` contains it (card_last4 = last 4 digits; account_number/iban = exact).

After gathering information, give your answer.

If the receipt has multiple distinct items with different categories, output a transaction_group.
If it's a single purchase or all items share the same category, output a single transaction.

Each transaction has exactly ONE entry — the account the money was spent from, the amount, and the asset (currency). The amount must be negative to represent cash going out (e.g. "-73.59" for a purchase). The system handles the accounting automatically.

If you cannot determine a field, set it to null.
"#;

const INITIAL_USER_PROMPT: &str =
    "Please analyze this receipt and extract the transaction details.";

/// Initial extraction. Records the user prompt with the receipt attachment
/// and runs the agent. The wrapper handles persistence and image resolution.
#[tracing::instrument(level = "debug", skip_all, fields(file_id = %source_file_id))]
pub async fn process<D, C, R>(
    config: AiConfig,
    data: Arc<D>,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    source_file_id: Uuid,
) -> anyhow::Result<ReceiptProcessorOutput>
where
    D: AiDataProvider,
    C: ConversationProvider,
    R: RateLimitProvider,
{
    run_with_prompt(
        config,
        data,
        conversation,
        rate_limit,
        INITIAL_USER_PROMPT.to_string(),
        vec![source_file_id],
    )
    .await
}

/// Apply a user correction. The wrapper loads the prior history (including
/// the receipt image) and runs the agent with the correction text.
#[tracing::instrument(level = "debug", skip_all)]
pub async fn correct<D, C, R>(
    config: AiConfig,
    data: Arc<D>,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    correction: String,
) -> anyhow::Result<ReceiptProcessorOutput>
where
    D: AiDataProvider,
    C: ConversationProvider,
    R: RateLimitProvider,
{
    run_with_prompt(config, data, conversation, rate_limit, correction, vec![]).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn retry<D, C, R>(
    config: AiConfig,
    data: Arc<D>,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
) -> anyhow::Result<ReceiptProcessorOutput>
where
    D: AiDataProvider,
    C: ConversationProvider,
    R: RateLimitProvider,
{
    let agent = build_agent(&config, data);
    let conv = Conversation::new(conversation, rate_limit);
    let output = conv
        .run_structured(agent, ChatTurn::Continuation, Persistence::Persist)
        .await?;

    proposal_output(output.value)
}

#[tracing::instrument(level = "debug", skip_all, fields(count = file_ids.len()))]
async fn run_with_prompt<D, C, R>(
    config: AiConfig,
    data: Arc<D>,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    message: String,
    file_ids: Vec<Uuid>,
) -> anyhow::Result<ReceiptProcessorOutput>
where
    D: AiDataProvider,
    C: ConversationProvider,
    R: RateLimitProvider,
{
    let agent = build_agent(&config, data);
    let conv = Conversation::new(conversation, rate_limit);
    let output = conv
        .run_structured(
            agent,
            ChatTurn::Message { message, file_ids },
            Persistence::Persist,
        )
        .await?;

    proposal_output(output.value)
}

fn build_agent<D>(config: &AiConfig, data: Arc<D>) -> StructuredAgent<ReceiptAnswer>
where
    D: AiDataProvider,
{
    let embedding_model = embedding_model(config);

    let builder = AgentBuilder::new(completion_model(config))
        .preamble(SYSTEM_PROMPT)
        .default_max_turns(5)
        .cache(CacheRetention::Short)
        .tool(ListAccountsTool::new(data.clone()))
        .tool(SearchCategoriesTool::new(
            data.clone(),
            embedding_model.clone(),
        ))
        .tool(SearchAssetsTool::new(data.clone(), embedding_model.clone()));
    StructuredAgent::new(builder, config.model.clone())
}

fn proposal_output(answer: ReceiptAnswer) -> anyhow::Result<ReceiptProcessorOutput> {
    let ReceiptAnswer {
        proposal_type,
        proposal,
    } = answer;
    let body = match proposal_type {
        ProposalKind::Transaction => serde_json::to_value(ProposedTransaction {
            description: proposal.description,
            date: proposal.date,
            account_id: proposal.account_id,
            amount: proposal.amount,
            asset_id: proposal.asset_id,
            category_id: proposal.category_id,
        })?,
        ProposalKind::TransactionGroup => serde_json::to_value(ProposedTransactionGroup {
            description: proposal.description,
            date: proposal.date,
            category_id: proposal.category_id,
            transactions: proposal.transactions.unwrap_or_default(),
        })?,
    };
    let proposal_type = serde_json::to_value(proposal_type)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("A proposal type serializes as a string"))?;
    Ok(ReceiptProcessorOutput {
        proposal_type,
        proposal: body,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_answer_schema_is_plain_required_properties() {
        let schema = serde_json::to_value(schemars::schema_for!(ReceiptAnswer)).unwrap();

        assert_eq!(schema["type"], json!("object"));
        assert_eq!(schema["required"], json!(["proposal_type", "proposal"]));
        assert!(schema.get("oneOf").is_none());
    }
}
