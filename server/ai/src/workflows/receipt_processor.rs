//! Receipt-processing workflow. A thin facade over `Conversation::prompt`:
//! configures the rig agent with receipt-specific prompt + tools, calls the
//! wrapper, then parses the agent's final assistant message into a
//! `Proposal`. All conversation persistence and file fetching is handled
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

const SYSTEM_PROMPT: &str = r#"You are a receipt processing assistant. You get one receipt — a photo, a scan or a PDF — and turn it into transactions the user will review.

Read from the receipt:
1. The merchant name
2. The date of the purchase
3. Every line item and its price
4. The total paid, the currency, and any card ending, account number or IBAN

Look things up with the tools, all in your first turn:
- list_accounts: the account the money was spent from. If the receipt shows a card ending, account number or IBAN, pick the account whose `identifiers` contains it (card_last4 = last 4 digits; account_number/iban = exact).
- search_categories with no query: the full list of categories to choose from.
- search_assets: the currency, by the code or name printed on the receipt (e.g. "EUR") with asset_type "currency".

Answer with a transaction_group that has one transaction per line item:
- Each transaction's description is the item's name as printed on the receipt, in the receipt's language: don't translate or reword it. A line with a quantity, such as "2 x Milk", is one transaction for the line total.
- Give every line its own category: the most specific one that fits that item, not the shop. Cleaning products, toiletries, medicine, alcohol, pet food and clothes each belong in their own categories even on a grocery receipt. Pick from the categories that exist; don't use one category for every line just because the shop sells food.
- A discount printed against an item reduces that item's amount. A receipt-wide discount, coupon, deposit, bag charge or service fee becomes its own transaction.
- The amounts must add up exactly to the total paid. If they don't, read the receipt again.
- The group's description is the merchant name, and its category is the one that fits the shop as a whole.

Answer with a single transaction only when the receipt has one line item, or when no individual items can be read; then use the total.

Each transaction has exactly ONE entry: the account the money was spent from, the amount, and the currency asset. Spending amounts are negative because money went out (e.g. "-3.49"); a discount or refund line is positive. The system handles the accounting automatically.

If you cannot determine a field, set it to null.
"#;

const INITIAL_USER_PROMPT: &str =
    "Please analyze this receipt and extract the transaction details.";

/// Initial extraction. Records the user prompt with the receipt attachment
/// and runs the agent. The wrapper handles persistence and file resolution.
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
/// the receipt file) and runs the agent with the correction text.
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
    source_file_id: Uuid,
) -> anyhow::Result<ReceiptProcessorOutput>
where
    D: AiDataProvider,
    C: ConversationProvider,
    R: RateLimitProvider,
{
    let agent = build_agent(&config, data);
    let conv = Conversation::new(conversation, rate_limit);
    // A run that failed before the model was called, or whose answer was rejected, has no turn to
    // continue, so the retry asks again with the receipt attached.
    let turn = if conv.can_continue().await? {
        ChatTurn::Continuation
    } else {
        ChatTurn::Message {
            message: INITIAL_USER_PROMPT.to_string(),
            file_ids: vec![source_file_id],
        }
    };
    let output = conv
        .run_structured(agent, turn, Persistence::Persist)
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
