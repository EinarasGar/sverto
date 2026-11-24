use dal::query_params::ai_conversation_params::{ProposalType, QuickUploadStatus};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use dal::models::ai_conversation_models::QuickUploadModel;

use crate::dtos::ai_error_dto::AiErrorDto;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum QuickUploadNotification {
    #[serde(rename = "status")]
    Status { step: String },
    #[serde(rename = "proposal")]
    Proposal {
        proposal_type: ProposalType,
        data: serde_json::Value,
    },
    #[serde(rename = "error")]
    Error { error: AiErrorDto },
    #[serde(rename = "done")]
    Done,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuickUploadDto {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub status: QuickUploadStatus,
    pub source_file_id: Uuid,
    pub proposal_type: Option<ProposalType>,
    pub proposal_data: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<QuickUploadModel> for QuickUploadDto {
    fn from(m: QuickUploadModel) -> Self {
        Self {
            id: m.id,
            conversation_id: m.conversation_id,
            status: m.status.parse().unwrap_or(QuickUploadStatus::Pending),
            source_file_id: m.source_file_id,
            proposal_type: m.proposal_type.and_then(|s| s.parse().ok()),
            proposal_data: m.proposal_data,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReceiptProposalDto {
    pub description: Option<String>,
    pub date: Option<String>,
    pub amount: Option<Decimal>,
    pub asset_id: Option<i32>,
    pub transactions: Option<Vec<ReceiptProposalLineDto>>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiptProposalLineDto {
    pub amount: Option<Decimal>,
    pub asset_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct MergeQuickUploadDto {
    pub transaction_id: Uuid,
    pub description: Option<String>,
    pub category_id: i32,
    pub date: OffsetDateTime,
    pub lines: Vec<MergeQuickUploadLineDto>,
}

#[derive(Debug, Clone)]
pub struct MergeQuickUploadLineDto {
    pub description: Option<String>,
    pub category_id: i32,
    pub amount: Decimal,
}
