use ai::models::history::TranscriptItem;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::ai_chat_dto::ChatHistoryMessageDto;

#[derive(Debug, Clone, Serialize)]
pub struct MessageDto {
    pub id: String,
    pub message: ChatHistoryMessageDto,
    pub file_ids: Vec<Uuid>,
    pub created_at: OffsetDateTime,
}

impl From<TranscriptItem> for MessageDto {
    fn from(item: TranscriptItem) -> Self {
        Self {
            id: item.id,
            message: item.entry.into(),
            file_ids: item.file_ids,
            created_at: item.created_at,
        }
    }
}
