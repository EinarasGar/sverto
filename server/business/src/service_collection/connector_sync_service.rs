#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::enums::transaction_types::DatabaseTransactionTypes;
use dal::job_queue::JobQueueHandle;
use dal::models::connector_models::{
    ActiveStoredBindingRow, AddConnectorProviderAccountModel, ConnectorBindingRow,
    ConnectorRawPageRow, ConnectorTransactionRow, PositionTimelineEntryRow,
    UpdateProviderAccountSyncResultModel,
};
use dal::queries::connector_queries;
use dal::query_params::connector_params::{GetConnectorBindingsParams, GetRawPagesParams};
#[mockall_double::double]
use dal::redis_connection::RedisConnection;
use dal::secrets::SecretProvider;

use connectors::client_supplied::{ClientSuppliedConnector, ClientSuppliedStream};
use connectors::models::sync::RawPage;
use connectors::models::transaction::ProviderTransaction;
use connectors::port::{Connector, ConnectorStore, SyncParams, SyncRunOutcome};
use connectors::provider::{CredentialSource, ProviderKind};

use crate::dtos::bad_gateway_error_dto::BusinessBadGatewayError;
use crate::dtos::bad_request_error_dto::BusinessBadRequestError;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::connectors::{
    ActiveStoredBinding, BindingStatusDto, ConnectionStatusDto, ConnectorBindingDto,
    ConnectorConnectionDto, CredentialModeDto, SyncDispatchDto, SyncOutcomeDto, SyncReportDto,
    TransientSyncCredentialDto,
};
use crate::dtos::transaction_dto::{TransactionDto, TransactionTypeDto, TransactionVisibilityDto};
use crate::jobs::SyncConnectorBindingJob;
use crate::providers::connector_store::BusinessConnectorStore;

use crate::entities::connectors::asset_candidate::HeldAssets;
use crate::entities::connectors::connector_transaction_batch::ConnectorTransactionBatch;
use crate::entities::connectors::opening_buys::{
    is_synthetic_external_id, opening_buy_changed, plan_opening_buys, required_opening_buys,
    ExistingOpeningBuy, PositionEvent, RequiredOpeningBuy, SYNTHETIC_OPENING_BUY_PREFIX,
};
use crate::entities::connectors::provider_transaction_import::{
    ProviderTransactionImport, TransactionImportOutcome,
};
use crate::entities::transactions::metadata::ConnectorLinkMeta;
use crate::entities::transactions::transaction::Transaction;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

use super::asset_service::AssetsService;
use super::connector_service::ConnectorService;
use super::portfolio_overview_service::PortfolioOverviewService;
use super::transaction_management_service::TransactionManagementService;
use super::ServiceProviders;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

enum FetchPhaseOutcome {
    Complete,
    Partial {
        pages_fetched: i32,
        next_cursor: Option<serde_json::Value>,
    },
    Failed(anyhow::Error),
}

pub struct ConnectorSyncService {
    db: MyraDb,
    secret_provider: Arc<dyn SecretProvider>,
    redis: RedisConnection,
    queue: JobQueueHandle,
    connectors: ConnectorService,
    transaction_management: TransactionManagementService,
    assets: AssetsService,
    portfolio: PortfolioOverviewService,
}

impl ConnectorSyncService {
    pub fn new(providers: &ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            secret_provider: providers.secret_provider.clone(),
            redis: providers.redis.clone(),
            queue: providers.job_queue.clone(),
            connectors: ConnectorService::new(providers),
            transaction_management: TransactionManagementService::new(providers),
            assets: AssetsService::new(providers),
            portfolio: PortfolioOverviewService::new(providers),
        }
    }

    fn store_for_provider_account(
        &self,
        provider_account_ref: Uuid,
        connection: &ConnectorConnectionDto,
    ) -> anyhow::Result<BusinessConnectorStore> {
        BusinessConnectorStore::for_provider_account(
            self.db.clone(),
            self.secret_provider.clone(),
            self.redis.clone(),
            provider_account_ref,
            connection,
        )
    }

    async fn build_provider_connector(
        &self,
        connection: &ConnectorConnectionDto,
        binding: &ConnectorBindingDto,
        transient_credential: Option<&str>,
    ) -> anyhow::Result<Box<dyn Connector>> {
        let store = self.store_for_provider_account(binding.provider_account_ref, connection)?;
        let credential = match transient_credential {
            Some(secret) => CredentialSource::Transient(secret.to_string()),
            None => CredentialSource::Stored,
        };
        store
            .provider_kind()
            .provider()
            .build_connector(&binding.provider_account_id, credential, &store)
            .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn enqueue_sync(&self, user_id: Uuid, binding_id: Uuid) -> anyhow::Result<()> {
        self.queue
            .push(SyncConnectorBindingJob {
                binding_id,
                user_id,
            })
            .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn sync_binding(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
    ) -> anyhow::Result<SyncReportDto> {
        let (binding, connection) = self.load_active_binding(user_id, binding_id).await?;
        self.claim_provider_account(binding.provider_account_ref)
            .await?;

        let connector = match self
            .build_provider_connector(&connection, &binding, None)
            .await
        {
            Ok(connector) => connector,
            Err(e) => {
                self.release_provider_account_claim(binding.provider_account_ref)
                    .await;
                return Err(e);
            }
        };

        match self
            .run_sync(
                user_id,
                &binding,
                &*connector,
                &connection,
                binding.synced_through,
                None,
                0,
            )
            .await?
        {
            SyncOutcomeDto::Complete { report } => Ok(report),
            SyncOutcomeDto::Failed { error } => Err(anyhow::anyhow!(error)),
            SyncOutcomeDto::Partial { .. } => {
                anyhow::bail!("unbounded walk returned a partial outcome")
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn sync_binding_transient(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
        credential: TransientSyncCredentialDto,
    ) -> anyhow::Result<SyncOutcomeDto> {
        let (binding, connection) = self.load_active_binding(user_id, binding_id).await?;

        match (&credential, connection.credential_mode) {
            (TransientSyncCredentialDto::Transient(_), CredentialModeDto::Transient) => {}
            (
                TransientSyncCredentialDto::ClientSupplied { .. },
                CredentialModeDto::ClientSupplied,
            ) => {}
            _ => anyhow::bail!(
                "credential kind supplied does not match connection {}'s credential_mode",
                connection.id
            ),
        }

        let connector: Box<dyn Connector> = match credential {
            TransientSyncCredentialDto::Transient(secret) => {
                self.build_provider_connector(&connection, &binding, Some(secret.as_str()))
                    .await?
            }
            TransientSyncCredentialDto::ClientSupplied {
                streams,
                raw_balance,
            } => Box::new(ClientSuppliedConnector::new(
                streams
                    .into_iter()
                    .map(|s| ClientSuppliedStream {
                        stream: s.stream,
                        items: s.items,
                    })
                    .collect(),
                raw_balance,
                connection.provider_kind.clone(),
            )),
        };

        self.claim_provider_account(binding.provider_account_ref)
            .await?;

        self.run_sync(
            user_id,
            &binding,
            &*connector,
            &connection,
            binding.synced_through,
            Some(Duration::seconds(20)),
            2,
        )
        .await
    }

    async fn load_active_binding(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
    ) -> anyhow::Result<(ConnectorBindingDto, ConnectorConnectionDto)> {
        let binding = self.connectors.get_binding(user_id, binding_id).await?;
        let connection = self
            .connectors
            .get_connection(user_id, binding.connection_id)
            .await?;

        if binding.status != BindingStatusDto::Active {
            return Err(anyhow::Error::new(BusinessConflictError {
                message: format!(
                    "binding is {} — resume it before syncing",
                    binding.status.as_str()
                ),
            }));
        }

        if connection.status != ConnectionStatusDto::Active {
            return Err(anyhow::Error::new(BusinessConflictError {
                message: format!(
                    "connection is {} — syncing is unavailable",
                    connection.status.as_str()
                ),
            }));
        }

        Ok((binding, connection))
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_sync(
        &self,
        user_id: Uuid,
        binding: &ConnectorBindingDto,
        connector: &dyn Connector,
        connection: &ConnectorConnectionDto,
        synced_through: Option<OffsetDateTime>,
        budget: Option<Duration>,
        inline_retries: u32,
    ) -> anyhow::Result<SyncOutcomeDto> {
        let params = SyncParams {
            synced_through,
            budget,
            inline_retries,
        };

        let fetch_error = match self
            .run_fetch_phase(binding.provider_account_ref, connection, connector, params)
            .await?
        {
            FetchPhaseOutcome::Partial {
                pages_fetched,
                next_cursor,
            } => {
                return Ok(SyncOutcomeDto::Partial {
                    pages_fetched,
                    next_cursor,
                });
            }
            FetchPhaseOutcome::Complete => None,
            FetchPhaseOutcome::Failed(e) => Some(e),
        };

        let report = match self.reconcile_and_commit(user_id, binding, connector).await {
            Ok(report) => report,
            Err(e) => {
                let _ = self.db.rollback_transaction().await;
                return Err(e);
            }
        };

        if let Some(e) = fetch_error {
            if report.pages_projected == 0 {
                return Err(e);
            }
            tracing::warn!(
                error = %e,
                pages_projected = report.pages_projected,
                "fetch failed; projected existing archive instead"
            );
        }

        Ok(SyncOutcomeDto::Complete { report })
    }

    async fn run_fetch_phase(
        &self,
        provider_account_ref: Uuid,
        connection: &ConnectorConnectionDto,
        connector: &dyn Connector,
        params: SyncParams,
    ) -> anyhow::Result<FetchPhaseOutcome> {
        let store = match self.store_for_provider_account(provider_account_ref, connection) {
            Ok(store) => store,
            Err(e) => {
                self.release_provider_account_claim(provider_account_ref)
                    .await;
                return Err(e);
            }
        };
        let walk_started_at = OffsetDateTime::now_utc();

        match connector.sync(&store, params).await {
            Ok(SyncRunOutcome::Partial {
                pages_fetched,
                next_cursor,
            }) => {
                self.db
                    .execute(connector_queries::release_provider_account_fetch_claim(
                        provider_account_ref,
                    ))
                    .await?;
                Ok(FetchPhaseOutcome::Partial {
                    pages_fetched,
                    next_cursor,
                })
            }
            Ok(SyncRunOutcome::Complete { .. }) => {
                self.record_fetch_result(provider_account_ref, Some(walk_started_at), None)
                    .await?;
                Ok(FetchPhaseOutcome::Complete)
            }
            Err(e) => {
                self.record_fetch_result(provider_account_ref, None, Some(e.to_string()))
                    .await?;
                Ok(FetchPhaseOutcome::Failed(e))
            }
        }
    }

    async fn record_fetch_result(
        &self,
        provider_account_ref: Uuid,
        synced_through: Option<OffsetDateTime>,
        error: Option<String>,
    ) -> anyhow::Result<()> {
        let update = UpdateProviderAccountSyncResultModel {
            provider_account_ref,
            last_sync_status: Some(if error.is_some() { "failed" } else { "ok" }.to_string()),
            last_sync_error: error,
            last_sync_at: OffsetDateTime::now_utc(),
            synced_through,
        };
        self.db
            .execute(connector_queries::update_provider_account_sync_result(
                update,
            ))
            .await?;
        Ok(())
    }

    /// Captures full history for every discoverable account and card *while the user is present*
    /// — called from the OAuth-completion handler, inside the SCA window. Open-banking providers
    /// restrict historical (esp. card) data for unattended access, so consent time is the only
    /// reliable moment to backfill. Pages land in the connection/account-scoped archive, so a
    /// binding created later projects the history without re-fetching. Pass-through providers
    /// (`needs_attended_backfill() == false`, e.g. Trading212) skip this.
    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn run_attended_backfill(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<()> {
        let connection = self
            .connectors
            .get_connection(user_id, connection_id)
            .await?;
        let provider_kind: ProviderKind = connection.provider_kind.parse()?;
        if !provider_kind.provider().needs_attended_backfill() {
            return Ok(());
        }

        let accounts = self
            .connectors
            .list_provider_accounts(user_id, connection_id)
            .await?;
        tracing::info!(
            count = accounts.len(),
            "attended backfill: capturing full history for discovered accounts + cards"
        );

        for account in accounts {
            if let Err(e) = self
                .backfill_provider_account(&connection, &account.provider_account_id)
                .await
            {
                tracing::warn!(
                    provider_account_id = %account.provider_account_id,
                    error = ?e,
                    "attended backfill failed for one provider account — continuing"
                );
            }
        }
        Ok(())
    }

    /// Full-history fetch for a single provider account into its archive. No projection — there
    /// are no bindings yet; a later binding projects from these pages.
    async fn backfill_provider_account(
        &self,
        connection: &ConnectorConnectionDto,
        external_account_id: &str,
    ) -> anyhow::Result<()> {
        let provider_account_ref: Uuid = self
            .db
            .fetch_one_scalar(connector_queries::get_or_create_provider_account(
                AddConnectorProviderAccountModel {
                    connection_id: connection.id,
                    external_account_id: external_account_id.to_string(),
                },
            ))
            .await?;

        let claimed = self
            .db
            .execute_with_rows_affected(connector_queries::claim_provider_account_for_fetch(
                provider_account_ref,
            ))
            .await?;
        if claimed == 0 {
            return Ok(());
        }

        let connector = match async {
            let store = self.store_for_provider_account(provider_account_ref, connection)?;
            store
                .provider_kind()
                .provider()
                .build_connector(external_account_id, CredentialSource::Stored, &store)
                .await
        }
        .await
        {
            Ok(connector) => connector,
            Err(e) => {
                self.release_provider_account_claim(provider_account_ref)
                    .await;
                return Err(e);
            }
        };

        let params = SyncParams {
            synced_through: None,
            budget: None,
            inline_retries: 2,
        };
        match self
            .run_fetch_phase(provider_account_ref, connection, &*connector, params)
            .await?
        {
            FetchPhaseOutcome::Failed(e) => Err(e),
            _ => Ok(()),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn dispatch_sync(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
        credential: Option<String>,
    ) -> anyhow::Result<SyncDispatchDto> {
        let (_binding, connection) = self.load_active_binding(user_id, binding_id).await?;

        match connection.credential_mode {
            CredentialModeDto::Stored => {
                self.enqueue_sync(user_id, binding_id).await?;
                Ok(SyncDispatchDto::Queued)
            }
            CredentialModeDto::Transient => {
                let credential = credential.ok_or_else(|| {
                    anyhow::Error::new(BusinessBadRequestError {
                        message: "credential is required for Transient-mode bindings".to_string(),
                    })
                })?;

                let outcome = self
                    .sync_binding_transient(user_id, binding_id, TransientSyncCredentialDto::Transient(credential))
                    .await?;

                match outcome {
                    SyncOutcomeDto::Complete { report } => Ok(SyncDispatchDto::Completed { report }),
                    SyncOutcomeDto::Partial { pages_fetched, .. } => Ok(SyncDispatchDto::Partial { pages_fetched }),
                    SyncOutcomeDto::Failed { error } => {
                        Err(anyhow::Error::new(BusinessBadGatewayError { message: error }))
                    }
                }
            }
            CredentialModeDto::ClientSupplied => Err(anyhow::Error::new(BusinessBadRequestError {
                message: "ClientSupplied-mode bindings cannot be synced via this endpoint — use POST .../ingest instead"
                    .to_string(),
            })),
        }
    }

    async fn release_provider_account_claim(&self, provider_account_ref: Uuid) {
        if let Err(e) = self
            .db
            .execute(connector_queries::release_provider_account_fetch_claim(
                provider_account_ref,
            ))
            .await
        {
            tracing::warn!(
                provider_account_ref = %provider_account_ref,
                error = ?e,
                "failed to release the provider account sync claim"
            );
        }
    }

    async fn claim_provider_account(&self, provider_account_ref: Uuid) -> anyhow::Result<()> {
        let claimed = self
            .db
            .execute_with_rows_affected(connector_queries::claim_provider_account_for_fetch(
                provider_account_ref,
            ))
            .await?;
        if claimed == 0 {
            return Err(anyhow::Error::new(BusinessConflictError {
                message: "a sync is already in progress for this provider account".to_string(),
            }));
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn get_sync_checkpoint(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
    ) -> anyhow::Result<(Option<serde_json::Value>, Option<OffsetDateTime>)> {
        let binding = self.connectors.get_binding(user_id, binding_id).await?;
        let connection = self
            .connectors
            .get_connection(user_id, binding.connection_id)
            .await?;

        let store = self.store_for_provider_account(binding.provider_account_ref, &connection)?;
        if let Some(cursor) = store.latest_cursor().await? {
            return Ok((Some(cursor), None));
        }

        Ok((None, binding.synced_through))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(limit = limit))]
    pub async fn get_active_stored_bindings(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<ActiveStoredBinding>> {
        let query = connector_queries::get_active_stored_bindings(limit);
        let results = self.db.fetch_all::<ActiveStoredBindingRow>(query).await?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding.id))]
    async fn reconcile_and_commit(
        &self,
        user_id: Uuid,
        binding: &ConnectorBindingDto,
        connector: &dyn Connector,
    ) -> anyhow::Result<SyncReportDto> {
        let binding_id = binding.id;
        let provider_account_ref = binding.provider_account_ref;

        let binding_query = connector_queries::get_connector_bindings(
            GetConnectorBindingsParams::by_id(user_id, binding_id),
        );
        let binding_row: ConnectorBindingRow = self.db.fetch_one(binding_query).await?;

        let checkpoint = binding_row.projected_page_id;

        let pages_query = connector_queries::get_connector_raw_pages(
            GetRawPagesParams::by_provider_account_ref(provider_account_ref, checkpoint),
        );
        let raw_pages = self
            .db
            .fetch_all::<ConnectorRawPageRow>(pages_query)
            .await?;
        let max_page_id = raw_pages.last().map(|page| page.id);

        let pages: Vec<RawPage> = raw_pages
            .iter()
            .map(|page| RawPage {
                stream: page.stream.clone(),
                payload: page.payload.0.clone(),
            })
            .collect();
        let map_pages = connector.map_pages(&pages);
        let batch = ConnectorTransactionBatch::from_mapped(map_pages);

        let existing_rows: HashMap<String, ConnectorTransactionRow> = if batch.is_empty() {
            HashMap::new()
        } else {
            self.db
                .fetch_all::<ConnectorTransactionRow>(
                    connector_queries::get_connector_transactions_by_external_ids(
                        binding_id,
                        batch.external_ids(),
                    ),
                )
                .await?
                .into_iter()
                .map(|row| (row.external_id.clone(), row))
                .collect()
        };

        let visibility = match binding_row.write_mode.as_str() {
            "trusted" => TransactionVisibilityDto::Default,
            _ => TransactionVisibilityDto::Ghost,
        };

        self.db.start_transaction().await?;

        let mut report = SyncReportDto {
            duplicates: batch.duplicates(),
            pages_projected: raw_pages.len(),
            ..SyncReportDto::default()
        };

        let classified = batch.classify(&existing_rows);
        report.unchanged = classified.unchanged.len();
        report.conflicts = classified.conflicts.len();
        report.amended = classified.amended.len();

        for tx in &classified.conflicts {
            tracing::warn!(
                binding_id = %binding_id,
                external_id = %tx.external_id,
                "provider amended a user-edited transaction — skipping (user edit wins)"
            );
        }

        for tx in &classified.amended {
            tracing::debug!(
                binding_id = %binding_id,
                external_id = %tx.external_id,
                "provider amended a transaction — content update not yet implemented"
            );
        }

        let to_import = |tx: &ProviderTransaction| {
            ProviderTransactionImport::new(
                tx.clone(),
                binding_row.sverto_account_id,
                visibility,
                binding_id,
            )
        };
        let imports: Vec<ProviderTransactionImport> =
            classified.new.iter().copied().map(to_import).collect();
        // Existing trades are resolved too, so a re-projection reconciles every traded instrument.
        let stored_trades: Vec<ProviderTransactionImport> = classified
            .stored()
            .into_iter()
            .map(to_import)
            .filter(ProviderTransactionImport::is_trade)
            .collect();
        let resolvable: Vec<&ProviderTransactionImport> =
            imports.iter().chain(&stored_trades).collect();

        let mut needed_tickers: HashSet<String> = HashSet::new();
        for import in &imports {
            needed_tickers.insert(import.currency().to_string());
        }
        for import in &resolvable {
            needed_tickers.extend(import.instrument_ticker_candidates());
        }
        let resolved = self.assets.resolve_tickers(user_id, needed_tickers).await?;
        let holdings = self.portfolio.get_holdings(user_id, false).await?;
        let held = HeldAssets::new(&holdings, binding_row.sverto_account_id);

        let isin_key = |import: &ProviderTransactionImport| {
            import
                .isin()
                .map(|isin| (isin.to_string(), import.currency().to_string()))
        };
        // Ticker-matched rows tell which listing an ambiguous ISIN refers to.
        let mut resolved_isins: HashMap<(String, String), i32> = HashMap::new();
        let mut needed_isins: HashSet<(String, String)> = HashSet::new();
        for import in &resolvable {
            let Some(key) = isin_key(import) else {
                continue;
            };
            match import
                .instrument_ticker_candidates()
                .iter()
                .find_map(|candidate| resolved.get(candidate).copied())
            {
                Some(asset_id) => {
                    resolved_isins.entry(key).or_insert(asset_id);
                }
                None => {
                    needed_isins.insert(key);
                }
            }
        }
        needed_isins.retain(|key| !resolved_isins.contains_key(key));
        resolved_isins.extend(
            self.assets
                .resolve_isins(user_id, &held, needed_isins)
                .await?,
        );

        let instrument_asset_ids: Vec<Option<i32>> = resolvable
            .iter()
            .map(|import| {
                import
                    .instrument_ticker_candidates()
                    .iter()
                    .find_map(|candidate| resolved.get(candidate).copied())
                    .or_else(|| isin_key(import).and_then(|key| resolved_isins.get(&key).copied()))
            })
            .collect();

        let root_symbol_key = |import: &ProviderTransactionImport| {
            import
                .root_symbol()
                .map(|root| (root, import.currency().to_string()))
        };
        let needed_root_symbols: HashSet<(String, String)> = resolvable
            .iter()
            .zip(&instrument_asset_ids)
            .filter(|(_, asset_id)| asset_id.is_none())
            .filter_map(|(import, _)| root_symbol_key(import))
            .collect();
        let resolved_root_symbols = self
            .assets
            .resolve_root_symbols(user_id, &held, needed_root_symbols)
            .await?;

        let instrument_asset_ids: Vec<Option<i32>> = resolvable
            .iter()
            .zip(instrument_asset_ids)
            .map(|(import, asset_id)| {
                asset_id.or_else(|| {
                    root_symbol_key(import).and_then(|key| resolved_root_symbols.get(&key).copied())
                })
            })
            .collect();
        let (new_instrument_asset_ids, stored_instrument_asset_ids) =
            instrument_asset_ids.split_at(imports.len());

        let mut entities: Vec<Transaction> = Vec::new();
        let mut traded_asset_ids: HashSet<i32> = stored_instrument_asset_ids
            .iter()
            .flatten()
            .copied()
            .collect();
        for (import, &instrument_asset_id) in imports.iter().zip(new_instrument_asset_ids) {
            let cash_asset_id = resolved.get(import.currency()).copied();

            match import.try_into_transaction(user_id, cash_asset_id, instrument_asset_id)? {
                TransactionImportOutcome::Ready(entity) => {
                    if import.is_trade() {
                        traded_asset_ids.extend(instrument_asset_id);
                    }
                    entities.push(entity);
                }
                TransactionImportOutcome::Unresolvable(reason) => {
                    report.unresolved += 1;
                    tracing::warn!(
                        binding_id = %binding_id,
                        external_id = %import.external_id(),
                        reason,
                        ticker_candidates = ?import.instrument_ticker_candidates(),
                        isin = ?import.isin(),
                        root_symbol = ?import.root_symbol(),
                        currency = %import.currency(),
                        amount = %import.amount(),
                        date = %import.date().date(),
                        "provider transaction not imported — unresolvable"
                    );
                }
            }
        }

        if !entities.is_empty() {
            self.transaction_management
                .add_transactions(&mut entities)
                .await?;
            report.new_transactions = entities.len();
        }

        self.reconcile_opening_buys(
            user_id,
            binding_id,
            binding_row.sverto_account_id,
            traded_asset_ids,
            &mut report,
        )
        .await?;

        // Projection checkpoint advanced on the binding (the fetch outcome is recorded on the
        // provider account by the caller's fetch phase).
        self.db
            .execute(connector_queries::update_binding_projection(
                binding_id,
                max_page_id,
            ))
            .await?;

        self.db.commit_transaction().await?;

        tracing::info!(
            binding_id = %binding_id,
            pages_projected = report.pages_projected,
            new = report.new_transactions,
            unchanged = report.unchanged,
            amended = report.amended,
            conflicts = report.conflicts,
            unresolved = report.unresolved,
            duplicates = report.duplicates,
            opening_buys_created = report.opening_buys_created,
            opening_buys_updated = report.opening_buys_updated,
            opening_buys_deleted = report.opening_buys_deleted,
            "projection committed"
        );
        Ok(report)
    }

    async fn reconcile_opening_buys(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
        account_id: Uuid,
        asset_ids: HashSet<i32>,
        report: &mut SyncReportDto,
    ) -> anyhow::Result<()> {
        if asset_ids.is_empty() {
            return Ok(());
        }

        let rows = self
            .db
            .fetch_all::<PositionTimelineEntryRow>(
                connector_queries::get_position_timeline_entries(
                    binding_id,
                    account_id,
                    asset_ids.into_iter().collect(),
                ),
            )
            .await?;

        let mut existing: Vec<ExistingOpeningBuy> = Vec::new();
        let mut events: Vec<PositionEvent> = Vec::new();
        let mut edited_synthetic: Vec<(Uuid, String)> = Vec::new();
        for row in rows {
            let synthetic_external_id = row
                .external_id
                .clone()
                .filter(|external_id| is_synthetic_external_id(external_id));
            if let Some(external_id) = synthetic_external_id.clone() {
                if row.edited_by_user == Some(false) {
                    if !existing
                        .iter()
                        .any(|buy| buy.transaction_id == row.transaction_id)
                    {
                        existing.push(ExistingOpeningBuy {
                            transaction_id: row.transaction_id,
                            external_id,
                        });
                    }
                    continue;
                }
                edited_synthetic.push((row.transaction_id, external_id));
            }

            let is_binding_sell = row.type_id == DatabaseTransactionTypes::AssetSale
                && row.quantity.is_sign_negative()
                && synthetic_external_id.is_none();
            events.push(PositionEvent {
                asset_id: row.asset_id,
                transaction_id: row.transaction_id,
                date: row.date_transacted,
                quantity: row.quantity,
                sell_external_id: row.external_id.filter(|_| is_binding_sell),
            });
        }

        let required = required_opening_buys(&events);

        if !edited_synthetic.is_empty() {
            let without_edited: Vec<PositionEvent> = events
                .iter()
                .filter(|event| {
                    !edited_synthetic
                        .iter()
                        .any(|(transaction_id, _)| *transaction_id == event.transaction_id)
                })
                .cloned()
                .collect();
            let required_without_edited = required_opening_buys(&without_edited);
            for (_, external_id) in &edited_synthetic {
                if !required_without_edited
                    .iter()
                    .any(|buy| &buy.external_id == external_id)
                {
                    tracing::warn!(
                        binding_id = %binding_id,
                        external_id = %external_id,
                        "user-edited synthetic opening buy is no longer needed — left as is"
                    );
                }
            }
        }

        let required_external_ids: Vec<String> =
            required.iter().map(|buy| buy.external_id.clone()).collect();
        let taken_external_ids: HashSet<String> = if required_external_ids.is_empty() {
            HashSet::new()
        } else {
            self.db
                .fetch_all::<ConnectorTransactionRow>(
                    connector_queries::get_connector_transactions_by_external_ids(
                        binding_id,
                        required_external_ids,
                    ),
                )
                .await?
                .into_iter()
                .map(|row| row.external_id)
                .filter(|external_id| !existing.iter().any(|buy| &buy.external_id == external_id))
                .collect()
        };

        let plan = plan_opening_buys(required, existing, &taken_external_ids);

        for buy in &plan.owned_by_user {
            tracing::warn!(
                binding_id = %binding_id,
                external_id = %buy.external_id,
                shortfall = %buy.quantity,
                "sell still lacks a position but its synthetic opening buy was edited or deleted by the user — not recreated"
            );
        }

        if !plan.delete.is_empty() {
            self.db
                .execute(
                    connector_queries::delete_connector_transactions_by_external_ids(
                        binding_id,
                        plan.delete
                            .iter()
                            .map(|buy| buy.external_id.clone())
                            .collect(),
                    ),
                )
                .await?;
            self.transaction_management
                .delete_transactions_inner(
                    user_id,
                    plan.delete.iter().map(|buy| buy.transaction_id).collect(),
                )
                .await?;
            report.opening_buys_deleted = plan.delete.len();
        }

        if plan.create.is_empty() && plan.keep.is_empty() {
            return Ok(());
        }

        let earliest_imported: Option<OffsetDateTime> = self
            .db
            .fetch_one_scalar(connector_queries::get_binding_earliest_transaction_date(
                binding_id,
                SYNTHETIC_OPENING_BUY_PREFIX,
            ))
            .await?;

        let lookup_ids: HashSet<Uuid> = plan
            .create
            .iter()
            .map(|buy| buy.sell_transaction_id)
            .chain(
                plan.keep
                    .iter()
                    .flat_map(|(existing, buy)| [existing.transaction_id, buy.sell_transaction_id]),
            )
            .collect();
        let dtos: HashMap<Uuid, TransactionDto> = self
            .transaction_management
            .get_transactions_by_ids(user_id, lookup_ids.into_iter().collect())
            .await?
            .into_iter()
            .filter_map(|dto| dto.transaction_id.map(|id| (id, dto)))
            .collect();

        let desired_dto = |buy: &RequiredOpeningBuy| -> Option<TransactionDto> {
            let sell = dtos.get(&buy.sell_transaction_id)?;
            let TransactionTypeDto::AssetSale(sale) = &sell.transaction_type else {
                return None;
            };
            let date = earliest_imported.map_or(sell.date, |earliest| earliest.min(sell.date));
            buy.to_transaction_dto(sale.sale.quantity, &sale.proceeds, date)
        };

        let mut new_entities: Vec<Transaction> = Vec::new();
        for buy in &plan.create {
            let Some(dto) = desired_dto(buy) else {
                tracing::warn!(
                    binding_id = %binding_id,
                    external_id = %buy.external_id,
                    "sell cannot be priced — synthetic opening buy not created"
                );
                continue;
            };
            let mut entity = create_transaction_from_dto(dto, user_id)?;
            entity.set_connector_link(Some(ConnectorLinkMeta {
                binding_id,
                external_id: buy.external_id.clone(),
                external_hash: buy.external_id.clone(),
            }));
            new_entities.push(entity);
        }
        if !new_entities.is_empty() {
            self.transaction_management
                .add_transactions(&mut new_entities)
                .await?;
            report.opening_buys_created = new_entities.len();
        }

        let mut updates: Vec<(Uuid, TransactionDto)> = Vec::new();
        for (existing, buy) in &plan.keep {
            let (Some(current), Some(mut desired)) =
                (dtos.get(&existing.transaction_id), desired_dto(buy))
            else {
                continue;
            };
            if !opening_buy_changed(current, &desired) {
                continue;
            }
            if let (
                TransactionTypeDto::AssetPurchase(current_purchase),
                TransactionTypeDto::AssetPurchase(desired_purchase),
            ) = (&current.transaction_type, &mut desired.transaction_type)
            {
                desired_purchase.purchase.entry_id = current_purchase.purchase.entry_id;
                desired_purchase.sale.entry_id = current_purchase.sale.entry_id;
            }
            updates.push((existing.transaction_id, desired));
        }
        report.opening_buys_updated = updates.len();
        self.transaction_management
            .update_synced_transactions(user_id, updates)
            .await?;

        Ok(())
    }
}
