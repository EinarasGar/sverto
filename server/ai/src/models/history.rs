use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryWrite {
    Written,
    Changed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptItem {
    pub id: String,
    pub entry: TranscriptEntry,
    pub file_ids: Vec<Uuid>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptEntry {
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ToolApprovalAside {
    pub tool_call_id: String,
    pub approved: bool,
}
