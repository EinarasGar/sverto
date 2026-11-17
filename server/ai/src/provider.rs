use rig::agent::AgentBuilder;
use rig::completion::Effort;
use rig::providers::anthropic::Anthropic;
use rig::providers::gemini::extension::GeminiOptions;
use rig::providers::gemini::Gemini;
use rig::typesafeai::JevConfig;

use crate::config::{AiConfig, SystemOneConfig};
use crate::embedding::{EmbeddingModel, EMBEDDING_DIMS};
use crate::models::provider::{CompletionProvider, EmbeddingProvider, SystemOneProvider};

pub type CompletionModel = rig::DynModel<rig::operation::Completion>;
pub type SystemOneModel = rig::driver::Model<JevConfig>;

pub fn completion_model(config: &AiConfig) -> CompletionModel {
    match config.provider {
        CompletionProvider::Gemini => Gemini::new(config.api_key.as_str())
            .completion(&config.model)
            .into(),
        CompletionProvider::Anthropic => Anthropic::new(config.api_key.as_str())
            .completion(&config.model)
            .into(),
    }
}

pub fn system_one_model(config: &SystemOneConfig) -> SystemOneModel {
    match config.provider {
        SystemOneProvider::TypeSafe => JevConfig::new(config.api_key.as_str())
            .model(config.model.as_str())
            .client()
            .evaluation(),
    }
}

pub fn embedding_model(config: &AiConfig) -> EmbeddingModel {
    match config.embedding_provider {
        EmbeddingProvider::Gemini => Gemini::new(config.embedding_api_key.as_str())
            .embedding(&config.embedding_model, Some(EMBEDDING_DIMS))
            .erase(),
    }
}

pub fn with_reasoning(builder: AgentBuilder, config: &AiConfig, effort: Effort) -> AgentBuilder {
    let builder = builder.reasoning(effort);
    match config.provider {
        CompletionProvider::Gemini => {
            builder.provider_option(GeminiOptions::new().include_thoughts(true))
        }
        CompletionProvider::Anthropic => builder,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rig::message::{AssistantContent, Message};
    use serde_json::{json, Value};

    use super::*;
    use crate::conversation::Conversation;
    use crate::models::agent::ModelAgent;
    use crate::models::chat::{ChatTurn, Persistence};
    use crate::test_support::{FakeModel, MemoryConversation, NoLimit, Reply, MODEL};
    use crate::tools::run_script::RunScriptTool;

    const CLAUDE: &str = "claude-sonnet-5-5";

    fn config(provider: CompletionProvider, model: &str) -> AiConfig {
        AiConfig {
            provider,
            api_key: "test-key".into(),
            model: model.into(),
            embedding_provider: EmbeddingProvider::Gemini,
            embedding_api_key: "test-key".into(),
            embedding_model: "gemini-embedding".into(),
        }
    }

    fn message(text: &str) -> ChatTurn {
        ChatTurn::Message {
            message: text.into(),
            file_ids: Vec::new(),
        }
    }

    #[tokio::test]
    async fn gemini_reasons_at_medium_effort_and_streams_its_thought_summaries() {
        let model = FakeModel::start(vec![Reply::Json(json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": "hi"}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 1, "candidatesTokenCount": 1, "totalTokenCount": 2}
        }))])
        .await;
        let builder = AgentBuilder::new(model.gemini().completion(MODEL));
        let agent = with_reasoning(
            builder,
            &config(CompletionProvider::Gemini, MODEL),
            Effort::Medium,
        )
        .build();

        agent.prompt(Message::user("hello")).await.unwrap();

        assert_eq!(
            model.requests()[0]["generationConfig"]["thinkingConfig"],
            json!({"thinkingLevel": "medium", "includeThoughts": true})
        );
    }

    fn claude_reply(content: Value, stop_reason: &str) -> Reply {
        Reply::Json(json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": CLAUDE,
            "content": content, "stop_reason": stop_reason, "stop_sequence": null,
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }))
    }

    fn claude_agent(model: &FakeModel) -> ModelAgent {
        let client = rig::providers::anthropic::AnthropicConfig::new("test-key")
            .with_base_url(&model.url)
            .client();
        let builder = AgentBuilder::new(client.completion(CLAUDE))
            .default_max_turns(5)
            .tool(RunScriptTool::new(rig::tool::ToolSet::default()));
        ModelAgent {
            agent: with_reasoning(
                builder,
                &config(CompletionProvider::Anthropic, CLAUDE),
                Effort::Medium,
            )
            .build(),
            model: CLAUDE.into(),
        }
    }

    fn thinking_blocks(request: &Value) -> Vec<Value> {
        request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["role"] == "assistant")
            .flat_map(|message| message["content"].as_array().cloned().unwrap_or_default())
            .filter(|block| block["type"] == "thinking")
            .collect()
    }

    #[tokio::test]
    async fn claude_gets_its_signed_thinking_back_from_the_saved_history() {
        let model = FakeModel::start(vec![
            claude_reply(
                json!([
                    {"type": "thinking", "thinking": "Add them.", "signature": "c2lnLTE="},
                    {"type": "tool_use", "id": "toolu_1", "name": "run_script", "input": {"script": "1 + 1"}}
                ]),
                "tool_use",
            ),
            claude_reply(json!([{"type": "text", "text": "It is 2."}]), "end_turn"),
            claude_reply(json!([{"type": "text", "text": "You're welcome."}]), "end_turn"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let wrapper = Conversation::new(conversation.clone(), Arc::new(NoLimit));

        wrapper
            .run(
                claude_agent(&model),
                message("what is 1 + 1"),
                Persistence::Persist,
            )
            .await
            .unwrap();
        wrapper
            .run(
                claude_agent(&model),
                message("thanks"),
                Persistence::Persist,
            )
            .await
            .unwrap();

        let requests = model.requests();
        assert!(requests[0]["max_tokens"].as_u64().is_some());
        assert_eq!(requests[0]["output_config"]["effort"], "medium");
        assert_eq!(
            thinking_blocks(&requests[2]),
            vec![json!({"type": "thinking", "thinking": "Add them.", "signature": "c2lnLTE="})]
        );
        let saved = crate::history::load(&*conversation).await.unwrap();
        assert!(saved.messages().iter().any(|message| matches!(&**message,
            Message::Assistant(reply) if reply.content.iter().any(|content| matches!(content, AssistantContent::Reasoning(_))))));
    }
}
