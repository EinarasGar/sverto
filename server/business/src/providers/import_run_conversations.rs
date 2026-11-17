use std::sync::Arc;

use ai::conversation_provider::ConversationSource;
use uuid::Uuid;

use super::user_conversation_provider::UserConversationProvider;
use crate::service_collection::import_classify_service::ImportClassifyService;
use crate::service_collection::ServiceProviders;

pub struct ImportRunConversations {
    providers: ServiceProviders,
    session_id: Uuid,
    run_id: Uuid,
    user_id: Uuid,
}

impl ImportRunConversations {
    pub fn new(providers: ServiceProviders, session_id: Uuid, run_id: Uuid, user_id: Uuid) -> Self {
        Self {
            providers,
            session_id,
            run_id,
            user_id,
        }
    }
}

impl ConversationSource for ImportRunConversations {
    type Conversation = UserConversationProvider;

    async fn open(&self) -> anyhow::Result<Option<Arc<UserConversationProvider>>> {
        let Some(conversation_id) = ImportClassifyService::new(&self.providers)
            .open_conversation(self.session_id, self.run_id, self.user_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Arc::new(
            UserConversationProvider::open(&self.providers, self.user_id, conversation_id).await?,
        )))
    }
}
