use ai::models::error::AiError;
use async_trait::async_trait;
use business::dtos::conflict_error_dto::BusinessConflictError;
use business::jobs::SyncConnectorBindingJob;
use business::service_collection::connector_sync_service::ConnectorSyncService;
use business::service_collection::ServiceProviders;

use crate::jobs::WorkerJob;
use crate::retry::RetryPolicy;

#[async_trait]
impl WorkerJob for SyncConnectorBindingJob {
    const NAME: &'static str = "sync_connector_binding";

    fn retry_policy() -> RetryPolicy {
        RetryPolicy::standard()
    }

    fn classify_error(error: &anyhow::Error) -> AiError {
        if error.downcast_ref::<BusinessConflictError>().is_some() {
            AiError::ConcurrencyLimited
        } else {
            AiError::ProviderUnavailable {
                detail: format!("{error:#}"),
            }
        }
    }

    #[tracing::instrument(level = "info", skip_all, fields(binding_id = %self.binding_id, user_id = %self.user_id))]
    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        let sync_svc = ConnectorSyncService::new(providers);
        sync_svc.sync_binding(self.user_id, self.binding_id).await?;
        Ok(())
    }
}
