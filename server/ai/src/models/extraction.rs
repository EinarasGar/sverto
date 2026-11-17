use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use time::Date;
use uuid::Uuid;

use super::chat::TokenUsage;

pub type ExtractionGate<T> = dyn Fn(&Value) -> Result<T, Value> + Send + Sync;

pub struct ExtractionInput {
    pub sample: String,
    pub records: Value,
    pub read_again: bool,
}

pub struct ImageReadingInput {
    pub file_id: Uuid,
    pub uploaded_on: Date,
    pub read_again: bool,
}

pub struct ExtractionRun<T> {
    pub outcome: ExtractionOutcome<T>,
    pub attempts: u32,
    pub usage: TokenUsage,
}

pub enum ExtractionOutcome<T> {
    Accepted(T),
    NotAccepted(ExtractionStop),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionStop {
    NothingFound,
    NoProgress,
    AttemptLimit,
    ModelFinished,
    TurnLimit,
    CutOff,
}

pub struct AttemptLog<T> {
    pub attempts: u32,
    pub explorations: u32,
    pub accepted: Option<T>,
    pub last_output: Option<Value>,
    pub repeats: u32,
    pub stop: Option<ExtractionStop>,
}

impl<T> AttemptLog<T> {
    pub fn accept(&mut self, result: T) -> Value {
        self.attempts += 1;
        self.accepted = Some(result);
        json!({ "accepted": true })
    }

    pub fn refuse(&mut self, output: Value, mut feedback: Value) -> Value {
        self.attempts += 1;
        let repeated = self.last_output.as_ref() == Some(&output);
        self.repeats = if repeated { self.repeats + 1 } else { 0 };
        self.last_output = Some(output);
        if repeated {
            feedback["note"] =
                json!("this output is identical to your previous one; change the approach");
        }
        feedback
    }
}

impl<T> Default for AttemptLog<T> {
    fn default() -> Self {
        Self {
            attempts: 0,
            explorations: 0,
            accepted: None,
            last_output: None,
            repeats: 0,
            stop: None,
        }
    }
}

pub type SharedAttempts<T> = Arc<Mutex<AttemptLog<T>>>;
