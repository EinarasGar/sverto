use std::time::Duration;

use super::ToolError;
use crate::models::sandbox::SandboxLimits;
use crate::models::tool_output::RunScriptArgs;
use crate::sandbox;
use rig::completion::ToolDefinition;
use rig::message::ToolResultContent;
use rig::tool::{Tool, ToolContext, ToolExecutionError, ToolSet};
use serde_json::{json, Value};

const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);
const LOOP_ITERATION_LIMIT: u64 = 50_000_000;
const MAX_TOTAL_INJECTED_ROWS: usize = 50_000;

pub struct RunScriptTool {
    sources: ToolSet,
    source_defs: Vec<std::sync::Arc<ToolDefinition>>,
}

impl RunScriptTool {
    pub fn new(sources: ToolSet) -> Self {
        let source_defs = sources.tool_definitions();
        Self {
            sources,
            source_defs,
        }
    }
}

impl Tool for RunScriptTool {
    const NAME: &'static str = "run_script";

    type Error = ToolError;
    type Args = RunScriptArgs;
    type Output = Value;

    fn description(&self) -> String {
        let sources_doc = self
            .source_defs
            .iter()
            .map(|d| format!("- {}: {}", d.name, d.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Run JavaScript in a sandboxed interpreter to compute over data. No network, file, or system access — pure computation only (arithmetic, arrays, objects, strings, JSON, Math). The value of the script's final expression is returned as JSON.\n\nOptionally declare `datasets` to load the user's data server-side (uncapped) before the script runs; each is injected as a global array named by its `name`. The script cannot fetch more data — declare everything up front. Use this to process large amounts of data (e.g. deduplicating or totalling hundreds of transactions) without listing it into the conversation.\n\nLoadable read-only sources (each injects a JSON array):\n{sources_doc}\n\nExample: datasets [{{\"name\": \"txns\", \"tool\": \"query_transactions\", \"args\": {{\"date_from\": \"2025-01-01\", \"date_to\": \"2025-06-30\"}}}}], script \"txns.filter(t => t.amount < 0).reduce((a, t) => a + t.amount, 0)\"."
        )
    }

    fn parameters(&self) -> Value {
        let source_names: Vec<String> = self
            .source_defs
            .iter()
            .map(|d| d.name.to_string())
            .collect();
        json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "JavaScript source. The value of the final expression is returned. Declared datasets are available as global arrays."
                },
                "datasets": {
                    "type": "array",
                    "description": "Optional datasets to fetch server-side and expose to the script as globals.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Global variable name to bind the dataset to. Must be a valid JavaScript identifier, e.g. `txns`." },
                            "tool": { "type": "string", "enum": source_names, "description": "Which read-only data source to load." },
                            "args": { "type": "object", "description": "Arguments for the source, using the parameters described for it above." }
                        },
                        "required": ["name", "tool"]
                    }
                }
            },
            "required": ["script"]
        })
    }

    fn map_error(&self, error: Self::Error) -> ToolExecutionError {
        error.into()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(tool = Self::NAME))]
    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> std::result::Result<Self::Output, Self::Error> {
        let mut globals: Vec<(String, Value)> = Vec::new();
        let mut total_rows = 0usize;

        for dataset in &args.datasets {
            if !self.sources.contains(&dataset.tool) {
                return Ok(error_output(&format!(
                    "unknown source `{}` for dataset `{}`",
                    dataset.tool, dataset.name
                )));
            }

            let dataset_args = if dataset.args.is_null() {
                "{}".to_string()
            } else {
                dataset.args.to_string()
            };

            let output = match self
                .sources
                .execute(&dataset.tool, dataset_args, &mut ToolContext::new())
                .await
                .into_result()
            {
                Ok(output) => output,
                Err(e) => {
                    return Ok(error_output(&format!(
                        "failed to load dataset `{}`: {}",
                        dataset.name,
                        e.message()
                    )))
                }
            };

            let Some(ToolResultContent::Json { value }) = output.into_content().into_iter().next()
            else {
                return Ok(error_output(&format!(
                    "dataset `{}` did not load as JSON",
                    dataset.name
                )));
            };
            total_rows += value.as_array().map_or(0, Vec::len);
            if total_rows > MAX_TOTAL_INJECTED_ROWS {
                return Ok(error_output(&format!(
                    "datasets exceed the {MAX_TOTAL_INJECTED_ROWS} row injection limit; narrow the filters (e.g. a tighter date range)"
                )));
            }

            globals.push((dataset.name.clone(), value));
        }

        let limits = SandboxLimits {
            timeout: SCRIPT_TIMEOUT,
            max_loop_iterations: LOOP_ITERATION_LIMIT,
        };
        Ok(
            match sandbox::execute("", args.script, globals, limits)
                .await
                .result
            {
                Ok(result) => json!({ "ok": true, "result": result }),
                Err(message) => error_output(&message),
            },
        )
    }
}

fn error_output(message: &str) -> Value {
    json!({ "ok": false, "error": message })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::models::tool_output::DatasetSpec;

    #[derive(Deserialize)]
    struct NoArgs {}

    struct Numbers;

    impl Tool for Numbers {
        const NAME: &'static str = "numbers";

        type Error = ToolError;
        type Args = NoArgs;
        type Output = Value;

        fn description(&self) -> String {
            "Numbers".to_string()
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }

        async fn call(
            &self,
            _context: &mut ToolContext,
            _args: NoArgs,
        ) -> Result<Value, ToolError> {
            Ok(json!([1, 2, 3]))
        }
    }

    fn run_script_over_numbers() -> RunScriptTool {
        let mut sources = ToolSet::default();
        sources.add_tool(Numbers);
        RunScriptTool::new(sources)
    }

    #[tokio::test]
    async fn a_dataset_is_loaded_from_its_source_tool() {
        let output = run_script_over_numbers()
            .call(
                &mut ToolContext::new(),
                RunScriptArgs {
                    script: "xs.reduce((sum, x) => sum + x, 0)".to_string(),
                    datasets: vec![DatasetSpec {
                        name: "xs".to_string(),
                        tool: "numbers".to_string(),
                        args: Value::Null,
                    }],
                },
            )
            .await
            .unwrap();

        assert_eq!(output, json!({"ok": true, "result": 6}));
    }
}
