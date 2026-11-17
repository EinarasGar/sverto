use std::sync::{Arc, Mutex, PoisonError};

use rig::agent::{AgentHook, HookContext, OutcomeAction, OutcomeEvent};
use rig::completion::UsageTotals;

#[derive(Clone, Default)]
pub(crate) struct UsageHook(Arc<Mutex<UsageTotals>>);

impl UsageHook {
    pub(crate) fn totals(&self) -> UsageTotals {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AgentHook for UsageHook {
    async fn on_outcome(&self, _ctx: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        if let Some(response) = event.completion() {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .record(&response.usage);
        }
        OutcomeAction::proceed()
    }
}
