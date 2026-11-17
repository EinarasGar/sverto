use std::sync::Arc;

use rig::tool::{Tool, ToolContext, ToolExecutionError};
use serde_json::{json, Value};

use super::ToolError;
use crate::models::extraction::{ExtractionGate, SharedAttempts};

pub struct SubmitReadingTool<T> {
    gate: Arc<ExtractionGate<T>>,
    attempts: SharedAttempts<T>,
}

impl<T> SubmitReadingTool<T> {
    pub fn new(gate: Arc<ExtractionGate<T>>, attempts: SharedAttempts<T>) -> Self {
        Self { gate, attempts }
    }
}

impl<T: Send + 'static> Tool for SubmitReadingTool<T> {
    const NAME: &'static str = "submit_reading";

    type Error = ToolError;
    type Args = Value;
    type Output = Value;

    fn description(&self) -> String {
        "Submit your reading of the attached document as the output object. Returns {accepted: true} or the problems to fix.".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "lines": {"type": "array", "items": {"type": "object"}},
                "balances": {"type": "array", "items": {"type": "object"}},
                "control_totals": {"type": "array", "items": {"type": "object"}},
                "period": {"type": "object"}
            },
            "required": ["lines"]
        })
    }

    fn map_error(&self, error: Self::Error) -> ToolExecutionError {
        error.into()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(tool = Self::NAME))]
    async fn call(
        &self,
        _context: &mut ToolContext,
        reading: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let verdict = (self.gate)(&reading);
        let Ok(mut log) = self.attempts.lock() else {
            return Ok(json!({ "accepted": false }));
        };
        Ok(match verdict {
            Ok(result) => log.accept(result),
            Err(feedback) => log.refuse(reading, feedback),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> (SubmitReadingTool<Value>, SharedAttempts<Value>) {
        let attempts: SharedAttempts<Value> = Arc::default();
        let gate: Arc<ExtractionGate<Value>> = Arc::new(|reading: &Value| {
            if reading["lines"] == json!([1]) {
                Ok(reading.clone())
            } else {
                Err(json!({ "accepted": false, "problems": ["wrong line"] }))
            }
        });
        (SubmitReadingTool::new(gate, attempts.clone()), attempts)
    }

    async fn submit(tool: &SubmitReadingTool<Value>, lines: Value) -> Value {
        tool.call(&mut ToolContext::new(), json!({ "lines": lines }))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_refused_reading_handed_back_unchanged_counts_as_a_repeat_until_one_is_accepted() {
        let (tool, attempts) = tool();

        submit(&tool, json!([0])).await;
        let repeated = submit(&tool, json!([0])).await;
        let accepted = submit(&tool, json!([1])).await;

        assert!(repeated.get("note").is_some());
        assert_eq!(accepted, json!({ "accepted": true }));
        let log = attempts.lock().unwrap();
        assert_eq!((log.attempts, log.repeats), (3, 1));
        assert_eq!(log.accepted, Some(json!({ "lines": [1] })));
    }
}
