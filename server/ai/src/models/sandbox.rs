use std::time::Duration;

use serde_json::Value;

pub struct SandboxLimits {
    pub timeout: Duration,
    pub max_loop_iterations: u64,
}

pub struct SandboxOutcome {
    pub result: Result<Value, String>,
    pub logs: Vec<String>,
}
