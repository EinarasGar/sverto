use ai::models::chat::ChatStreamEvent;
use ai::models::history::TranscriptEntry;
use serde::Serialize;
use uuid::Uuid;

use crate::dtos::ai_error_dto::AiErrorDto;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatHistoryMessageDto {
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    AssistantToolCall {
        tool_call_id: String,
        name: String,
        args: String,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
    ToolApproval {
        tool_call_id: String,
        approved: bool,
    },
}

impl ChatHistoryMessageDto {
    pub fn role(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Assistant { .. } => "assistant",
            Self::AssistantToolCall { .. } => "tool_call",
            Self::ToolResult { .. } => "tool_result",
            Self::ToolApproval { .. } => "tool_approval",
        }
    }
}

impl From<TranscriptEntry> for ChatHistoryMessageDto {
    fn from(entry: TranscriptEntry) -> Self {
        match entry {
            TranscriptEntry::User { content } => Self::User { content },
            TranscriptEntry::Assistant { content } => Self::Assistant { content },
            TranscriptEntry::AssistantToolCall {
                tool_call_id,
                name,
                args,
            } => Self::AssistantToolCall {
                tool_call_id,
                name,
                args,
            },
            TranscriptEntry::ToolResult {
                tool_call_id,
                content,
            } => Self::ToolResult {
                tool_call_id,
                content,
            },
            TranscriptEntry::ToolApproval {
                tool_call_id,
                approved,
            } => Self::ToolApproval {
                tool_call_id,
                approved,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatStreamEventDto {
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
    Error(AiErrorDto),
    ToolRequest {
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    Done,
}

impl From<ChatStreamEvent> for ChatStreamEventDto {
    fn from(event: ChatStreamEvent) -> Self {
        match event {
            ChatStreamEvent::Text(text) => ChatStreamEventDto::Text(text),
            ChatStreamEvent::ToolCall {
                call_id,
                name,
                input,
            } => ChatStreamEventDto::ToolCall {
                call_id,
                name,
                input,
            },
            ChatStreamEvent::ToolResult { name, output } => {
                ChatStreamEventDto::ToolResult { name, output }
            }
            ChatStreamEvent::Reasoning(text) => ChatStreamEventDto::Reasoning(text),
            ChatStreamEvent::Error(e) => ChatStreamEventDto::Error(AiErrorDto::from(e)),
            ChatStreamEvent::ToolRequest {
                tool_call_id,
                name,
                args,
            } => ChatStreamEventDto::ToolRequest {
                tool_call_id,
                name,
                args,
            },
            ChatStreamEvent::Usage {
                input_tokens,
                output_tokens,
            } => ChatStreamEventDto::Usage {
                input_tokens,
                output_tokens,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatTurnDto {
    Message {
        message: String,
        file_ids: Vec<Uuid>,
    },
    Approval {
        approvals: Vec<(String, bool)>,
    },
    Continuation,
}

impl From<ChatTurnDto> for ai::models::chat::ChatTurn {
    fn from(t: ChatTurnDto) -> Self {
        match t {
            ChatTurnDto::Message { message, file_ids } => Self::Message { message, file_ids },
            ChatTurnDto::Approval { approvals } => Self::Approval { approvals },
            ChatTurnDto::Continuation => Self::Continuation,
        }
    }
}
