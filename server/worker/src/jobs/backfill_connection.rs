use async_trait::async_trait;
use business::jobs::BackfillConnectionJob;
use business::service_collection::connector_sync_service::ConnectorSyncService;
use business::service_collection::ServiceProviders;

use crate::jobs::WorkerJob;

#[async_trait]
impl WorkerJob for BackfillConnectionJob {
    const NAME: &'static str = "backfill_connection";

    #[tracing::instrument(level = "info", skip_all, fields(connection_id = %self.connection_id, user_id = %self.user_id))]
    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        ConnectorSyncService::new(providers)
            .run_attended_backfill(self.user_id, self.connection_id)
            .await
    }
}
