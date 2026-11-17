use std::sync::Arc;

use rig::agent::AgentBuilder;
use rig::completion::{CacheRetention, Effort};

use crate::config::AiConfig;
use crate::conversation::Conversation;
use crate::conversation_provider::ConversationProvider;
use crate::history;
use crate::models::agent::ModelAgent;
use crate::models::chat::{ChatTurn, Persistence, TokenUsage};
use crate::models::error::AiError;
use crate::models::extraction::{
    ExtractionGate, ExtractionInput, ExtractionOutcome, ExtractionRun, ExtractionStop,
    ImageReadingInput, SharedAttempts,
};
use crate::provider::completion_model;
use crate::rate_limit_provider::RateLimitProvider;
use crate::tools::run_extraction_script::{RunExtractionScriptTool, EXPLORATION_BUDGET};
use crate::tools::submit_reading::SubmitReadingTool;

mod hook;

use hook::ExtractionHook;

const MAX_ATTEMPTS: u32 = 5;
const LONG_SOURCE_RECORDS: usize = 500;
const LONG_SOURCE_EXTRA_ATTEMPTS: u32 = 3;
const MAX_OUTPUT_TOKENS: u64 = 8192;
const MAX_IMAGE_OUTPUT_TOKENS: u64 = 32_768;
const CONTINUE: &str = "Continue from the last result: call run_script with a complete extraction.";
const CONTINUE_IMAGE: &str =
    "Continue from the last result: call submit_reading with a complete reading.";
const READ_AGAIN: &str =
    "Read the file again: call run_script with the script that was accepted, unchanged.";
const READ_IMAGE_AGAIN: &str =
    "Read the document again: call submit_reading with the reading that was accepted, unchanged.";

const SYSTEM_PROMPT: &str = r#"You turn one financial document into structured data by writing JavaScript. The global `records` holds the whole document: records[i] is {text, fields?, columns?, sheet?, row?, page?} and i is that record's ref; `fields` (strings) are its separate values. The sample opens with notes on how this file was read: follow them. You see a sample; your script runs on every record. Call run_script with a function body that returns the output:

{lines:[{refs, date, date_text, date_refs?, time?, description?, account_ref, transaction_key?, external_id?, related_asset_ref?, legs:[{asset_ref, quantity, amount_text, fee_type?}], balances?:[{asset_ref, quantity, amount_text}]}],
 balances:[{refs, kind, account_ref, asset_ref, as_of_date, date_text, quantity, amount_text}],
 control_totals:[{refs, kind, account_ref, asset_ref?, quantity, amount_text}],
 period?:{refs, from, from_text, to, to_text},
 skipped:[{refs, class, reason?}]}

Rules:
- refs are record indexes. Every record belongs to exactly one item, except that lines sharing a transaction_key may share records and the period may cite a record that also holds a line. A record that is no item is skipped with class "header", "footer" or "noise"; a row left out is "excluded", never noise.
- A line is one transaction: a document entry that changed what an account holds. An entry marked pending, declined, reverted or failed changed nothing, and neither did a row that only informs or summarises without stating a balance or a control total: skip those with class "excluded" and the reason. An entry that itemises the payments it bundles is one line for the booked total. A row moving money between two accounts gives two lines, one per account, sharing a transaction_key.
- A leg is one asset's change in the account's holdings: a trade has a cash leg and a units leg. An amount printed for information (the original-currency amount, an exchange rate, a unit price, a market value, a tax in a currency the account does not hold) is not a leg. A fee or tax on the same row is another leg with fee_type "transaction", "exchange" or "withholding_tax"; on its own row it is its own line sharing the entry's transaction_key. A fee is never counted twice: when the printed amount already accounts for it (a total with the fee in it, a dividend net of tax), the main leg cites that amount and its quantity is minus(amount, fee), so a total of -251.50 that includes a fee of -1.50 gives legs of -250.00 and -1.50.
- quantity is a decimal string: the change in what the account holder owns, negative when money or an asset leaves the account. On a statement of money owed (credit card, loan, mortgage) purchases, charges and interest are negative, repayments positive, and the printed balance owed is a negative balance.
- amount_text and date_text are copied exactly from the cited records. toDecimal(text, decimalSeparator) reads a printed amount, sign included, and returns a decimal string: never do arithmetic on one, minus(a, b) subtracts and minus("0", a) negates. When the document prints money as whole minor units (350 for 3.50), use fromMinorUnits(text, decimals) for every amount and balance of that currency. Never compute an amount otherwise.
- date is "YYYY-MM-DD", built from the text, never with new Date(). When a row prints two dates, use the booking (posting) date. When a row's date is printed elsewhere (once per day, or in a heading), put that record in date_refs. A date printed without its year takes the year from the statement period, so state the period; rows that run past a New Year change year with it. time is the printed time on the 24-hour clock, with seconds when printed ("14:05:09"); a timestamp gives the date and time it prints, never shifted by its UTC offset. external_id is the document's own transaction id, when printed.
- description is the row's free text verbatim; join separate text columns with " — ".
- account_ref names the account as the document does (IBAN, number or name), identically on every line of that account. When the document holds several accounts, give each record the account of its own sheet, or of the heading or account record it sits under. asset_ref is the ISO currency code for money. For a security it is the ISIN when the document prints one for it anywhere, followed by a space and the ticker when one is printed too ("US0378331005 AAPL"), otherwise the ticker or name as printed, identically on every line and balance of that security. related_asset_ref: the security a line concerns without a leg (dividend, withholding).
- A running balance printed on a row goes in that line's balances, on every line whose row prints one: never leave a balance off to pass. Top-level balances only for balances stated outright: opening (dated the first day covered), closing (the last day), snapshot (any other dated balance, such as one brought or carried forward between rows), value_total (a market value). A holding is stated in units: a positions row gives a snapshot of the quantity held, dated the document's as-of date, and its market value is a value_total beside it, never instead.
- control_totals only when printed: debit_total and credit_total (positive magnitudes), net_total (sum of signed amounts), row_count.
- Every call costs: make the first call a complete extraction built from the sample. Use the feedback, and log() only when it is unclear why, to fix it. The script holds only code, no comments. Never hard-code values you can read."#;

const IMAGE_PROMPT: &str = r#"You read one financial document from the attached picture: a screenshot of a banking app, a photo of a statement, or a scanned PDF. Call submit_reading with what it shows:

{lines:[{date, date_text, time?, description?, account_ref, transaction_key?, external_id?, related_asset_ref?, legs:[{asset_ref, quantity, amount_text, fee_type?}], balances?:[{asset_ref, quantity, amount_text}]}],
 balances:[{kind, account_ref, asset_ref, as_of_date, date_text, quantity, amount_text}],
 control_totals:[{kind, account_ref, asset_ref?, quantity, amount_text}],
 period?:{from, from_text, to, to_text}}

Rules:
- Read every row on every page, in the order the document shows them. A row cut off at an edge counts only when its date and amount are both visible.
- A line is one transaction: an entry that changed what an account holds. Leave out an entry marked pending, declined, reverted or failed, and a row that only informs or summarises without stating a balance or a control total. An entry that itemises the payments it bundles is one line for the booked total. A row moving money between two accounts gives two lines, one per account, sharing a transaction_key.
- A leg is one asset's change in the account's holdings: a trade has a cash leg and a units leg. An amount shown for information (the original-currency amount, an exchange rate, a unit price, a market value) is not a leg. A fee or tax on the same row is another leg with fee_type "transaction", "exchange" or "withholding_tax"; on its own row it is its own line sharing the entry's transaction_key. A fee is never counted twice: when the shown amount already includes it, the main leg's quantity is that amount less the fee.
- quantity is a decimal string such as "-12.34": the change in what the account holder owns, negative when money or an asset leaves the account. Where only a colour, an icon or a column shows which way the money moved, the sign follows it. On a statement of money owed (credit card, loan, mortgage) purchases, charges and interest are negative, repayments positive, and the balance owed is a negative balance.
- amount_text and date_text are copied exactly as shown, signs and currency symbols included. quantity is the shown amount, never one you worked out.
- date is "YYYY-MM-DD". When a row's date is shown once per day or in a heading, date_text is that heading. A day named rather than dated (Today, Yesterday, a weekday) is counted back from the upload date, and date_text is the name. A date shown without its year takes the year from the document's period or its other dates; when nothing in the document shows a year, it is the latest year that doesn't put the date after the upload. time is the shown time on the 24-hour clock. external_id is the document's own transaction id, when shown.
- description is the row's text as shown; join separate pieces of text with " — ".
- account_ref names the account as the document does (IBAN, number or name), identically on every line; when it names none, use a short name of your own. asset_ref is the ISO currency code for money; for a security, its ISIN or ticker as shown. related_asset_ref: the security a line concerns without a leg (dividend, withholding).
- A running balance shown beside a row goes in that line's balances, on every line that shows one. Top-level balances only for balances stated outright: opening (dated the first day covered), closing (the last day), snapshot (any other dated balance), value_total (a market value).
- control_totals only when shown: debit_total and credit_total (positive magnitudes), net_total (sum of signed amounts), row_count.
- When the document holds no transactions or balances, submit empty lines."#;

#[tracing::instrument(level = "debug", skip_all)]
pub async fn extract<C, R, T>(
    config: &AiConfig,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    input: ExtractionInput,
    gate: Arc<ExtractionGate<T>>,
) -> Result<ExtractionRun<T>, AiError>
where
    C: ConversationProvider,
    R: RateLimitProvider,
    T: Send + 'static,
{
    let attempts: SharedAttempts<T> = Arc::default();
    let record_count = input.records.as_array().map_or(0, Vec::len);
    let max_attempts = if record_count > LONG_SOURCE_RECORDS {
        MAX_ATTEMPTS + LONG_SOURCE_EXTRA_ATTEMPTS
    } else {
        MAX_ATTEMPTS
    };

    let agent = AgentBuilder::new(completion_model(config))
        .preamble(SYSTEM_PROMPT)
        .max_tokens(MAX_OUTPUT_TOKENS)
        .default_max_turns((max_attempts + EXPLORATION_BUDGET) as usize + 2)
        .reasoning(Effort::Low)
        .cache(CacheRetention::Short)
        .tool(RunExtractionScriptTool::new(
            Arc::new(input.records),
            gate,
            attempts.clone(),
        ))
        .build();
    let agent = ModelAgent {
        agent,
        model: config.model.clone(),
    };
    let opening = ChatTurn::Message {
        message: format!("{record_count} records.\n{}", input.sample),
        file_ids: Vec::new(),
    };
    run(
        agent,
        conversation,
        rate_limit,
        opening,
        input.read_again.then_some(READ_AGAIN),
        CONTINUE,
        ExtractionHook {
            attempts,
            max_attempts,
            ends_when_cut_off: false,
        },
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all, fields(file_id = %input.file_id))]
pub async fn read_image<C, R, T>(
    config: &AiConfig,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    input: ImageReadingInput,
    gate: Arc<ExtractionGate<T>>,
) -> Result<ExtractionRun<T>, AiError>
where
    C: ConversationProvider,
    R: RateLimitProvider,
    T: Send + 'static,
{
    let attempts: SharedAttempts<T> = Arc::default();
    let agent = AgentBuilder::new(completion_model(config))
        .preamble(IMAGE_PROMPT)
        .max_tokens(MAX_IMAGE_OUTPUT_TOKENS)
        .default_max_turns(MAX_ATTEMPTS as usize + 2)
        .reasoning(Effort::Low)
        .cache(CacheRetention::Short)
        .tool(SubmitReadingTool::new(gate, attempts.clone()))
        .build();
    let agent = ModelAgent {
        agent,
        model: config.model.clone(),
    };
    let opening = ChatTurn::Message {
        message: format!(
            "Read the attached document. It was uploaded on {}.",
            input.uploaded_on
        ),
        file_ids: vec![input.file_id],
    };
    run(
        agent,
        conversation,
        rate_limit,
        opening,
        input.read_again.then_some(READ_IMAGE_AGAIN),
        CONTINUE_IMAGE,
        ExtractionHook {
            attempts,
            max_attempts: MAX_ATTEMPTS,
            ends_when_cut_off: true,
        },
    )
    .await
}

async fn run<C, R, T>(
    agent: ModelAgent,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    opening: ChatTurn,
    read_again: Option<&str>,
    resume: &str,
    hook: ExtractionHook<T>,
) -> Result<ExtractionRun<T>, AiError>
where
    C: ConversationProvider,
    R: RateLimitProvider,
    T: Send + 'static,
{
    let attempts = hook.attempts.clone();
    let saved = history::load(&*conversation).await?;
    let message = |text: &str| ChatTurn::Message {
        message: text.to_string(),
        file_ids: Vec::new(),
    };
    let turn = if saved.messages().is_empty() {
        opening
    } else if let Some(read_again) = read_again {
        message(read_again)
    } else if saved.pending_calls().is_empty() && saved.awaits_model() {
        ChatTurn::Continuation
    } else {
        message(resume)
    };
    let result = Conversation::new(conversation, rate_limit)
        .run_with_hook(agent, turn, Persistence::Persist, hook)
        .await;

    let (accepted, stop, attempt_count) = attempts
        .lock()
        .map(|mut log| (log.accepted.take(), log.stop, log.attempts))
        .unwrap_or((None, None, 0));
    let (unaccepted, usage) = match result {
        Ok(output) => (stop.unwrap_or(ExtractionStop::ModelFinished), output.usage),
        Err(AiError::TurnLimit { .. }) => (ExtractionStop::TurnLimit, TokenUsage::default()),
        Err(error) => return Err(error),
    };
    let outcome = match accepted {
        Some(candidate) => ExtractionOutcome::Accepted(candidate),
        None => ExtractionOutcome::NotAccepted(unaccepted),
    };
    Ok(ExtractionRun {
        outcome,
        attempts: attempt_count,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::models::history::TranscriptEntry;
    use crate::test_support::{gemini_parts, CountingLimit, FakeModel, MemoryConversation, Reply};

    const OPENING: &str = "1 records.\na";

    fn script_call(id: &str, script: &str) -> Reply {
        Reply::Json(json!({
            "candidates": [{"content": {"role": "model", "parts": [
                {"functionCall": {"id": id, "name": "run_script", "args": {"script": script}}}
            ]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 100, "candidatesTokenCount": 20, "totalTokenCount": 120}
        }))
    }

    fn lines_gate(accepted_lines: Value) -> Arc<ExtractionGate<Value>> {
        Arc::new(move |output: &Value| {
            if output["lines"] == accepted_lines {
                Ok(output.clone())
            } else {
                Err(json!({ "accepted": false, "problems": ["wrong line"] }))
            }
        })
    }

    async fn run_on(
        model: &FakeModel,
        conversation: &Arc<MemoryConversation>,
        limit: &Arc<CountingLimit>,
        max_attempts: u32,
    ) -> ExtractionRun<Value> {
        run_again_on(model, conversation, limit, max_attempts, false).await
    }

    async fn run_again_on(
        model: &FakeModel,
        conversation: &Arc<MemoryConversation>,
        limit: &Arc<CountingLimit>,
        max_attempts: u32,
        read_again: bool,
    ) -> ExtractionRun<Value> {
        let attempts: SharedAttempts<Value> = Arc::default();
        let tool = RunExtractionScriptTool::new(
            Arc::new(json!([{ "text": "a" }])),
            lines_gate(json!([1])),
            attempts.clone(),
        );
        let agent = model.agent(|builder| {
            builder
                .default_max_turns(max_attempts as usize + 2)
                .tool(tool)
        });
        run(
            agent,
            conversation.clone(),
            limit.clone(),
            ChatTurn::Message {
                message: OPENING.to_string(),
                file_ids: Vec::new(),
            },
            read_again.then_some(READ_AGAIN),
            CONTINUE,
            ExtractionHook {
                attempts,
                max_attempts,
                ends_when_cut_off: false,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_reading_cut_off_at_the_output_cap_ends_without_being_judged() {
        let model = FakeModel::start(vec![Reply::Json(json!({
            "candidates": [{"content": {"role": "model", "parts": [
                {"functionCall": {"id": "c1", "name": "submit_reading", "args": {}}}
            ]}, "finishReason": "MAX_TOKENS"}],
            "usageMetadata": {"promptTokenCount": 100, "candidatesTokenCount": 20, "totalTokenCount": 120}
        }))])
        .await;
        let attempts: SharedAttempts<Value> = Arc::default();
        let tool = SubmitReadingTool::new(lines_gate(json!([1])), attempts.clone());
        let agent = model.agent(|builder| {
            builder
                .default_max_turns(MAX_ATTEMPTS as usize + 2)
                .tool(tool)
        });

        let run = run(
            agent,
            Arc::new(MemoryConversation::default()),
            Arc::new(CountingLimit::default()),
            ChatTurn::Message {
                message: "Read the attached document.".to_string(),
                file_ids: Vec::new(),
            },
            None,
            CONTINUE_IMAGE,
            ExtractionHook {
                attempts,
                max_attempts: MAX_ATTEMPTS,
                ends_when_cut_off: true,
            },
        )
        .await
        .unwrap();

        assert!(matches!(
            run.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::CutOff)
        ));
        assert_eq!(run.attempts, 0);
    }

    #[tokio::test]
    async fn reading_a_file_again_asks_for_the_extraction_after_an_accepted_one() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { lines: [1] };"),
            script_call("c2", "return { lines: [1] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;
        let again = run_again_on(&model, &conversation, &limit, MAX_ATTEMPTS, true).await;

        assert!(matches!(again.outcome, ExtractionOutcome::Accepted(_)));
        let parts = gemini_parts(&model.requests()[1]);
        assert_eq!(parts.last().unwrap()["text"], READ_AGAIN);
    }

    #[tokio::test]
    async fn an_accepted_script_ends_the_run_and_stays_in_the_history() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { lines: [0] };"),
            script_call("c2", "return { lines: [1] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let run = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;

        assert_eq!(model.requests().len(), 2);
        assert_eq!(run.attempts, 2);
        assert_eq!((run.usage.input, run.usage.output), (200, 40));
        assert_eq!(limit.charged(), vec![(200, 40)]);
        let ExtractionOutcome::Accepted(accepted) = run.outcome else {
            panic!("the second script should be accepted");
        };
        assert_eq!(accepted["lines"], json!([1]));
        let items = history::transcript(&conversation.history()).unwrap();
        assert!(matches!(&items.last().unwrap().entry,
            TranscriptEntry::ToolResult { tool_call_id, content } if tool_call_id == "c2" && content.contains("\"accepted\":true")));
    }

    #[tokio::test]
    async fn a_run_at_the_attempt_cap_is_not_accepted() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { lines: [0] };"),
            script_call("c2", "return { lines: [2] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let run = run_on(&model, &conversation, &limit, 2).await;

        assert_eq!(model.requests().len(), 2);
        assert!(matches!(
            run.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::AttemptLimit)
        ));
    }

    #[tokio::test]
    async fn a_repeated_output_stops_for_lack_of_progress() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { lines: [0] };"),
            script_call("c2", "return { lines: [0] };"),
            script_call("c3", "return { lines: [0] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let run = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;

        assert_eq!(model.requests().len(), 3);
        assert!(matches!(
            run.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::NoProgress)
        ));
    }

    #[tokio::test]
    async fn handing_back_the_same_refused_empty_output_ends_the_run_as_nothing_found() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { skipped: [0] };"),
            script_call("c2", "return { skipped: [0] };"),
            script_call("c3", "return { lines: [1] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let run = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;

        assert_eq!(model.requests().len(), 2);
        assert!(matches!(
            run.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::NothingFound)
        ));
    }

    #[tokio::test]
    async fn a_retried_extraction_continues_the_saved_conversation() {
        let model = FakeModel::start(vec![
            script_call("c1", "return { lines: [0] };"),
            script_call("c2", "return { lines: [1] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let first = run_on(&model, &conversation, &limit, 1).await;
        let second = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;

        assert!(matches!(
            first.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::AttemptLimit)
        ));
        assert!(matches!(second.outcome, ExtractionOutcome::Accepted(_)));
        let parts = gemini_parts(&model.requests()[1]);
        assert!(parts
            .iter()
            .any(|part| part.get("functionResponse").is_some()));
        assert_eq!(
            parts.iter().filter(|part| part["text"] == OPENING).count(),
            1
        );
    }

    #[tokio::test]
    async fn a_retry_after_the_model_gave_up_nudges_it_to_continue() {
        let model = FakeModel::start(vec![
            Reply::Json(json!({
                "candidates": [{"content": {"role": "model", "parts": [
                    {"text": "I cannot read this."}
                ]}, "finishReason": "STOP"}],
                "usageMetadata": {"promptTokenCount": 100, "candidatesTokenCount": 20, "totalTokenCount": 120}
            })),
            script_call("c1", "return { lines: [1] };"),
        ])
        .await;
        let conversation = Arc::new(MemoryConversation::default());
        let limit = Arc::new(CountingLimit::default());

        let first = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;
        let second = run_on(&model, &conversation, &limit, MAX_ATTEMPTS).await;

        assert!(matches!(
            first.outcome,
            ExtractionOutcome::NotAccepted(ExtractionStop::ModelFinished)
        ));
        assert!(matches!(second.outcome, ExtractionOutcome::Accepted(_)));
        let parts = gemini_parts(&model.requests()[1]);
        assert!(parts.iter().any(|part| part["text"] == CONTINUE));
        assert_eq!(
            parts.iter().filter(|part| part["text"] == OPENING).count(),
            1
        );
    }
}
