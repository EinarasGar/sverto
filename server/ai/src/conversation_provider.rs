use std::sync::Arc;

use anyhow::Result;
use uuid::Uuid;

use crate::models::chat::Base64Attachment;
use crate::models::error::AiError;
use crate::models::history::HistoryWrite;

pub trait ConversationProvider: Send + Sync + 'static {
    fn read_history(&self) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;

    fn append_history(
        &self,
        expected_len: u64,
        bytes: &[u8],
    ) -> impl std::future::Future<Output = Result<HistoryWrite>> + Send;

    fn fetch_attachments(
        &self,
        file_ids: &[Uuid],
    ) -> impl std::future::Future<Output = Result<Vec<Base64Attachment>>> + Send;

    /// Persist the failed-turn marker on the conversation. Replaces any
    /// previous marker.
    fn record_turn_error(
        &self,
        error: &AiError,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Clear the failed-turn marker.
    fn clear_turn_error(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}

pub trait ConversationSource: Send + Sync {
    type Conversation: ConversationProvider;

    fn open(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<Arc<Self::Conversation>>>> + Send;
}
