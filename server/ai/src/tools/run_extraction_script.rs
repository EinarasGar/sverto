use std::sync::Arc;
use std::time::Duration;

use rig::tool::{Tool, ToolContext, ToolExecutionError};
use serde_json::{json, Value};

use super::ToolError;
use crate::models::extraction::{ExtractionGate, SharedAttempts};
use crate::models::sandbox::SandboxLimits;
use crate::models::tool_output::ExtractionScriptArgs;
use crate::sandbox;

const SCRIPT_TIMEOUT: Duration = Duration::from_secs(30);
const LOOP_ITERATION_LIMIT: u64 = 500_000_000;
const MAX_LOG_CHARS: usize = 2000;
pub const EXPLORATION_BUDGET: u32 = 2;

const PRELUDE: &str = r#"
const __logs = [];
function log(...values) {
  __logs.push(values.map((v) => (typeof v === "string" ? v : JSON.stringify(v))).join(" "));
}
function __shiftPoint(decimal, places) {
  let [whole, fraction = ""] = decimal.split(".");
  if (places >= 0) {
    whole = whole.padStart(places + 1, "0");
    fraction = whole.slice(whole.length - places) + fraction;
    whole = whole.slice(0, whole.length - places);
  } else {
    fraction = fraction.padEnd(-places, "0");
    whole += fraction.slice(0, -places);
    fraction = fraction.slice(-places);
  }
  whole = whole.replace(/^0+(?=[0-9])/, "");
  return fraction ? whole + "." + fraction : whole;
}
function toDecimal(text, separator) {
  if (separator !== "." && separator !== ",") {
    throw new Error('toDecimal(text, decimalSeparator) needs "." or "," as the document prints it');
  }
  const raw = String(text);
  const span = raw.match(/[0-9](?:[^]*[0-9])?/);
  if (!span) return null;
  const before = raw.slice(0, span.index);
  const after = raw.slice(span.index + span[0].length);
  const negative =
    /[-−–][^\s0-9]{0,3}$/.test(before) ||
    /^\s*[-−–]\s*[^\s0-9]{0,3}\s*$/.test(before) ||
    /^[-−–]/.test(after) ||
    /^\s*[^\s0-9]{0,3}\s*[-−–]\s*$/.test(after) ||
    (/\([^)]*$/.test(before) && /^[^(]*\)/.test(after)) ||
    /^\s*DR(?![A-Za-z])/i.test(after);
  const scientific = span[0].match(/^([0-9]+(?:[.,][0-9]+)?)[eE]([+-]?[0-9]{1,2})$/);
  const digits = (scientific ? scientific[1] : span[0])
    .replace(separator === "," ? /[^0-9,]/g : /[^0-9.]/g, "")
    .replace(",", ".");
  const decimal = (before.endsWith(separator) ? "0." : "") + digits;
  return (negative ? "-" : "") + (scientific ? __shiftPoint(decimal, -Number(scientific[2])) : decimal);
}
let __minorUnits = false;
function fromMinorUnits(text, decimals) {
  if (!Number.isInteger(decimals) || decimals < 0) {
    throw new Error("fromMinorUnits(text, decimals) needs the currency's number of decimal places");
  }
  const whole = toDecimal(text, ".");
  if (whole === null || whole.includes(".")) return null;
  __minorUnits = true;
  return (whole.startsWith("-") ? "-" : "") + __shiftPoint(whole.replace("-", ""), decimals);
}
function __statingMinorUnits(output) {
  if (__minorUnits && output !== null && typeof output === "object" && !Array.isArray(output)) {
    output.minor_units = true;
  }
  return output;
}
function minus(a, b) {
  const [left, right] = [a, b].map((value) => {
    const parts = String(value).trim().match(/^(-?)([0-9]*)\.?([0-9]*)$/);
    if (!parts || parts[2] + parts[3] === "") {
      throw new Error("minus(a, b) needs two decimal strings such as toDecimal returns, got " + JSON.stringify(value));
    }
    return { sign: parts[1], digits: parts[2], fraction: parts[3] };
  });
  const places = Math.max(left.fraction.length, right.fraction.length);
  const units = (n) => BigInt(n.sign + (n.digits + n.fraction.padEnd(places, "0")));
  const difference = units(left) - units(right);
  const magnitude = String(difference).replace("-", "");
  return (difference < 0n ? "-" : "") + __shiftPoint(magnitude, places);
}
function fromSerialDate(text) {
  const days = Math.floor(Number(text));
  if (!(days >= 1)) return null;
  return new Date(Date.UTC(1899, 11, 30 + days)).toISOString().slice(0, 10);
}
for (const method of ["toFixed", "toPrecision"]) {
  String.prototype[method] = function () {
    throw new Error(method + " is a number method, and toDecimal returns a decimal string: never do arithmetic on it; subtract or negate with minus(a, b)");
  };
}
"#;

pub struct RunExtractionScriptTool<T> {
    records: Arc<Value>,
    gate: Arc<ExtractionGate<T>>,
    attempts: SharedAttempts<T>,
}

impl<T> RunExtractionScriptTool<T> {
    pub fn new(
        records: Arc<Value>,
        gate: Arc<ExtractionGate<T>>,
        attempts: SharedAttempts<T>,
    ) -> Self {
        Self {
            records,
            gate,
            attempts,
        }
    }
}

impl<T: Send + 'static> Tool for RunExtractionScriptTool<T> {
    const NAME: &'static str = "run_script";

    type Error = ToolError;
    type Args = ExtractionScriptArgs;
    type Output = Value;

    fn description(&self) -> String {
        "Run your extraction script over every record. It is a function body that must `return` the output object. Globals: `records` (index = ref); toDecimal(text, decimalSeparator), which reads a printed amount with its sign and returns a decimal string, not a number; fromMinorUnits(text, decimals) for amounts printed as whole minor units (350 for 3.50); minus(a, b), the only arithmetic on such strings: it subtracts exactly, and minus(\"0\", a) negates; fromSerialDate(text) for a spreadsheet day number; and log(...values), whose output comes back with the result. Returns {accepted: true} or the problems to fix. The first two calls that return no lines, balances or skipped records are a look at the records: they cost no attempt and get back their logs, or what they returned when nothing was logged.".to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"script": {"type": "string"}},
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
    ) -> Result<Self::Output, Self::Error> {
        let limits = SandboxLimits {
            timeout: SCRIPT_TIMEOUT,
            max_loop_iterations: LOOP_ITERATION_LIMIT,
        };
        let globals = vec![("records".to_string(), self.records.as_ref().clone())];
        let program = format!(
            "__statingMinorUnits((function () {{\n{}\n}})())",
            args.script
        );
        let outcome = sandbox::execute(PRELUDE, program, globals, limits).await;
        let logged = !outcome.logs.is_empty();
        let mut feedback = match outcome.result {
            Ok(output) => judge(self.gate.as_ref(), &self.attempts, output, logged),
            Err(message) => {
                record_attempt(&self.attempts);
                json!({ "accepted": false, "script_error": message })
            }
        };
        let budget_spent = feedback.get("exploration_budget_spent").is_some();
        if logged && !budget_spent {
            let logs = outcome.logs.join("\n");
            feedback["logs"] = json!(logs.chars().take(MAX_LOG_CHARS).collect::<String>());
        }
        Ok(feedback)
    }
}

fn judge<T>(
    gate: &ExtractionGate<T>,
    attempts: &SharedAttempts<T>,
    output: Value,
    logged: bool,
) -> Value {
    let verdict = gate(&output);
    let Ok(mut log) = attempts.lock() else {
        return json!({ "accepted": false });
    };
    let feedback = match verdict {
        Ok(result) => return log.accept(result),
        Err(feedback) => feedback,
    };
    let exploring = ["lines", "balances", "skipped"].iter().all(|key| {
        output
            .get(key)
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    });
    if exploring && log.explorations < EXPLORATION_BUDGET {
        log.explorations += 1;
        let mut explored = json!({ "accepted": false, "explored": true });
        if !logged && !output.is_null() {
            let returned = match &output {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            explored["returned"] = json!(returned.chars().take(MAX_LOG_CHARS).collect::<String>());
        }
        return explored;
    }
    if exploring {
        log.attempts += 1;
        return json!({
            "accepted": false,
            "exploration_budget_spent": "the next call must return a complete extraction; a partial one is fine, the feedback will guide you"
        });
    }
    log.refuse(output, feedback)
}

fn record_attempt<T>(attempts: &SharedAttempts<T>) {
    if let Ok(mut log) = attempts.lock() {
        log.attempts += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;

    fn gate(accept_when: &'static str) -> Arc<ExtractionGate<Value>> {
        Arc::new(move |output: &Value| {
            if output.get(accept_when).is_some() {
                Ok(output.clone())
            } else {
                Err(json!({ "accepted": false, "problems": [] }))
            }
        })
    }

    fn tool(accept_when: &'static str) -> (RunExtractionScriptTool<Value>, SharedAttempts<Value>) {
        let attempts: SharedAttempts<Value> = Arc::default();
        let records = Arc::new(json!([{ "text": "2024-01-02,Coffee,1.234,50-" }]));
        (
            RunExtractionScriptTool::new(records, gate(accept_when), attempts.clone()),
            attempts,
        )
    }

    async fn run(tool: &RunExtractionScriptTool<Value>, script: &str) -> Value {
        tool.call(
            &mut ToolContext::new(),
            ExtractionScriptArgs {
                script: script.to_string(),
            },
        )
        .await
        .unwrap()
    }

    async fn logged(expressions: &[&str]) -> Vec<Value> {
        let (tool, _) = tool("never");
        let script = format!(
            "log(JSON.stringify([{}])); return {{}};",
            expressions.join(", ")
        );
        let feedback = run(&tool, &script).await;
        serde_json::from_str(feedback["logs"].as_str().unwrap()).unwrap()
    }

    #[tokio::test]
    async fn the_sign_is_read_wherever_it_is_printed() {
        let read = logged(&[
            "toDecimal('€ -12,50', ',')",
            "toDecimal('EUR -12,50', ',')",
            "toDecimal('$-12.50', '.')",
            "toDecimal('CHF -1’234.50', '.')",
            "toDecimal('R$ -1.234,56', ',')",
            "toDecimal('-EUR 12.50', '.')",
            "toDecimal('(12.50)', '.')",
            "toDecimal('$(12.50)', '.')",
            "toDecimal('12,50 €-', ',')",
            "toDecimal('12.50 Dr.', '.')",
            "toDecimal('12.50 CR.', '.')",
            "toDecimal('(USD) 12.50', '.')",
            "toDecimal('-.75', '.')",
            "toDecimal('Fee', '.')",
            "toDecimal('–12,50', ',')",
            "toDecimal('TESCO - 12.50', '.')",
            "toDecimal('12.50 - fee', '.')",
            "toDecimal('Dr. Smith 12.50', '.')",
            "toDecimal('SHOP (LONDON) 12.50 (ref)', '.')",
        ])
        .await;

        assert_eq!(
            read,
            vec![
                json!("-12.50"),
                json!("-12.50"),
                json!("-12.50"),
                json!("-1234.50"),
                json!("-1234.56"),
                json!("-12.50"),
                json!("-12.50"),
                json!("-12.50"),
                json!("-12.50"),
                json!("-12.50"),
                json!("12.50"),
                json!("12.50"),
                json!("-0.75"),
                Value::Null,
                json!("-12.50"),
                json!("12.50"),
                json!("12.50"),
                json!("12.50"),
                json!("12.50"),
            ]
        );
    }

    #[tokio::test]
    async fn exponents_minor_units_and_serial_dates_are_converted_as_text() {
        let read = logged(&[
            "toDecimal('5.0E-5', '.')",
            "toDecimal('-1e-8', '.')",
            "toDecimal('1,5E3', ',')",
            "fromMinorUnits('-350', 2)",
            "fromMinorUnits('5', 2)",
            "fromMinorUnits('100000', 2)",
            "fromMinorUnits('12345', 3)",
            "fromMinorUnits('500', 0)",
            "fromMinorUnits('3.50', 2)",
            "fromSerialDate('45293')",
            "fromSerialDate(45293.75)",
            "fromSerialDate('Date')",
        ])
        .await;

        assert_eq!(
            read,
            vec![
                json!("0.000050"),
                json!("-0.00000001"),
                json!("1500"),
                json!("-3.50"),
                json!("0.05"),
                json!("1000.00"),
                json!("12.345"),
                json!("500"),
                Value::Null,
                json!("2024-01-02"),
                json!("2024-01-02"),
                Value::Null,
            ]
        );
    }

    #[tokio::test]
    async fn quantities_are_subtracted_exactly() {
        let read = logged(&[
            "minus('-251.50', '-1.50')",
            "minus('120.00', '-0.75')",
            "minus(toDecimal('8.50', '.'), '-1.50')",
            "minus('0.3', '0.1')",
            "minus('1', '0.00000001')",
            "minus('-.5', '2')",
            "minus('1234567890123456789.01', '0.02')",
            "minus('5.00', '5')",
        ])
        .await;

        assert_eq!(
            read,
            vec![
                json!("-250.00"),
                json!("120.75"),
                json!("10.00"),
                json!("0.2"),
                json!("0.99999999"),
                json!("-2.5"),
                json!("1234567890123456788.99"),
                json!("0.00"),
            ]
        );

        let (tool, _) = tool("never");
        let feedback = run(&tool, "return { lines: [minus(null, '1.17')] };").await;
        assert!(feedback["script_error"]
            .as_str()
            .unwrap()
            .contains("minus(a, b) needs two decimal strings"));
    }

    #[tokio::test]
    async fn a_missing_decimal_separator_is_an_error_not_a_default() {
        let (tool, _) = tool("never");
        let feedback = run(&tool, "return { lines: [toDecimal('12,50')] };").await;

        assert!(feedback["script_error"]
            .as_str()
            .unwrap()
            .contains("decimalSeparator"));
    }

    #[tokio::test]
    async fn reading_minor_units_through_the_helper_states_it_in_the_output() {
        let (tool, attempts) = tool("lines");
        run(
            &tool,
            "return { lines: [fromMinorUnits('350', 2)], balances: [] };",
        )
        .await;
        assert_eq!(
            attempts.lock().unwrap().accepted.take().unwrap(),
            json!({ "lines": ["3.50"], "balances": [], "minor_units": true })
        );

        run(&tool, "return { lines: [toDecimal('350', '.')] };").await;
        assert_eq!(
            attempts.lock().unwrap().accepted.take().unwrap(),
            json!({ "lines": ["350"] })
        );
    }

    #[tokio::test]
    async fn exploration_is_budgeted_and_then_loses_its_logs() {
        let (tool, attempts) = tool("never");
        for _ in 0..EXPLORATION_BUDGET {
            let feedback = run(&tool, "log('looking'); return { skipped: [] };").await;
            assert_eq!(feedback["explored"], json!(true));
            assert_eq!(feedback["logs"], json!("looking"));
        }
        assert_eq!(attempts.lock().unwrap().attempts, 0);
        let feedback = run(&tool, "log('looking'); return {};").await;

        assert!(feedback.get("exploration_budget_spent").is_some());
        assert!(feedback.get("logs").is_none());
        assert_eq!(attempts.lock().unwrap().attempts, 1);
    }

    #[tokio::test]
    async fn an_exploration_that_logs_nothing_gets_back_what_it_returned() {
        let (tool, attempts) = tool("never");

        let feedback = run(&tool, "return records.map((r) => r.text.split(',')[1]);").await;
        assert_eq!(
            feedback,
            json!({ "accepted": false, "explored": true, "returned": "[\"Coffee\"]" })
        );
        let feedback = run(&tool, "return records.length + ' record';").await;
        assert_eq!(feedback["returned"], json!("1 record"));
        assert_eq!(attempts.lock().unwrap().attempts, 0);
    }

    #[tokio::test]
    async fn script_errors_are_reported_and_counted() {
        let (tool, attempts) = tool("never");
        let feedback = run(&tool, "return null.boom;").await;

        assert!(feedback["script_error"]
            .as_str()
            .unwrap()
            .contains("TypeError"));
        assert_eq!(attempts.lock().unwrap().attempts, 1);
    }
}
