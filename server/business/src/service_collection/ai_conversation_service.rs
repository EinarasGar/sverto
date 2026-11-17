#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::ai_conversation_models::{ChatNeedingTitleModel, ConversationModel};
use dal::queries::ai_conversation_queries;
use dal::query_params::ai_conversation_params::{
    GetConversationsParams, UpdateConversationErrorParams,
};
use itertools::Itertools;
use uuid::Uuid;

use crate::dtos::ai_conversation_dto::{ChatNeedingTitleDto, ConversationDto};
use crate::dtos::ai_error_dto::AiErrorDto;
use crate::dtos::ai_message_dto::MessageDto;
#[derive(Clone)]
pub struct AiConversationService {
    db: MyraDb,
}
impl AiConversationService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_conversation(&self, user_id: Uuid) -> anyhow::Result<Uuid> {
        let id: Uuid = self
            .db
            .fetch_one_scalar(ai_conversation_queries::create_conversation(user_id))
            .await?;
        Ok(id)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_chat(&self, user_id: Uuid) -> anyhow::Result<ConversationDto> {
        self.db.start_transaction().await?;
        let id = self.create_conversation(user_id).await?;
        self.db
            .execute(ai_conversation_queries::create_chat(id))
            .await?;
        self.db.commit_transaction().await?;

        let now = time::OffsetDateTime::now_utc();
        Ok(ConversationDto {
            id,
            user_id,
            title: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, user_id = %user_id))]
    pub async fn get_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<ConversationDto> {
        let query = ai_conversation_queries::get_conversations(GetConversationsParams::by_id(
            conversation_id,
            user_id,
        ));
        let model: ConversationModel = self.db.fetch_one(query).await?;
        Ok(with_preview(model))
    }

    /// Verifies the conversation exists and belongs to the user.
    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, user_id = %user_id))]
    pub async fn ensure_owns_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        let query = ai_conversation_queries::get_owned_conversation_id(conversation_id, user_id);
        let _: Uuid = self.db.fetch_one_scalar(query).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, limit, offset))]
    pub async fn get_conversations(
        &self,
        user_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> anyhow::Result<Vec<ConversationDto>> {
        let query = ai_conversation_queries::get_conversations(GetConversationsParams::all(
            user_id, offset, limit,
        ));
        let models: Vec<ConversationModel> = self.db.fetch_all(query).await?;
        Ok(models.into_iter().map(with_preview).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, user_id = %user_id))]
    pub async fn get_messages(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<MessageDto>> {
        let history = self.get_history(conversation_id, user_id).await?;
        Ok(ai::history::transcript(&history)?
            .into_iter()
            .map_into()
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, user_id = %user_id))]
    pub async fn get_history(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<u8>> {
        let query = ai_conversation_queries::get_conversation_history(conversation_id, user_id);
        Ok(self.db.fetch_one_scalar(query).await?)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, expected_len, len = bytes.len()))]
    pub async fn append_history(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        expected_len: u64,
        bytes: &[u8],
    ) -> anyhow::Result<bool> {
        let query = ai_conversation_queries::append_conversation_history(
            conversation_id,
            user_id,
            expected_len,
            bytes.to_vec(),
        );
        Ok(self.db.execute_with_rows_affected(query).await? == 1)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id, user_id = %user_id))]
    pub async fn delete_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        let query = ai_conversation_queries::delete_conversation(conversation_id, user_id);
        self.db.execute(query).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn delete_conversations(
        &self,
        conversation_ids: Vec<Uuid>,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        let query = ai_conversation_queries::delete_conversations(conversation_ids, user_id);
        self.db.execute(query).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(limit))]
    pub async fn get_chats_needing_titles(
        &self,
        limit: u64,
    ) -> anyhow::Result<Vec<ChatNeedingTitleDto>> {
        let query = ai_conversation_queries::get_chats_needing_titles(limit);
        let models: Vec<ChatNeedingTitleModel> = self.db.fetch_all(query).await?;
        Ok(models.into_iter().map_into().collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id))]
    pub async fn set_generated_title_if_null(
        &self,
        conversation_id: Uuid,
        title: String,
    ) -> anyhow::Result<u64> {
        let query = ai_conversation_queries::update_chat_title_if_null(conversation_id, title);
        Ok(self.db.execute_with_rows_affected(query).await?)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id))]
    pub async fn set_conversation_error(
        &self,
        conversation_id: Uuid,
        error: &AiErrorDto,
    ) -> anyhow::Result<()> {
        let query = ai_conversation_queries::update_conversation_error(
            UpdateConversationErrorParams::set(conversation_id, error),
        );
        self.db.execute(query).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(conversation_id = %conversation_id))]
    pub async fn clear_conversation_error(&self, conversation_id: Uuid) -> anyhow::Result<()> {
        let query = ai_conversation_queries::update_conversation_error(
            UpdateConversationErrorParams::clear(conversation_id),
        );
        self.db.execute(query).await?;
        Ok(())
    }
}

fn with_preview(model: ConversationModel) -> ConversationDto {
    let preview = model.untitled_history_head.as_deref().and_then(|head| {
        ai::history::preview(head)
            .inspect_err(|error| {
                tracing::warn!(
                    error = %error,
                    error.type = "conversation_preview",
                    "failed to read the conversation history for its preview"
                );
            })
            .ok()
            .flatten()
    });
    let dto = ConversationDto::from(model);
    ConversationDto {
        title: dto.title.or(preview),
        ..dto
    }
}
