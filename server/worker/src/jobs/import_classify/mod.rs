mod classification;

use std::sync::Arc;

use ai::config::{AiConfig, ImportClassifier, SystemOneConfig};
use ai::models::classification::{ClassificationLine, ClassificationOutcome};
use ai::models::error::AiError;
use ai::workflows::line_classification;
use async_trait::async_trait;
use business::dtos::ai_error_dto::AiErrorDto;
use business::jobs::ImportClassifyJob;
use business::providers::import_run_conversations::ImportRunConversations;
use business::providers::user_rate_limiter::UserRateLimiter;
use business::rate_limiting::rate_limiter::RateLimiter;
use business::service_collection::category_service::CategoryService;
use business::service_collection::import_classify_service::ImportClassifyService;
use business::service_collection::user_service::UsersService;
use business::service_collection::ServiceProviders;
use uuid::Uuid;

use classification::FileClassification;

use crate::jobs::WorkerJob;
use crate::retry::RetryDecision;

const GLOBAL_CATEGORIES: u64 = 1000;

#[async_trait]
impl WorkerJob for ImportClassifyJob {
    const NAME: &'static str = "import_classify";

    #[tracing::instrument(level = "info", skip_all, fields(session_id = %self.session_id, run_id = %self.run_id, user_id = %self.user_id))]
    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        let service = ImportClassifyService::new(providers);
        if !UsersService::new(providers)
            .is_ai_features_enabled(self.user_id)
            .await?
        {
            return service
                .fail(
                    self.session_id,
                    self.run_id,
                    self.user_id,
                    "AI features are turned off, so the transactions weren't categorized. Turn them on in Settings, then retry.".to_string(),
                )
                .await;
        }
        let lines = service.parsed_lines(self.session_id, self.user_id).await?;
        let category_service = CategoryService::new(providers);
        let global_categories = category_service
            .search_categories(0, GLOBAL_CATEGORIES, None, None)
            .await?
            .results;
        let user_categories = category_service
            .get_all_user_categories(self.user_id)
            .await?;

        let mut classification = FileClassification::new();
        classification.add_lines(lines);
        classification.add_categories(global_categories);
        classification.add_categories(user_categories);
        let (line_ids, classification_lines): (Vec<Uuid>, Vec<ClassificationLine>) =
            classification.classification_lines().into_iter().unzip();
        let categories = classification.offered_categories();

        let ai_providers = providers.services.create_providers();
        let conversations = ImportRunConversations::new(
            ai_providers.clone(),
            self.session_id,
            self.run_id,
            self.user_id,
        );
        let classified = match ImportClassifier::try_from_env()? {
            ImportClassifier::Llm => {
                let rate_limit = Arc::new(
                    UserRateLimiter::new(
                        RateLimiter::new(ai_providers.redis.clone(), ai_providers.db.clone()),
                        self.user_id,
                    )
                    .waiting_for_slot(),
                );
                line_classification::llm_classify(
                    &AiConfig::try_from_env()?,
                    &conversations,
                    rate_limit,
                    &classification_lines,
                    &categories,
                )
                .await?
            }
            ImportClassifier::SystemOne => {
                line_classification::system_one_classify(
                    &SystemOneConfig::try_from_env()?,
                    &conversations,
                    &classification_lines,
                    &categories,
                )
                .await?
            }
        };
        let run = match classified {
            ClassificationOutcome::Classified(run) => run,
            ClassificationOutcome::Stopped => {
                tracing::info!(
                    "stopped: this run is no longer the session's current classification"
                );
                return Ok(());
            }
        };
        tracing::info!(
            groups = run.groups,
            classified = run.lines.iter().flatten().count(),
            confident = run
                .lines
                .iter()
                .flatten()
                .filter(|line| line.confident)
                .count(),
            input_tokens = run.usage.input,
            output_tokens = run.usage.output,
            "import lines classified"
        );

        classification.add_classifications(&line_ids, run.lines);
        service
            .save(
                self.session_id,
                self.run_id,
                self.user_id,
                classification.typings(),
            )
            .await
    }

    async fn on_failure(
        &self,
        providers: &ServiceProviders,
        decision: &RetryDecision,
        error: &AiError,
    ) {
        if matches!(decision, RetryDecision::RetryAfter(_)) {
            return;
        }
        let message = match AiErrorDto::from(error.clone()) {
            AiErrorDto::RateLimited { .. } => "Your AI usage limit was reached. Retry later.",
            AiErrorDto::ConcurrencyLimited => {
                "Other AI work kept the classification waiting too long. Retry it."
            }
            _ => "Something went wrong while categorizing the transactions. Retry it.",
        };
        if let Err(e) = ImportClassifyService::new(providers)
            .fail(
                self.session_id,
                self.run_id,
                self.user_id,
                message.to_string(),
            )
            .await
        {
            tracing::warn!(
                run_id = %self.run_id,
                error = ?e,
                error.type = "import_classification_status",
                "failed to record the classification's failure"
            );
        }
    }
}
