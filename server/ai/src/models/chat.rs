use uuid::Uuid;

use crate::models::error::AiError;

#[derive(Clone)]
pub struct Base64Attachment {
    pub media_type: String,
    pub data: String,
}

pub struct PromptOutput {
    pub output: String,
    pub usage: TokenUsage,
}

pub struct StructuredOutput<T> {
    pub value: T,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cached_input: u64,
    pub reasoning: u64,
}

impl From<rig::completion::Usage> for TokenUsage {
    fn from(usage: rig::completion::Usage) -> Self {
        Self {
            input: usage.input_tokens.unwrap_or(0),
            output: usage.output_tokens.unwrap_or(0),
            cached_input: usage.cached_input_tokens.unwrap_or(0),
            reasoning: usage.reasoning_tokens.unwrap_or(0),
        }
    }
}

/// Whether a prompt turn writes to conversation history. Rate limiting (the
/// `pre_check`/`record_usage` pair) runs either way — this only gates the
/// user message and the model's response messages.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Persistence {
    /// Record the user message and persist the response (normal chat turns).
    Persist,
    /// Write nothing to history — for one-off runs like title generation.
    Ephemeral,
}

pub enum ChatTurn {
    Message {
        message: String,
        file_ids: Vec<Uuid>,
    },
    Approval {
        approvals: Vec<(String, bool)>,
    },
    Continuation,
}

pub struct ToolRequestPayload {
    pub tool_call_id: String,
    pub name: String,
    pub args: String,
}

pub enum ChatStreamEvent {
    Text(String),
    ToolCall {
        call_id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        name: String,
        output: String,
    },
    Reasoning(String),
    Error(AiError),
    ToolRequest {
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
}
