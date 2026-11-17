use std::io;

use rig::conversation::Conversation;
use rig::memory::{load_async, save_async, AsyncStore, LogWriteError, MemoryStore, Store};

use crate::conversation_provider::ConversationProvider;
use crate::models::error::AiError;
use crate::models::history::HistoryWrite;

pub(super) const LOG: &str = "history";

struct ProviderStore<'a, C>(&'a C);

impl<C: ConversationProvider> AsyncStore for ProviderStore<'_, C> {
    async fn read_log_async(&self, _: &str) -> io::Result<Vec<u8>> {
        match self.0.read_history().await.map_err(io::Error::other)? {
            history if history.is_empty() => Err(io::ErrorKind::NotFound.into()),
            history => Ok(history),
        }
    }

    async fn append_log_async(&self, _: &str, len: u64, bytes: &[u8]) -> Result<(), LogWriteError> {
        written(self.0.append_history(len, bytes).await)
    }

    async fn replace_log_async(&self, _: &str, _: u64, _: &[u8]) -> Result<(), LogWriteError> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "the history is never rewritten").into())
    }

    async fn read_file_async(&self, _: &str) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::NotFound.into())
    }

    async fn write_file_async(&self, name: &str, _: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("the history keeps no files, so {name} was not saved"),
        ))
    }
}

fn written(result: anyhow::Result<HistoryWrite>) -> Result<(), LogWriteError> {
    match result.map_err(io::Error::other)? {
        HistoryWrite::Written => Ok(()),
        HistoryWrite::Changed => Err(LogWriteError::Changed),
    }
}

pub(crate) async fn load<C: ConversationProvider>(provider: &C) -> Result<Conversation, AiError> {
    Ok(load_async(&ProviderStore(provider), LOG).await?)
}

pub(crate) async fn save<C: ConversationProvider>(
    provider: &C,
    conversation: &mut Conversation,
) -> Result<(), AiError> {
    Ok(save_async(&ProviderStore(provider), LOG, conversation).await?)
}

pub(super) fn parse(history: &[u8]) -> Result<Conversation, AiError> {
    if history.is_empty() {
        return Ok(Conversation::new());
    }
    let store = MemoryStore::default();
    store
        .append_log(LOG, 0, history)
        .map_err(|error| AiError::unknown(format!("{error:#}")))?;
    Ok(rig::memory::load(&store, LOG)?)
}

#[cfg(test)]
mod tests {
    use rig::message::Message;

    use super::*;
    use crate::test_support::MemoryConversation;

    #[tokio::test]
    async fn of_two_writers_that_read_the_same_history_only_the_first_saves() {
        let conversation = MemoryConversation::default();
        let mut first = load(&conversation).await.unwrap();
        let mut second = load(&conversation).await.unwrap();
        first.push(Message::user("one"));
        second.push(Message::user("two"));

        save(&conversation, &mut first).await.unwrap();
        let refused = save(&conversation, &mut second).await;

        assert!(matches!(refused, Err(AiError::ConcurrencyLimited)));
        let messages = load(&conversation).await.unwrap().messages().len();
        assert_eq!(messages, 1);
    }
}
