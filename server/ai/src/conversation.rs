use std::collections::HashMap;
use std::sync::Arc;

use futures::{Stream, StreamExt};
use rig::agent::{AgentHook, MultiTurnStreamItem};
use rig::completion::{PromptError, UsageTotals};
use rig::conversation::{pairing, Conversation as History};
use rig::message::{Message, ToolCall, UserContent};
use rig::streaming::{Item, StreamEvent};
use rig::tool::ToolContext;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::action_provider::AiActionProvider;
use crate::agents::approval::{build_gated_toolset, ApprovalHook, GatedToolSet};
use crate::agents::usage::UsageHook;
use crate::conversation_provider::ConversationProvider;
use crate::history::{self, render_tool_result, CommitHook};
use crate::models::agent::{ModelAgent, StructuredAgent};
use crate::models::chat::{
    Base64Attachment, ChatStreamEvent, ChatTurn, Persistence, PromptOutput, StructuredOutput,
    TokenUsage,
};
use crate::models::error::AiError;
use crate::rate_limit_provider::RateLimitProvider;

const DECLINED: &str = "User declined this action.";
const SUPERSEDED: &str = "The user sent a new message instead of answering this request.";

pub struct Conversation<C, R>
where
    C: ConversationProvider,
    R: RateLimitProvider,
{
    conversation: Arc<C>,
    rate_limit: Arc<R>,
}

impl<C, R> Conversation<C, R>
where
    C: ConversationProvider,
    R: RateLimitProvider,
{
    pub fn new(conversation: Arc<C>, rate_limit: Arc<R>) -> Self {
        Self {
            conversation,
            rate_limit,
        }
    }

    pub async fn can_continue(&self) -> Result<bool, AiError> {
        let history = history::load(&*self.conversation).await?;
        Ok(history.pending_calls().is_empty() && history.awaits_model())
    }

    pub async fn run_structured<T: DeserializeOwned>(
        &self,
        agent: StructuredAgent<T>,
        turn: ChatTurn,
        persistence: Persistence,
    ) -> Result<StructuredOutput<T>, AiError> {
        let output = self.run(agent.agent, turn, persistence).await?;
        let value =
            serde_json::from_str(output.output.trim()).map_err(|error| AiError::InvalidAnswer {
                detail: error.to_string(),
            })?;
        Ok(StructuredOutput {
            value,
            usage: output.usage,
        })
    }

    pub async fn run(
        &self,
        agent: ModelAgent,
        turn: ChatTurn,
        persistence: Persistence,
    ) -> Result<PromptOutput, AiError> {
        self.run_with_hook(agent, turn, persistence, ()).await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(otel.kind = "client"))]
    pub async fn run_with_hook<H: AgentHook + 'static>(
        &self,
        agent: ModelAgent,
        turn: ChatTurn,
        persistence: Persistence,
        hook: H,
    ) -> Result<PromptOutput, AiError> {
        let persist = persistence == Persistence::Persist;
        let mut history = history::load(&*self.conversation).await?;
        let files = match turn {
            ChatTurn::Message { message, file_ids } => {
                self.add_message(&mut history, message, &file_ids).await?
            }
            ChatTurn::Continuation => self.files(&history, &[]).await?,
            ChatTurn::Approval { .. } => {
                return Err(AiError::Fatal {
                    detail: "Approval turns require the streaming API".to_string(),
                })
            }
        };
        let request = self.prepare(&agent, &mut history, &files, persist).await?;
        let (prompt, earlier) = request
            .split_last()
            .expect("a prepared request has a prompt");

        let commit = persist
            .then(|| CommitHook::new(self.conversation.clone(), history, earlier.len(), None));
        let usage = UsageHook::default();
        let mut runner = agent
            .agent
            .prompt(prompt.clone())
            .history(earlier.to_vec())
            .add_hook(usage.clone());
        if let Some(commit) = &commit {
            runner = runner.add_hook(commit.clone());
        }
        let result = runner.add_hook(hook).await;

        let spent = usage.totals();
        charge(&*self.rate_limit, &spent).await;
        let failure = commit.as_ref().and_then(CommitHook::take_failure);
        let output = match (result, failure) {
            (_, Some(failure)) => {
                self.record_turn_error(&failure).await;
                return Err(failure);
            }
            (Ok(response), None) => response.output(),
            (Err(PromptError::Cancelled { reason, .. }), None) => reason,
            (Err(error), None) => {
                let error = AiError::from(error);
                if persist {
                    self.record_turn_error(&error).await;
                }
                return Err(error);
            }
        };
        Ok(PromptOutput {
            output,
            usage: spent.tokens.into(),
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(otel.kind = "client"))]
    pub async fn stream<A>(
        &self,
        agent: ModelAgent,
        actions: Arc<A>,
        turn: ChatTurn,
    ) -> Result<impl Stream<Item = ChatStreamEvent>, AiError>
    where
        A: AiActionProvider,
    {
        let mut history = history::load(&*self.conversation).await?;
        let gated = build_gated_toolset(actions);
        let mut opening = Vec::new();
        let files = match turn {
            ChatTurn::Message { message, file_ids } => {
                self.add_message(&mut history, message, &file_ids).await?
            }
            ChatTurn::Approval { approvals } => {
                let (events, decided) = self.decide(&mut history, &gated, approvals).await?;
                opening = events;
                if !decided {
                    return Ok(futures::stream::iter(opening).left_stream());
                }
                self.files(&history, &[]).await?
            }
            ChatTurn::Continuation => self.files(&history, &[]).await?,
        };

        let still_pending = history.pending_calls();
        if !still_pending.is_empty() {
            opening.extend(still_pending.iter().map(tool_request));
            return Ok(futures::stream::iter(opening).left_stream());
        }

        let request = self.prepare(&agent, &mut history, &files, true).await?;
        let (prompt, earlier) = request
            .split_last()
            .expect("a prepared request has a prompt");

        let approvals = ApprovalHook::new(gated.gated_names);
        let commit = CommitHook::new(
            self.conversation.clone(),
            history,
            earlier.len(),
            Some(approvals.clone()),
        );
        let usage = UsageHook::default();
        let mut rig_stream = agent
            .agent
            .prompt(prompt.clone())
            .history(earlier.to_vec())
            .add_hook(usage.clone())
            .add_hook(commit.clone())
            .add_hook(approvals.clone())
            .max_turns(8)
            .stream();

        let conversation = self.conversation.clone();
        let rate_limit = self.rate_limit.clone();

        Ok(async_stream::stream! {
            for event in opening { yield event; }

            let mut thinking_span: Option<tracing::Span> = None;
            let mut completed = false;
            let mut error_yielded = false;
            let mut paused_for_approval = false;

            while let Some(item) = rig_stream.next().await {
                match item {
                    Ok(MultiTurnStreamItem::StreamAssistantItem(Item::Event(StreamEvent::Text { text, .. }))) => {
                        let _ = thinking_span.take();
                        yield ChatStreamEvent::Text(text);
                    }
                    Ok(MultiTurnStreamItem::ToolCall { tool_call }) => {
                        let _ = thinking_span.take();
                        yield ChatStreamEvent::ToolCall {
                            call_id: tool_call.id.to_string(),
                            name: tool_call.function.name.to_string(),
                            input: tool_call.function.arguments_value(),
                        };
                    }
                    Ok(MultiTurnStreamItem::ToolResult { tool_result }) if approvals.is_pending(&tool_result.call.to_string()) => {}
                    Ok(MultiTurnStreamItem::ToolResult { tool_result }) => {
                        yield ChatStreamEvent::ToolResult {
                            name: tool_result.name.to_string(),
                            output: render_tool_result(&tool_result),
                        };
                    }
                    Ok(MultiTurnStreamItem::StreamAssistantItem(Item::Event(StreamEvent::Reasoning { text, .. }))) => {
                        if thinking_span.is_none() {
                            thinking_span = Some(tracing::debug_span!("thinking"));
                        }
                        if !text.is_empty() {
                            yield ChatStreamEvent::Reasoning(text);
                        }
                    }
                    Ok(MultiTurnStreamItem::FinalResponse(_)) => {
                        let spent = TokenUsage::from(usage.totals().tokens);
                        tracing::debug!(
                            input_tokens = spent.input,
                            cached_input_tokens = spent.cached_input,
                            output_tokens = spent.output,
                            "chat stream completed"
                        );
                        completed = true;
                        yield ChatStreamEvent::Usage {
                            input_tokens: spent.input,
                            output_tokens: spent.output,
                        };
                    }
                    Ok(_) => {}
                    Err(PromptError::Cancelled { .. } | PromptError::MaxTurns { .. }) if !commit.has_failed() && approvals.has_pending() => {
                        paused_for_approval = true;
                        break;
                    }
                    Err(e) => {
                        let err = commit.take_failure().unwrap_or_else(|| AiError::from(e));
                        record_turn_error(&*conversation, &err).await;
                        error_yielded = true;
                        yield ChatStreamEvent::Error(err);
                        break;
                    }
                }
            }

            if let Some(err) = commit.take_failure() {
                record_turn_error(&*conversation, &err).await;
                error_yielded = true;
                yield ChatStreamEvent::Error(err);
            }

            charge(&*rate_limit, &usage.totals()).await;

            let tool_requests = approvals.take_pending();
            let tool_requests = if error_yielded { Vec::new() } else { tool_requests };

            if !completed && !error_yielded && !paused_for_approval && tool_requests.is_empty() {
                tracing::error!(
                    error.type = "stream_ended_without_completion",
                    "AI stream ended without final response, error, or pending approval"
                );
                let err = AiError::ProviderUnavailable {
                    detail: "The model stream ended unexpectedly without completing the turn".to_string(),
                };
                record_turn_error(&*conversation, &err).await;
                yield ChatStreamEvent::Error(err);
            }

            for payload in tool_requests {
                match serde_json::from_str(&payload.args) {
                    Ok(args) => {
                        yield ChatStreamEvent::ToolRequest {
                            tool_call_id: payload.tool_call_id,
                            name: payload.name,
                            args,
                        };
                    }
                    Err(e) => {
                        yield ChatStreamEvent::Error(AiError::unknown(format!(
                            "Failed to parse tool arguments for {}: {e}", payload.name
                        )));
                    }
                }
            }
        }
        .right_stream())
    }

    async fn add_message(
        &self,
        history: &mut History,
        message: String,
        file_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Base64Attachment>, AiError> {
        let files = self.files(history, file_ids).await?;
        let passed_over = history.pending_calls();
        if !passed_over.is_empty() {
            history.push(pairing::close_pending_with(&passed_over, SUPERSEDED));
        }
        history.push(history::user_message(message, file_ids, &files)?);
        Ok(files)
    }

    async fn decide(
        &self,
        history: &mut History,
        gated: &GatedToolSet,
        approvals: Vec<(String, bool)>,
    ) -> Result<(Vec<ChatStreamEvent>, bool), AiError> {
        let pending = history.pending_calls();
        let already_decided = history::decided_calls(history);
        let after = history
            .messages()
            .len()
            .checked_sub(1)
            .and_then(|last| history.id(last));
        let mut events = Vec::new();
        let mut decided: Vec<(&ToolCall, bool)> = Vec::new();

        for (tool_call_id, approved) in approvals {
            let call = pending.iter().find(|call| {
                call.id.to_string() == tool_call_id
                    && !already_decided.contains(&tool_call_id)
                    && !decided.iter().any(|(chosen, _)| chosen.id == call.id)
            });
            let Some(call) = call else {
                events.push(ChatStreamEvent::Error(AiError::unknown(format!(
                    "No pending tool call found for approval: {tool_call_id}"
                ))));
                continue;
            };
            history.add_aside(history::tool_approval(tool_call_id, approved, after));
            decided.push((call, approved));
        }
        if decided.is_empty() {
            return Ok((events, false));
        }
        history::save(&*self.conversation, history).await?;

        let mut results: Vec<UserContent> = Vec::new();
        for (call, approved) in decided {
            if approved {
                let result = gated
                    .toolset
                    .execute(
                        call.function.name.as_ref(),
                        call.function.raw_arguments(),
                        &mut ToolContext::new(),
                    )
                    .await;
                let result = call.answer(&result);
                events.push(ChatStreamEvent::ToolResult {
                    name: result.name.to_string(),
                    output: render_tool_result(&result),
                });
                results.push(UserContent::ToolResult(result));
            } else if let Message::User { content } = pairing::close_pending_with([call], DECLINED)
            {
                results.extend(content);
            }
        }
        history.push(Message::User { content: results });
        history::save(&*self.conversation, history).await?;
        Ok((events, true))
    }

    async fn files(
        &self,
        history: &History,
        new_files: &[Uuid],
    ) -> Result<HashMap<Uuid, Base64Attachment>, AiError> {
        let mut ids = history::referenced_files(history.messages());
        ids.extend(
            new_files
                .iter()
                .filter(|id| !ids.contains(id))
                .copied()
                .collect::<Vec<_>>(),
        );
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let attachments = self
            .conversation
            .fetch_attachments(&ids)
            .await
            .map_err(|e| AiError::InvalidAttachment {
                detail: format!("{e:#}"),
            })?;
        Ok(ids.into_iter().zip(attachments).collect())
    }

    async fn prepare(
        &self,
        agent: &ModelAgent,
        history: &mut History,
        files: &HashMap<Uuid, Base64Attachment>,
        persist: bool,
    ) -> Result<Vec<Message>, AiError> {
        if !history.awaits_model() {
            return Err(AiError::Fatal {
                detail: "Conversation has no interrupted turn to continue".to_string(),
            });
        }
        let request = history::resolve(history.messages(), files);
        self.rate_limit
            .pre_check(request_tokens(agent, &request).await)
            .await?;
        if persist {
            if let Err(error) = history::save(&*self.conversation, history).await {
                self.rate_limit.release().await;
                return Err(error);
            }
            if let Err(e) = self.conversation.clear_turn_error().await {
                tracing::warn!(
                    error = ?e,
                    error.type = "clear_turn_error",
                    "failed to clear turn error marker"
                );
            }
        }
        Ok(request)
    }

    async fn record_turn_error(&self, error: &AiError) {
        record_turn_error(&*self.conversation, error).await;
    }
}

async fn charge<R: RateLimitProvider>(rate_limit: &R, spent: &UsageTotals) {
    if spent.calls == 0 {
        rate_limit.release().await;
    } else {
        let spent = TokenUsage::from(spent.tokens);
        rate_limit.record_usage(spent.input, spent.output).await;
    }
}

async fn record_turn_error<C: ConversationProvider>(conversation: &C, error: &AiError) {
    if let Err(pe) = conversation.record_turn_error(error).await {
        tracing::warn!(
            error = ?pe,
            error.type = "persist_turn_error",
            "failed to persist turn error"
        );
    }
}

fn tool_request(call: &ToolCall) -> ChatStreamEvent {
    ChatStreamEvent::ToolRequest {
        tool_call_id: call.id.to_string(),
        name: call.function.name.to_string(),
        args: call.function.arguments_value(),
    }
}

async fn request_tokens(agent: &ModelAgent, messages: &[Message]) -> u64 {
    let tools = agent.agent.tool_definitions(None).await.unwrap_or_default();
    let spec = agent.agent.run_spec();
    let conversation = History::from_messages(messages.iter().cloned());
    let estimate = rig::memory::Prepared {
        model: &agent.model,
        prompt: spec.preamble.as_deref(),
        tools: &tools,
        history: &conversation,
    }
    .whole_tokens();
    tracing::debug!(estimate, "estimated request tokens");
    estimate
}

#[cfg(test)]
mod tests {
    use rig::conversation::Conversation as History;
    use serde_json::{json, Value};

    use super::*;
    use crate::models::history::TranscriptEntry;
    use crate::test_support::{
        gemini_parts, CountingLimit, FakeModel, FeeActions, MemoryConversation, NoLimit, Reply,
    };
    use crate::tools::record_fee::RecordFeeTool;
    use crate::tools::run_script::RunScriptTool;

    fn reply(parts: Value) -> Value {
        json!({
            "candidates": [{"content": {"role": "model", "parts": parts}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}
        })
    }

    fn text(text: &str) -> Value {
        reply(json!([{ "text": text }]))
    }

    fn run_script_call(id: &str) -> Value {
        json!({"functionCall": {"id": id, "name": "run_script", "args": {"script": "1 + 1"}}, "thoughtSignature": "c2lnbmF0dXJl"})
    }

    fn fee_call(id: &str, amount: f64) -> Value {
        json!({"functionCall": {"id": id, "name": "record_fee", "args": {
            "account_id": "00000000-0000-0000-0000-000000000000", "asset_id": 45, "amount": amount
        }}})
    }

    fn message(text: &str) -> ChatTurn {
        ChatTurn::Message {
            message: text.to_string(),
            file_ids: Vec::new(),
        }
    }

    fn chat_agent(model: &FakeModel, actions: &Arc<FeeActions>) -> ModelAgent {
        let actions = actions.clone();
        model.agent(|builder| {
            builder
                .tool(RunScriptTool::new(rig::tool::ToolSet::default()))
                .tool(RecordFeeTool::new(actions))
        })
    }

    async fn stream(
        conversation: &Arc<MemoryConversation>,
        model: &FakeModel,
        actions: &Arc<FeeActions>,
        turn: ChatTurn,
    ) -> Vec<ChatStreamEvent> {
        Conversation::new(conversation.clone(), Arc::new(NoLimit))
            .stream(chat_agent(model, actions), actions.clone(), turn)
            .await
            .unwrap()
            .collect()
            .await
    }

    async fn saved(conversation: &MemoryConversation) -> History {
        history::load(conversation).await.unwrap()
    }

    fn pending(history: &History) -> Vec<String> {
        history
            .pending_calls()
            .iter()
            .map(|call| call.id.to_string())
            .collect()
    }

    fn tool_results(request: &Value) -> Vec<Value> {
        gemini_parts(request)
            .into_iter()
            .filter_map(|part| part.get("functionResponse").cloned())
            .collect()
    }

    fn errors(events: &[ChatStreamEvent]) -> Vec<&AiError> {
        events
            .iter()
            .filter_map(|event| match event {
                ChatStreamEvent::Error(error) => Some(error),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn an_ephemeral_run_saves_nothing() {
        let model = FakeModel::start(vec![Reply::Json(text("A Title"))]).await;
        let conversation = Arc::new(MemoryConversation::default());

        Conversation::new(conversation.clone(), Arc::new(NoLimit))
            .run(
                model.agent(|builder| builder),
                message("title this"),
                Persistence::Ephemeral,
            )
            .await
            .unwrap();

        assert!(conversation.history().is_empty());
    }

    #[tokio::test]
    async fn a_message_with_files_saves_references_and_sends_the_files() {
        let image = Uuid::new_v4();
        let statement = Uuid::new_v4();
        let csv = Uuid::new_v4();
        let model = FakeModel::start(vec![Reply::Json(text("seen"))]).await;
        let conversation = Arc::new(
            MemoryConversation::default()
                .with_file(image, "image/png", b"PNG receipt")
                .with_file(statement, "application/pdf", b"%PDF-1.7 statement")
                .with_file(csv, "text/csv", b"date,amount\n2026-10-01,-12.50\n"),
        );

        Conversation::new(conversation.clone(), Arc::new(NoLimit))
            .run(
                model.agent(|builder| builder),
                ChatTurn::Message {
                    message: "what are these".into(),
                    file_ids: vec![image, statement, csv],
                },
                Persistence::Persist,
            )
            .await
            .unwrap();

        let saved = String::from_utf8(conversation.history()).unwrap();
        assert!(!saved.contains("UE5HIHJlY2VpcHQ=") && !saved.contains("2026-10-01"));
        let parts = gemini_parts(&model.requests()[0]);
        let mime_types: Vec<&Value> = parts
            .iter()
            .filter_map(|part| part.pointer("/inlineData/mimeType"))
            .collect();
        assert_eq!(mime_types, vec!["image/png", "application/pdf"]);
        assert!(parts
            .iter()
            .any(|part| part["text"] == "date,amount\n2026-10-01,-12.50\n"));
        let items = history::transcript(&conversation.history()).unwrap();
        assert_eq!(items[0].file_ids, vec![image, statement, csv]);
    }

    #[tokio::test]
    async fn a_gated_call_pauses_the_stream_and_its_neighbour_still_runs() {
        let model = FakeModel::start(vec![Reply::Sse(reply(json!([
            run_script_call("c1"),
            fee_call("c2", 1.5)
        ])))])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        let events = stream(&conversation, &model, &actions, message("record a fee")).await;

        assert!(events.iter().any(|event| matches!(event,
            ChatStreamEvent::ToolResult { name, output } if name == "run_script" && output.contains("\"result\":2"))));
        assert!(events.iter().any(|event| matches!(event,
            ChatStreamEvent::ToolRequest { tool_call_id, name, .. } if tool_call_id == "c2" && name == "record_fee")));
        assert!(errors(&events).is_empty());
        assert!(actions.fees.lock().unwrap().is_empty());
        let history = saved(&conversation).await;
        assert_eq!(pending(&history), vec!["c2"]);
        assert_eq!(history.messages().len(), 3);
        let can_continue = Conversation::new(conversation.clone(), Arc::new(NoLimit))
            .can_continue()
            .await
            .unwrap();
        assert!(!can_continue);
    }

    #[tokio::test]
    async fn approving_one_call_and_declining_another_resumes_the_model_with_both_answers() {
        let model = FakeModel::start(vec![
            Reply::Sse(reply(json!([
                {"text": "Two fees to record.", "thought": true},
                fee_call("c1", 1.5),
                fee_call("c2", 2.5)
            ]))),
            Reply::Sse(text("Recorded one fee and skipped the other.")),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        let paused = stream(&conversation, &model, &actions, message("record two fees")).await;
        let requested: Vec<&String> = paused
            .iter()
            .filter_map(|event| match event {
                ChatStreamEvent::ToolRequest { tool_call_id, .. } => Some(tool_call_id),
                _ => None,
            })
            .collect();
        assert_eq!(requested, vec!["c1", "c2"]);

        let resumed = stream(
            &conversation,
            &model,
            &actions,
            ChatTurn::Approval {
                approvals: vec![("c1".into(), true), ("c2".into(), false)],
            },
        )
        .await;

        assert!(errors(&resumed).is_empty());
        assert!(resumed.iter().any(|event| matches!(event,
            ChatStreamEvent::ToolResult { name, output } if name == "record_fee" && output.contains("Recorded a fee of 1.5"))));
        assert!(resumed
            .iter()
            .any(|event| matches!(event, ChatStreamEvent::Text(_))));
        assert_eq!(*actions.fees.lock().unwrap(), vec!["1.5"]);

        let resume_request = &model.requests()[1];
        let answers = tool_results(resume_request);
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0]["id"], "c1");
        assert!(answers[1].to_string().contains(DECLINED));
        assert!(gemini_parts(resume_request)
            .iter()
            .any(|part| part["thought"] == true && part["text"] == "Two fees to record."));

        let history = saved(&conversation).await;
        assert!(pending(&history).is_empty());
        assert!(!history.awaits_model());
        let approvals: Vec<(String, bool)> = history::transcript(&conversation.history())
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.entry {
                TranscriptEntry::ToolApproval {
                    tool_call_id,
                    approved,
                } => Some((tool_call_id, approved)),
                _ => None,
            })
            .collect();
        assert_eq!(approvals, vec![("c1".into(), true), ("c2".into(), false)]);
    }

    #[tokio::test]
    async fn a_call_replayed_from_the_saved_history_keeps_its_signature() {
        let model = FakeModel::start(vec![
            Reply::Sse(reply(json!([run_script_call("c1")]))),
            Reply::Sse(text("It is 2.")),
            Reply::Sse(text("You're welcome.")),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        stream(&conversation, &model, &actions, message("what is 1 + 1")).await;
        stream(&conversation, &model, &actions, message("thanks")).await;

        let signatures: Vec<Value> = gemini_parts(&model.requests()[2])
            .into_iter()
            .filter_map(|part| part.get("thoughtSignature").cloned())
            .collect();
        assert_eq!(signatures, vec![json!("c2lnbmF0dXJl")]);
    }

    #[tokio::test]
    async fn a_model_call_that_fails_after_a_tool_ran_keeps_the_tool_and_can_be_retried() {
        let model = FakeModel::start(vec![
            Reply::Sse(reply(json!([run_script_call("c1")]))),
            Reply::Status(
                400,
                json!({"error": {"code": 400, "message": "bad request", "status": "INVALID_ARGUMENT"}}),
            ),
            Reply::Sse(text("It is 2.")),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        let failed = stream(&conversation, &model, &actions, message("what is 1 + 1")).await;

        assert!(matches!(errors(&failed)[..], [AiError::Fatal { .. }]));
        assert_eq!(conversation.turn_errors.lock().unwrap().len(), 1);
        assert_eq!(saved(&conversation).await.messages().len(), 3);
        let wrapper = Conversation::new(conversation.clone(), Arc::new(NoLimit));
        assert!(wrapper.can_continue().await.unwrap());

        let retried = stream(&conversation, &model, &actions, ChatTurn::Continuation).await;

        assert!(errors(&retried).is_empty());
        assert!(!retried
            .iter()
            .any(|event| matches!(event, ChatStreamEvent::ToolResult { .. })));
        assert_eq!(tool_results(&model.requests()[2]).len(), 1);
        assert!(!wrapper.can_continue().await.unwrap());
    }

    #[tokio::test]
    async fn a_new_message_answers_calls_left_waiting_for_approval() {
        let model = FakeModel::start(vec![
            Reply::Sse(reply(json!([fee_call("c1", 1.5)]))),
            Reply::Sse(text("Okay, never mind the fee.")),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        stream(&conversation, &model, &actions, message("record a fee")).await;
        stream(&conversation, &model, &actions, message("actually, don't")).await;

        let answers = tool_results(&model.requests()[1]);
        assert_eq!(answers.len(), 1);
        assert!(answers[0].to_string().contains(SUPERSEDED));
        assert!(actions.fees.lock().unwrap().is_empty());
        assert!(pending(&saved(&conversation).await).is_empty());
    }

    #[tokio::test]
    async fn a_finished_conversation_has_nothing_to_continue() {
        let model = FakeModel::start(vec![Reply::Json(text("done"))]).await;
        let conversation = Arc::new(MemoryConversation::default());
        let wrapper = Conversation::new(conversation.clone(), Arc::new(NoLimit));
        wrapper
            .run(
                model.agent(|builder| builder),
                message("hello"),
                Persistence::Persist,
            )
            .await
            .unwrap();

        assert!(!wrapper.can_continue().await.unwrap());
        let continued = wrapper
            .run(
                model.agent(|builder| builder),
                ChatTurn::Continuation,
                Persistence::Persist,
            )
            .await;
        assert!(matches!(continued, Err(AiError::Fatal { .. })));
    }

    async fn pause_on_a_fee(
        conversation: &Arc<MemoryConversation>,
        actions: &Arc<FeeActions>,
        replies: Vec<Reply>,
    ) -> FakeModel {
        let mut all = vec![Reply::Sse(reply(json!([fee_call("c1", 1.5)])))];
        all.extend(replies);
        let model = FakeModel::start(all).await;
        stream(conversation, &model, actions, message("record a fee")).await;
        model
    }

    fn approve(id: &str) -> ChatTurn {
        ChatTurn::Approval {
            approvals: vec![(id.into(), true)],
        }
    }

    #[tokio::test]
    async fn a_repeated_approval_runs_the_call_once() {
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());
        let model =
            pause_on_a_fee(&conversation, &actions, vec![Reply::Sse(text("Recorded."))]).await;

        let doubled = ChatTurn::Approval {
            approvals: vec![("c1".into(), true), ("c1".into(), true)],
        };
        let first = stream(&conversation, &model, &actions, doubled).await;
        let again = stream(&conversation, &model, &actions, approve("c1")).await;

        assert_eq!(*actions.fees.lock().unwrap(), vec!["1.5"]);
        assert_eq!(errors(&first).len(), 1);
        assert!(matches!(errors(&again)[..], [AiError::Unknown { .. }]));
        assert_eq!(model.requests().len(), 2);
        let answers = tool_results(&model.requests()[1]);
        assert_eq!(answers.len(), 1);
    }

    #[tokio::test]
    async fn an_approval_that_another_request_got_to_first_runs_nothing() {
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());
        let model = pause_on_a_fee(&conversation, &actions, Vec::new()).await;
        conversation
            .refuse_writes
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let refused = Conversation::new(conversation.clone(), Arc::new(NoLimit))
            .stream(chat_agent(&model, &actions), actions.clone(), approve("c1"))
            .await;

        assert!(matches!(refused.err(), Some(AiError::ConcurrencyLimited)));
        assert!(actions.fees.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_turn_with_calls_is_answered_so_it_can_be_retried() {
        let failed = json!({
            "candidates": [{"content": {"role": "model", "parts": [run_script_call("c1")]}, "finishReason": "UNEXPECTED_TOOL_CALL"}],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}
        });
        let model = FakeModel::start(vec![Reply::Sse(failed)]).await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());

        let events = stream(&conversation, &model, &actions, message("what is 1 + 1")).await;

        assert_eq!(errors(&events).len(), 1);
        assert!(!events
            .iter()
            .any(|event| matches!(event, ChatStreamEvent::ToolRequest { .. })));
        let history = saved(&conversation).await;
        assert!(pending(&history).is_empty());
        let wrapper = Conversation::new(conversation.clone(), Arc::new(NoLimit));
        assert!(wrapper.can_continue().await.unwrap());
    }

    #[tokio::test]
    async fn every_admitted_turn_ends_with_one_usage_record_or_release() {
        let limit = Arc::new(CountingLimit::default());
        let model = FakeModel::start(vec![
            Reply::Sse(text("hello")),
            Reply::Sse(reply(json!([fee_call("c1", 1.5)]))),
            Reply::Status(
                400,
                json!({"error": {"code": 400, "message": "bad request", "status": "INVALID_ARGUMENT"}}),
            ),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let actions = Arc::new(FeeActions::default());
        let wrapper = Conversation::new(conversation.clone(), limit.clone());

        for turn in [message("hi"), message("record a fee"), approve("c1")] {
            let _: Vec<ChatStreamEvent> = wrapper
                .stream(chat_agent(&model, &actions), actions.clone(), turn)
                .await
                .unwrap()
                .collect()
                .await;
        }

        let (admitted, _, released) = limit.counts();
        assert_eq!(admitted, 3);
        assert_eq!(limit.charged(), vec![(10, 5), (10, 5)]);
        assert_eq!(released, 1);
    }

    #[tokio::test]
    async fn a_run_that_fails_after_a_model_call_answered_is_charged_for_it() {
        let limit = Arc::new(CountingLimit::default());
        let model = FakeModel::start(vec![
            Reply::Json(reply(json!([run_script_call("c1")]))),
            Reply::Status(
                400,
                json!({"error": {"code": 400, "message": "bad request", "status": "INVALID_ARGUMENT"}}),
            ),
        ])
        .await;

        let result = Conversation::new(Arc::new(MemoryConversation::default()), limit.clone())
            .run(
                model.agent(|builder| {
                    builder
                        .default_max_turns(5)
                        .tool(RunScriptTool::new(rig::tool::ToolSet::default()))
                }),
                message("what is 1 + 1"),
                Persistence::Ephemeral,
            )
            .await;

        assert!(matches!(result, Err(AiError::Fatal { .. })));
        assert_eq!(limit.charged(), vec![(10, 5)]);
        assert_eq!(limit.counts().2, 0);
    }

    #[tokio::test]
    async fn files_stay_out_of_the_history_across_tool_calls_and_turns() {
        let receipt = Uuid::new_v4();
        let statement = Uuid::new_v4();
        let model = FakeModel::start(vec![
            Reply::Sse(reply(json!([run_script_call("c1")]))),
            Reply::Sse(text("Totals match.")),
            Reply::Sse(text("Still the same.")),
        ])
        .await;
        let conversation = Arc::new(
            MemoryConversation::default()
                .with_file(receipt, "image/png", b"PNG receipt")
                .with_file(statement, "application/pdf", b"%PDF-1.7 statement"),
        );
        let actions = Arc::new(FeeActions::default());

        stream(
            &conversation,
            &model,
            &actions,
            ChatTurn::Message {
                message: "do these match".into(),
                file_ids: vec![receipt, statement],
            },
        )
        .await;
        stream(&conversation, &model, &actions, message("and now?")).await;

        let saved = String::from_utf8(conversation.history()).unwrap();
        assert!(!saved.contains("UE5HIHJlY2VpcHQ="));
        assert!(!saved.contains("JVBERi0xLjcgc3RhdGVtZW50"));
        let inline = gemini_parts(&model.requests()[2])
            .into_iter()
            .filter(|part| part.get("inlineData").is_some())
            .count();
        assert_eq!(inline, 2);
    }

    #[derive(Debug, PartialEq, serde::Deserialize, schemars::JsonSchema)]
    struct Answer {
        rows: Vec<u32>,
    }

    fn text_reply(text: &str) -> Reply {
        Reply::Json(json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": text}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 2, "totalTokenCount": 12}
        }))
    }

    #[tokio::test]
    async fn a_structured_answer_is_parsed_and_a_malformed_one_is_an_invalid_answer() {
        let model = FakeModel::start(vec![
            text_reply(r#"{"rows":[1,2]}"#),
            text_reply("not json"),
        ])
        .await;
        let conversation =
            Conversation::new(Arc::new(MemoryConversation::default()), Arc::new(NoLimit));
        let turn = || ChatTurn::Message {
            message: "classify".to_string(),
            file_ids: Vec::new(),
        };

        let parsed = conversation
            .run_structured(
                model.structured::<Answer>(|builder| builder),
                turn(),
                Persistence::Ephemeral,
            )
            .await
            .unwrap();
        let malformed = conversation
            .run_structured(
                model.structured::<Answer>(|builder| builder),
                turn(),
                Persistence::Ephemeral,
            )
            .await;

        assert_eq!(parsed.value, Answer { rows: vec![1, 2] });
        assert_eq!(parsed.usage.input, 10);
        assert!(matches!(malformed, Err(AiError::InvalidAnswer { .. })));
    }
}
