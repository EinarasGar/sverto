use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rig::agent::{AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, RunSettled};
use rig::conversation::{pairing, Conversation};
use rig::message::{Message, ToolCall, UserContent};

use crate::agents::approval::ApprovalHook;
use crate::conversation_provider::ConversationProvider;
use crate::models::error::AiError;

const NOT_RUN: &str = "This call never ran: the model's turn failed.";

pub(crate) struct CommitHook<C> {
    provider: Arc<C>,
    conversation: Arc<tokio::sync::Mutex<Conversation>>,
    earlier: usize,
    approvals: Option<ApprovalHook>,
    committed: Arc<Mutex<usize>>,
    failure: Arc<Mutex<Option<AiError>>>,
}

impl<C> Clone for CommitHook<C> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            conversation: self.conversation.clone(),
            earlier: self.earlier,
            approvals: self.approvals.clone(),
            committed: self.committed.clone(),
            failure: self.failure.clone(),
        }
    }
}

impl<C: ConversationProvider> CommitHook<C> {
    pub(crate) fn new(
        provider: Arc<C>,
        conversation: Conversation,
        earlier: usize,
        approvals: Option<ApprovalHook>,
    ) -> Self {
        Self {
            provider,
            conversation: Arc::new(tokio::sync::Mutex::new(conversation)),
            earlier,
            approvals,
            committed: Arc::default(),
            failure: Arc::default(),
        }
    }

    pub(crate) fn has_failed(&self) -> bool {
        lock(&self.failure).is_some()
    }

    pub(crate) fn take_failure(&self) -> Option<AiError> {
        lock(&self.failure).take()
    }

    async fn commit(&self, run: Vec<Message>, settled: bool) -> bool {
        let new: Vec<Message> = {
            let mut committed = lock(&self.committed);
            let new = run.into_iter().skip(*committed).collect::<Vec<_>>();
            *committed += new.len();
            new.into_iter()
                .filter_map(|message| self.without_held_results(message))
                .collect()
        };
        let mut conversation = self.conversation.lock().await;
        let mut changed = !new.is_empty();
        conversation.extend(new);
        if settled {
            let not_run: Vec<ToolCall> = conversation
                .pending_calls()
                .into_iter()
                .filter(|call| !self.is_held(call))
                .collect();
            if !not_run.is_empty() {
                conversation.push(pairing::close_pending_with(&not_run, NOT_RUN));
                changed = true;
            }
        }
        if !changed {
            return true;
        }
        match super::save(&*self.provider, &mut conversation).await {
            Ok(()) => true,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    error.type = "save_conversation_history",
                    "failed to save the conversation history"
                );
                *lock(&self.failure) = Some(error);
                false
            }
        }
    }

    fn without_held_results(&self, message: Message) -> Option<Message> {
        let Message::User { content } = &message else {
            return Some(message);
        };
        let content: Vec<UserContent> = content
            .iter()
            .filter(|part| !matches!(part, UserContent::ToolResult(result) if self.is_held_id(&result.call.to_string())))
            .cloned()
            .collect();
        (!content.is_empty()).then_some(Message::User { content })
    }

    fn is_held(&self, call: &ToolCall) -> bool {
        self.is_held_id(&call.id.to_string())
    }

    fn is_held_id(&self, call_id: &str) -> bool {
        self.approvals
            .as_ref()
            .is_some_and(|approvals| approvals.is_pending(call_id))
    }
}

impl<C: ConversationProvider> AgentHook for CommitHook<C> {
    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        if event.turn == 1 {
            return CompletionCallAction::continue_run();
        }
        let run = event
            .history
            .iter()
            .skip(self.earlier + 1)
            .map(|message| (**message).clone())
            .chain([event.prompt.clone()])
            .collect();
        if self.commit(run, false).await {
            CompletionCallAction::continue_run()
        } else {
            CompletionCallAction::stop("the conversation history could not be saved")
        }
    }

    async fn on_run_settled(&self, _ctx: &HookContext, event: RunSettled<'_>) {
        let run = event
            .messages
            .unwrap_or_default()
            .iter()
            .skip(1)
            .map(|message| (**message).clone())
            .collect();
        self.commit(run, true).await;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
