use rig::agent::{
    AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, ModelTurnAction,
    ModelTurnFinished,
};
use rig::completion::FinishReason;
use serde_json::Value;

use crate::models::extraction::{ExtractionStop, SharedAttempts};

const REPEAT_LIMIT: u32 = 2;

pub struct ExtractionHook<T> {
    pub attempts: SharedAttempts<T>,
    pub max_attempts: u32,
    pub ends_when_cut_off: bool,
}

impl<T: Send + 'static> AgentHook for ExtractionHook<T> {
    async fn on_model_turn_finished(
        &self,
        _ctx: &HookContext,
        event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        if !self.ends_when_cut_off || event.finish_reason != Some(&FinishReason::Length) {
            return ModelTurnAction::continue_run();
        }
        if let Ok(mut log) = self.attempts.lock() {
            log.stop = Some(ExtractionStop::CutOff);
        }
        ModelTurnAction::Stop(format!("{:?}", ExtractionStop::CutOff))
    }

    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        _event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        let Ok(mut log) = self.attempts.lock() else {
            return CompletionCallAction::continue_run();
        };
        if log.accepted.is_some() {
            return CompletionCallAction::stop("accepted");
        }
        let holds_nothing = log.last_output.as_ref().is_some_and(|output| {
            ["lines", "balances"].iter().all(|key| {
                output
                    .get(key)
                    .and_then(Value::as_array)
                    .is_none_or(Vec::is_empty)
            })
        });
        let stop = if holds_nothing && log.repeats >= 1 {
            ExtractionStop::NothingFound
        } else if log.repeats >= REPEAT_LIMIT {
            ExtractionStop::NoProgress
        } else if log.attempts >= self.max_attempts {
            ExtractionStop::AttemptLimit
        } else {
            return CompletionCallAction::continue_run();
        };
        log.stop = Some(stop);
        CompletionCallAction::stop(format!("{stop:?}"))
    }
}
