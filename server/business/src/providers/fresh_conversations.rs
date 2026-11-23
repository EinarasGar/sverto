use std::sync::{Arc, Mutex};

use ai::conversation_provider::ConversationSource;
use uuid::Uuid;

use super::user_conversation_provider::UserConversationProvider;
use crate::service_collection::ai_conversation_service::AiConversationService;
use crate::service_collection::ServiceProviders;

pub struct FreshConversations {
    providers: ServiceProviders,
    user_id: Uuid,
    opened: Mutex<Vec<Uuid>>,
}

impl FreshConversations {
    pub fn new(providers: ServiceProviders, user_id: Uuid) -> Self {
        Self {
            providers,
            user_id,
            opened: Mutex::new(Vec::new()),
        }
    }

    pub async fn delete(self) -> anyhow::Result<()> {
        let conversation_ids = self
            .opened
            .into_inner()
            .map_err(|_| anyhow::anyhow!("a conversation was being opened when it panicked"))?;
        if conversation_ids.is_empty() {
            return Ok(());
        }
        AiConversationService::new(&self.providers)
            .delete_conversations(conversation_ids, self.user_id)
            .await
    }
}

impl ConversationSource for FreshConversations {
    type Conversation = UserConversationProvider;

    async fn open(&self) -> anyhow::Result<Option<Arc<UserConversationProvider>>> {
        let conversation_id = AiConversationService::new(&self.providers)
            .create_conversation(self.user_id)
            .await?;
        self.opened
            .lock()
            .map_err(|_| anyhow::anyhow!("a conversation was being opened when it panicked"))?
            .push(conversation_id);
        Ok(Some(Arc::new(
            UserConversationProvider::open(&self.providers, self.user_id, conversation_id).await?,
        )))
    }
}
