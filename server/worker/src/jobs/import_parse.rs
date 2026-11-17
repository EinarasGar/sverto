use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ai::config::AiConfig;
use ai::models::error::AiError;
use ai::models::extraction::{
    ExtractionGate, ExtractionInput, ExtractionOutcome, ExtractionStop, ImageReadingInput,
};
use ai::workflows::statement_extraction;
use async_trait::async_trait;
use business::dtos::ai_error_dto::AiErrorDto;
use business::dtos::imports::import_file_dto::ImportDateOrderDto;
use business::jobs::ImportParseJob;
use business::providers::user_conversation_provider::UserConversationProvider;
use business::providers::user_rate_limiter::UserRateLimiter;
use business::rate_limiting::rate_limiter::RateLimiter;
use business::service_collection::file_service::FileService;
use business::service_collection::import_parse_service::ImportParseService;
use business::service_collection::user_service::UsersService;
use business::service_collection::ServiceProviders;
use dal::models::file_models::FileStatus;
use extraction::models::accepted::{AcceptedExtraction, DateOrder};
use extraction::models::error::SourceError;
use extraction::models::finding::Finding;
use extraction::models::output::ExtractionOutput;
use extraction::models::source::{ImportSource, SourceKind};
use rand::RngExt;
use serde_json::Value;
use time::{Date, OffsetDateTime};

use crate::jobs::WorkerJob;
use crate::retry::{self, RetryDecision};

const SLOT_WAIT: Duration = Duration::from_secs(30);
const SLOT_WAIT_JITTER_SECS: u64 = 15;
const UPLOAD_REJECTED: &str = "The upload was rejected, so this file can't be read.";
const NOTHING_FOUND: &str = "No transactions or balances were found in this file.";
const AI_FEATURES_OFF: &str =
    "AI features are turned off, so this file wasn't read. Turn them on in Settings, then retry it.";
const BALANCE_DOES_NOT_FOLLOW: &str = "This file couldn't be read: its rows and the running balance printed beside them don't add up. A row may be missing from the file, or the balance may have been adjusted without a row for it.";

#[async_trait]
impl WorkerJob for ImportParseJob {
    const NAME: &'static str = "import_parse";

    fn decide(error: &anyhow::Error, attempts: i32) -> RetryDecision {
        if matches!(retry::extract_ai_error(error), AiError::ConcurrencyLimited) {
            let jitter = rand::rng().random_range(0..=SLOT_WAIT_JITTER_SECS);
            return RetryDecision::RetryAfter(SLOT_WAIT + Duration::from_secs(jitter));
        }
        retry::default_decision(error, attempts, &Self::retry_policy())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(file_id = %self.file_id))]
    async fn before_run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        ImportParseService::new(providers)
            .start(self.session_id, self.file_id, self.user_id)
            .await
    }

    #[tracing::instrument(level = "info", skip_all, fields(session_id = %self.session_id, file_id = %self.file_id, user_id = %self.user_id))]
    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        let service = ImportParseService::new(providers);
        let Some(file) = service
            .processing_file(self.session_id, self.file_id, self.user_id)
            .await?
        else {
            return Ok(());
        };
        let date_order = file.date_order.map(|date_order| match date_order {
            ImportDateOrderDto::DayFirst => DateOrder::DayFirst,
            ImportDateOrderDto::MonthFirst => DateOrder::MonthFirst,
        });
        if !UsersService::new(providers)
            .is_ai_features_enabled(self.user_id)
            .await?
        {
            return fail(&service, self, AI_FEATURES_OFF.to_string()).await;
        }

        let file_service = FileService::new(providers);
        let upload = file_service.get_file(self.user_id, self.file_id).await?;
        if FileStatus::try_from(upload.status.as_str()).map_err(anyhow::Error::msg)?
            == FileStatus::Failed
        {
            return fail(&service, self, UPLOAD_REJECTED.to_string()).await;
        }
        let uploaded_on = upload.created_at.date();
        let bytes = file_service
            .download_file(self.user_id, self.file_id)
            .await?;
        let source = match tokio::task::spawn_blocking(move || ImportSource::read(&bytes)).await? {
            Ok(source) => Arc::new(source),
            Err(error) => return fail(&service, self, source_failure_message(&error)).await,
        };
        let read_again = date_order.is_some();

        let ai_providers = providers.services.create_providers();
        let conversation = Arc::new(
            UserConversationProvider::open(&ai_providers, self.user_id, file.conversation_id)
                .await?,
        );
        let rate_limit = Arc::new(UserRateLimiter::new(
            RateLimiter::new(ai_providers.redis.clone(), ai_providers.db.clone()),
            self.user_id,
        ));
        let config = AiConfig::try_from_env()?;
        let refused_on_the_balance_alone = Arc::new(AtomicBool::new(false));
        let gate = gate(
            source.clone(),
            refused_on_the_balance_alone.clone(),
            date_order,
            uploaded_on,
        );
        let run = match source.kind {
            SourceKind::Image => {
                let input = ImageReadingInput {
                    file_id: self.file_id,
                    uploaded_on,
                    read_again,
                };
                statement_extraction::read_image(&config, conversation, rate_limit, input, gate)
                    .await
            }
            _ => {
                let input = ExtractionInput {
                    sample: source.sample(),
                    records: source.script_records(),
                    read_again,
                };
                statement_extraction::extract(&config, conversation, rate_limit, input, gate).await
            }
        };
        let run = match run {
            Ok(run) => run,
            Err(error) => {
                let detached = service
                    .processing_file(self.session_id, self.file_id, self.user_id)
                    .await?
                    .is_none();
                if detached {
                    tracing::info!(%error, "discarded the parse of a file that changed while it was read");
                    return Ok(());
                }
                return Err(error.into());
            }
        };
        tracing::info!(
            attempts = run.attempts,
            input_tokens = run.usage.input,
            cached_input_tokens = run.usage.cached_input,
            output_tokens = run.usage.output,
            "statement extraction finished"
        );

        match run.outcome {
            ExtractionOutcome::Accepted(accepted)
                if accepted.lines.is_empty() && accepted.assertions.is_empty() =>
            {
                fail(&service, self, NOTHING_FOUND.to_string()).await
            }
            ExtractionOutcome::Accepted(accepted) => {
                service
                    .save(self.session_id, self.file_id, self.user_id, &accepted)
                    .await
            }
            ExtractionOutcome::NotAccepted(_)
                if refused_on_the_balance_alone.load(Ordering::Relaxed) =>
            {
                fail(&service, self, BALANCE_DOES_NOT_FOLLOW.to_string()).await
            }
            ExtractionOutcome::NotAccepted(stop) => {
                fail(&service, self, not_accepted_message(stop, run.attempts)).await
            }
        }
    }

    async fn on_failure(
        &self,
        providers: &ServiceProviders,
        decision: &RetryDecision,
        error: &AiError,
    ) {
        let service = ImportParseService::new(providers);
        let result = match decision {
            RetryDecision::RetryAfter(delay) => {
                service
                    .pause(
                        self.session_id,
                        self.file_id,
                        self.user_id,
                        OffsetDateTime::now_utc() + *delay,
                    )
                    .await
            }
            RetryDecision::Abort => {
                fail(
                    &service,
                    self,
                    failure_message(&AiErrorDto::from(error.clone())),
                )
                .await
            }
        };
        if let Err(e) = result {
            tracing::warn!(
                file_id = %self.file_id,
                error = ?e,
                error.type = "import_parse_status",
                "failed to record the import file's parse status"
            );
        }
    }
}

async fn fail(
    service: &ImportParseService,
    job: &ImportParseJob,
    message: String,
) -> anyhow::Result<()> {
    service
        .fail(job.session_id, job.file_id, job.user_id, message)
        .await
}

fn gate(
    source: Arc<ImportSource>,
    refused_on_the_balance_alone: Arc<AtomicBool>,
    date_order: Option<DateOrder>,
    uploaded_on: Date,
) -> Arc<ExtractionGate<AcceptedExtraction>> {
    Arc::new(move |value: &Value| {
        refused_on_the_balance_alone.store(false, Ordering::Relaxed);
        let output = ExtractionOutput::from_json(value.clone())
            .map_err(|report| report.feedback(&source, &ExtractionOutput::default()))?;
        let evaluated = match date_order {
            Some(date_order) => output.evaluate_in_date_order(&source, uploaded_on, date_order),
            None => output.evaluate(&source, uploaded_on),
        };
        evaluated.map_err(|report| {
            let balance_alone = report
                .findings
                .iter()
                .filter(|finding| finding.is_blocking())
                .all(|finding| {
                    matches!(
                        finding,
                        Finding::BalanceChainBroken { .. } | Finding::BalanceColumnIgnored { .. }
                    )
                });
            refused_on_the_balance_alone.store(balance_alone, Ordering::Relaxed);
            report.feedback(&source, &output)
        })
    })
}

fn source_failure_message(error: &SourceError) -> String {
    match error {
        SourceError::ImageFormat => {
            "This kind of image can't be read. Save it as PNG or JPEG and add it again.".to_string()
        }
        other => other.to_string(),
    }
}

fn not_accepted_message(stop: ExtractionStop, attempts: u32) -> String {
    match stop {
        ExtractionStop::NothingFound => NOTHING_FOUND.to_string(),
        ExtractionStop::NoProgress => {
            "This file couldn't be read: the AI kept returning the same result.".to_string()
        }
        ExtractionStop::AttemptLimit | ExtractionStop::TurnLimit => {
            format!("This file couldn't be read after {attempts} attempts.")
        }
        ExtractionStop::ModelFinished => "This file couldn't be read.".to_string(),
        ExtractionStop::CutOff => {
            "This document has too many rows to read from a picture at once. Split it into smaller files and add them again.".to_string()
        }
    }
}

fn failure_message(error: &AiErrorDto) -> String {
    match error {
        AiErrorDto::RateLimited { .. } => "Your AI usage limit was reached. Retry this file later.",
        AiErrorDto::ConcurrencyLimited => {
            "Other AI work kept this file waiting too long. Retry it."
        }
        AiErrorDto::InputTooLarge => {
            "This file's reading history grew too long to continue. Remove the file and add it again."
        }
        _ => "Something went wrong while reading this file. Retry it.",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_waits_for_a_slot_with_jitter_however_many_attempts_it_took() {
        let error = anyhow::Error::from(AiError::ConcurrencyLimited);

        let RetryDecision::RetryAfter(delay) = ImportParseJob::decide(&error, 40) else {
            panic!("a busy slot should be waited for");
        };
        assert!(
            (SLOT_WAIT..=SLOT_WAIT + Duration::from_secs(SLOT_WAIT_JITTER_SECS)).contains(&delay)
        );
    }

    #[test]
    fn a_reading_refused_on_the_balance_alone_is_flagged_for_the_failure_message() {
        let source = Arc::new(
            ImportSource::read(
                b"Date,Description,Amount,Balance\n\
                  2024-01-02,Shop,-10.00,990.00\n\
                  2024-01-03,Shop,-20.00,975.00\n",
            )
            .unwrap(),
        );
        let flag = Arc::new(AtomicBool::new(false));
        let gate = gate(
            source,
            flag.clone(),
            None,
            time::macros::date!(2024 - 01 - 31),
        );
        let line = |record: usize, day: &str, amount: &str, balance: &str| {
            serde_json::json!({"refs": [record], "date": day, "date_text": day, "account_ref": "A",
                "legs": [{"asset_ref": "EUR", "quantity": amount, "amount_text": amount}],
                "balances": [{"asset_ref": "EUR", "quantity": balance, "amount_text": balance}]})
        };
        let output = |second_amount: &str| {
            serde_json::json!({"lines": [
                line(1, "2024-01-02", "-10.00", "990.00"),
                line(2, "2024-01-03", second_amount, "975.00"),
            ], "skipped": [{"refs": [0], "class": "header"}]})
        };

        assert!(gate(&output("-20.00")).is_err());
        assert!(flag.load(Ordering::Relaxed));
        assert!(gate(&output("-15.00")).is_err());
        assert!(!flag.load(Ordering::Relaxed));
    }
}
