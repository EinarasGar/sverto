use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsValue, Source};
use serde_json::Value;

use crate::models::sandbox::{SandboxLimits, SandboxOutcome};

const LOG_GLOBAL: &str = "__logs";

pub async fn execute(
    prelude: &'static str,
    script: String,
    globals: Vec<(String, Value)>,
    limits: SandboxLimits,
) -> SandboxOutcome {
    let max_loop_iterations = limits.max_loop_iterations;
    let join =
        tokio::task::spawn_blocking(move || run(prelude, &script, &globals, max_loop_iterations));
    let failed = |message: String| SandboxOutcome {
        result: Err(message),
        logs: Vec::new(),
    };
    match tokio::time::timeout(limits.timeout, join).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => failed("script execution panicked".to_string()),
        Err(_) => failed(format!(
            "script timed out after {}s",
            limits.timeout.as_secs()
        )),
    }
}

fn run(
    prelude: &str,
    script: &str,
    globals: &[(String, Value)],
    max_loop_iterations: u64,
) -> SandboxOutcome {
    let mut context = Context::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(max_loop_iterations);
    let result = evaluate(&mut context, prelude, script, globals);
    let logs = context
        .eval(Source::from_bytes(&format!(
            "typeof {LOG_GLOBAL} === 'undefined' ? [] : {LOG_GLOBAL}"
        )))
        .ok()
        .and_then(|value| value.to_json(&mut context).ok().flatten())
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    SandboxOutcome { result, logs }
}

fn evaluate(
    context: &mut Context,
    prelude: &str,
    script: &str,
    globals: &[(String, Value)],
) -> Result<Value, String> {
    for (name, value) in globals {
        let js_value = JsValue::from_json(value, context)
            .map_err(|e| format!("failed to inject `{name}`: {e}"))?;
        context
            .register_global_property(js_string!(name.as_str()), js_value, Attribute::all())
            .map_err(|e| format!("failed to register `{name}`: {e}"))?;
    }
    if !prelude.is_empty() {
        context
            .eval(Source::from_bytes(prelude))
            .map_err(|e| format!("prelude failed: {e}"))?;
    }

    let value = context
        .eval(Source::from_bytes(script))
        .map_err(|e| e.to_string())?;
    value
        .to_json(context)
        .map(|result| result.unwrap_or(Value::Null))
        .map_err(|e| format!("result not serializable: {e}"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn limits() -> SandboxLimits {
        SandboxLimits {
            timeout: Duration::from_secs(5),
            max_loop_iterations: 1_000_000,
        }
    }

    #[tokio::test]
    async fn logs_survive_a_failing_script() {
        let outcome = execute(
            "const __logs = []; function log(v) { __logs.push(String(v)); }",
            "log('seen'); null.boom".to_string(),
            Vec::new(),
            limits(),
        )
        .await;

        assert!(outcome.result.is_err());
        assert_eq!(outcome.logs, vec!["seen".to_string()]);
    }

    #[tokio::test]
    async fn runaway_loops_are_stopped() {
        let outcome = execute("", "while (true) {}".to_string(), Vec::new(), limits()).await;

        assert!(outcome.result.is_err());
    }
}
