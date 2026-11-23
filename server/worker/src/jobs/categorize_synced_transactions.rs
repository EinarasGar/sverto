use ai::models::classification::{ClassificationLine, ClassificationOutcome};
use async_trait::async_trait;
use business::jobs::CategorizeSyncedTransactionsJob;
use business::providers::fresh_conversations::FreshConversations;
use business::service_collection::bank_ingestion_service::BankIngestionService;
use business::service_collection::category_service::CategoryService;
use business::service_collection::user_service::UsersService;
use business::service_collection::ServiceProviders;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::jobs::line_classification::{self, offered_categories, typing};
use crate::jobs::WorkerJob;

const GLOBAL_CATEGORIES: u64 = 1000;

#[async_trait]
impl WorkerJob for CategorizeSyncedTransactionsJob {
    const NAME: &'static str = "categorize_synced_transactions";

    #[tracing::instrument(level = "info", skip_all, fields(user_id = %self.user_id, count = self.transaction_ids.len()))]
    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        if !UsersService::new(providers)
            .is_ai_features_enabled(self.user_id)
            .await?
        {
            tracing::info!("AI features are off: synced transactions keep their category");
            return Ok(());
        }
        let bank_ingestion = BankIngestionService::new(providers);
        let lines = bank_ingestion
            .synced_transactions_to_categorize(self.user_id, self.transaction_ids.clone())
            .await?;
        if lines.is_empty() {
            tracing::info!("the synced transactions were edited or categorized already");
            return Ok(());
        }
        let category_service = CategoryService::new(providers);
        let mut categories = category_service
            .search_categories(0, GLOBAL_CATEGORIES, None, None)
            .await?
            .results;
        categories.extend(
            category_service
                .get_all_user_categories(self.user_id)
                .await?,
        );
        let categories = offered_categories(&categories);
        let classification_lines: Vec<ClassificationLine> = lines
            .iter()
            .map(|line| ClassificationLine {
                description: line.description.clone(),
                amount: line.quantity,
                currency: line.ticker.clone(),
                account: line.account_name.clone(),
                has_fee: false,
            })
            .collect();

        let ai_providers = providers.services.create_providers();
        let conversations = FreshConversations::new(ai_providers.clone(), self.user_id);
        let classified = line_classification::classify(
            &ai_providers,
            self.user_id,
            &conversations,
            &classification_lines,
            &categories,
        )
        .await?;
        let ClassificationOutcome::Classified(run) = classified else {
            anyhow::bail!("the classification stopped, but nothing withdraws a sync's rows");
        };

        let categorized: Vec<(Uuid, i32)> = lines
            .iter()
            .zip(run.lines)
            .filter_map(|(line, classification)| {
                let (_, category_id) = typing(&classification?, line.quantity > Decimal::ZERO);
                Some((line.transaction_id, category_id?))
            })
            .collect();
        let confident = categorized.len();
        let updated = bank_ingestion
            .categorize_synced_transactions(self.user_id, categorized)
            .await?;
        tracing::info!(
            lines = lines.len(),
            confident,
            updated,
            input_tokens = run.usage.input,
            output_tokens = run.usage.output,
            "synced transactions categorized"
        );
        if let Err(error) = conversations.delete().await {
            tracing::warn!(error = ?error, "deleting the categorisation conversations failed");
        }
        Ok(())
    }
}
