#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::job_queue::JobQueueHandle;
use dal::models::connector_models::{
    AddConnectorBindingModel, AddConnectorConnectionModel, ConnectorBindingRow,
    ConnectorConnectionRow, ConnectorRawPageRow, InsertedConnectorBindingRow,
    ProviderAccountRefRow, ProviderAccountSyncStateRow,
};
use dal::queries::connector_queries;
use dal::query_params::connector_params::{
    DeleteHeldTransactionsParams, GetConnectorBindingsParams, GetConnectorConnectionsParams,
    GetRawPagesParams,
};
#[mockall_double::double]
use dal::redis_connection::RedisConnection;
use dal::secrets::SecretProvider;

use connectors::models::sync::RawPage;
use connectors::port::ConnectorStore;
use connectors::provider::{map_pages, BeginOauthOptions, ProviderKind};
use itertools::Itertools;

use crate::dtos::bad_request_error_dto::BusinessBadRequestError;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::connectors::{
    BeginOauthOptionsDto, BindingStatusDto, BindingUpdateStatusDto, BindingWriteModeDto,
    ConnectionStatusDto, ConnectorBindingDto, ConnectorConnectionDto, ConnectorImportFromDto,
    CredentialModeDto, HistoryStartDto, OAuthSessionStartDto, OAuthSessionStateDto,
    ProviderAccountDto, ProviderAccountHistoryDto, ProviderAccountTransactionDto,
};
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::connectors::bank_account_history;
use crate::jobs::BackfillConnectionJob;
use crate::providers::connector_store::BusinessConnectorStore;

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::accounts_service::AccountsService;
use super::entries_service::EntriesService;
use super::ServiceProviders;

pub struct ConnectorService {
    db: MyraDb,
    secret_provider: Arc<dyn SecretProvider>,
    redis: RedisConnection,
    queue: JobQueueHandle,
    accounts: AccountsService,
    entries: EntriesService,
}

impl ConnectorService {
    pub fn new(providers: &ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            secret_provider: providers.secret_provider.clone(),
            redis: providers.redis.clone(),
            queue: providers.job_queue.clone(),
            accounts: AccountsService::new(providers),
            entries: EntriesService::new(providers),
        }
    }

    fn store_for_connection(
        &self,
        connection: &ConnectorConnectionDto,
    ) -> anyhow::Result<BusinessConnectorStore> {
        BusinessConnectorStore::for_connection(
            self.db.clone(),
            self.secret_provider.clone(),
            self.redis.clone(),
            connection,
        )
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_connection(
        &self,
        user_id: Uuid,
        provider_kind: String,
        credential_mode: CredentialModeDto,
        provider_key_id: Option<String>,
        credential_bytes: Option<String>,
    ) -> anyhow::Result<Uuid> {
        let provider_id_query =
            connector_queries::get_connector_provider_id_by_kind(provider_kind.clone());
        let provider_id: Uuid = self
            .db
            .fetch_one_scalar(provider_id_query)
            .await
            .map_err(|_| anyhow::anyhow!("unknown provider kind: {provider_kind}"))?;

        let model = AddConnectorConnectionModel {
            user_id,
            provider_id,
            credential_mode: credential_mode.as_str().to_string(),
            provider_key_id,
            status: ConnectionStatusDto::Active.as_str().to_string(),
            consent_expires_at: None,
        };

        let query = connector_queries::insert_connector_connection(model);
        let connection_id: Uuid = self.db.fetch_one_scalar(query).await?;

        if credential_mode == CredentialModeDto::Stored {
            if let Some(cred) = credential_bytes {
                let secret_key = crate::providers::connector_store::credential_ref(connection_id);
                self.secret_provider
                    .store_secret(&secret_key, cred.as_bytes())
                    .await?;
                self.enqueue_attended_backfill(user_id, connection_id).await;
            }
        }

        Ok(connection_id)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn list_connections(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<ConnectorConnectionDto>> {
        let params = GetConnectorConnectionsParams::all(user_id);
        let query = connector_queries::get_connector_connections(params);
        let results = self.db.fetch_all::<ConnectorConnectionRow>(query).await?;

        Ok(results.into_iter().map_into().collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn get_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<ConnectorConnectionDto> {
        let query = connector_queries::get_connector_connections(
            GetConnectorConnectionsParams::by_id(user_id, connection_id),
        );
        let result = self
            .db
            .fetch_optional::<ConnectorConnectionRow>(query)
            .await?
            .ok_or_else(|| {
                anyhow::Error::new(BusinessNotFoundError {
                    message: format!("connection {connection_id} not found"),
                })
            })?;

        Ok(result.into())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn list_provider_accounts(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<Vec<ProviderAccountDto>> {
        let connection = self.get_connection(user_id, connection_id).await?;

        let store = self.store_for_connection(&connection)?;
        let accounts = store
            .provider_kind()
            .provider()
            .list_accounts(&store)
            .await
            .map_err(|e| {
                anyhow::Error::new(BusinessBadRequestError {
                    message: e.to_string(),
                })
            })?;

        let states: HashMap<String, ProviderAccountSyncStateRow> = self
            .db
            .fetch_all::<ProviderAccountSyncStateRow>(
                connector_queries::get_provider_account_sync_states(user_id, connection_id),
            )
            .await?
            .into_iter()
            .map(|row| (row.external_account_id.clone(), row))
            .collect();

        Ok(accounts
            .into_iter()
            .map(|account| {
                let history = match states.get(&account.provider_account_id) {
                    None => ProviderAccountHistoryDto::NotFetched,
                    Some(state) if state.synced_through.is_some() => {
                        ProviderAccountHistoryDto::Fetched
                    }
                    Some(state) if state.last_sync_status.as_deref() == Some("failed") => {
                        ProviderAccountHistoryDto::Failed
                    }
                    Some(_) => ProviderAccountHistoryDto::Fetching,
                };
                (account, history).into()
            })
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn list_provider_account_transactions(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        external_account_id: &str,
    ) -> anyhow::Result<Vec<ProviderAccountTransactionDto>> {
        let connection = self.get_connection(user_id, connection_id).await?;
        let kind: ProviderKind = connection.provider_kind.parse()?;

        let raw_pages = self
            .db
            .fetch_all::<ConnectorRawPageRow>(connector_queries::get_connector_raw_pages(
                GetRawPagesParams::by_external_account(
                    connection_id,
                    external_account_id.to_string(),
                ),
            ))
            .await?;

        let pages: Vec<RawPage> = raw_pages
            .into_iter()
            .map(|page| RawPage {
                stream: page.stream,
                payload: page.payload.0,
            })
            .collect();

        let mut transactions = map_pages(kind, &pages);
        transactions.sort_by(|a, b| b.date.cmp(&a.date));

        Ok(transactions.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn revoke_connection(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<()> {
        self.get_connection(user_id, connection_id).await?;

        let secret_key = crate::providers::connector_store::credential_ref(connection_id);
        if let Err(e) = self.secret_provider.delete_secret(&secret_key).await {
            tracing::warn!("failed to delete secret for revoked connection {connection_id}: {e}");
        }

        let query = connector_queries::update_connector_connection_status(
            user_id,
            connection_id,
            ConnectionStatusDto::Revoked.as_str().to_string(),
        );
        self.db.execute(query).await?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn get_binding(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
    ) -> anyhow::Result<ConnectorBindingDto> {
        let query = connector_queries::get_connector_bindings(GetConnectorBindingsParams::by_id(
            user_id, binding_id,
        ));
        let result = self
            .db
            .fetch_optional::<ConnectorBindingRow>(query)
            .await?
            .ok_or_else(|| {
                anyhow::Error::new(BusinessNotFoundError {
                    message: format!("binding {binding_id} not found"),
                })
            })?;

        Ok(result.into())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn list_bindings(&self, user_id: Uuid) -> anyhow::Result<Vec<ConnectorBindingDto>> {
        let params = GetConnectorBindingsParams::all(user_id);
        let query = connector_queries::get_connector_bindings(params);
        let results = self.db.fetch_all::<ConnectorBindingRow>(query).await?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(connection_id = %connection_id, user_id = %user_id))]
    pub async fn create_binding(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
        sverto_account_id: Uuid,
        provider_account_id: Option<String>,
        write_mode: String,
        import_from: ConnectorImportFromDto,
    ) -> anyhow::Result<Uuid> {
        let connection = self.get_connection(user_id, connection_id).await?;
        if connection.links_by_import()? {
            return Err(BusinessConflictError {
                message: "This connection's accounts are linked by importing their history."
                    .to_string(),
            }
            .into());
        }

        self.accounts
            .get_account_with_metadata(user_id, sverto_account_id)
            .await?;
        let latest_entries = match import_from {
            ConnectorImportFromDto::AfterLastEntry => {
                self.entries
                    .get_latest_entry_dates(user_id, vec![sverto_account_id])
                    .await?
            }
            ConnectorImportFromDto::All | ConnectorImportFromDto::Date(_) => HashMap::new(),
        };
        let history_start =
            bank_account_history::history_start(import_from, latest_entries.into_values());

        let store = self.store_for_connection(&connection)?;
        let external_account_id = store
            .provider_kind()
            .provider()
            .resolve_provider_account_id(provider_account_id, &store)
            .map_err(|e| {
                anyhow::Error::new(BusinessBadRequestError {
                    message: e.to_string(),
                })
            })?;

        let ibans = self
            .linked_account_ibans(
                user_id,
                connection_id,
                vec![(external_account_id.clone(), sverto_account_id)],
            )
            .await;

        self.db.start_transaction().await?;
        let provider_account: ProviderAccountRefRow = self
            .db
            .fetch_one(connector_queries::get_or_create_provider_accounts(
                connection_id,
                vec![external_account_id.clone()],
            ))
            .await?;

        let binding: InsertedConnectorBindingRow = self
            .db
            .fetch_one(connector_queries::insert_connector_bindings(
                user_id,
                vec![AddConnectorBindingModel {
                    provider_account_ref: provider_account.id,
                    sverto_account_id,
                    write_mode,
                    status: BindingStatusDto::Active.as_str().to_string(),
                    history_from: history_start.map(HistoryStartDto::day),
                    history_from_last_entry: matches!(
                        history_start,
                        Some(HistoryStartDto::AfterLastEntry(_))
                    ),
                }],
            ))
            .await?;
        self.accounts.add_ibans(user_id, ibans).await?;
        self.db.commit_transaction().await?;

        Ok(binding.id)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub(crate) async fn linked_account_ibans(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        links: Vec<(String, Uuid)>,
    ) -> Vec<(Uuid, String)> {
        let accounts = match self.list_provider_accounts(user_id, connection_id).await {
            Ok(accounts) => accounts,
            Err(error) => {
                tracing::warn!(error = ?error, "provider accounts unavailable; the linked accounts get no IBAN");
                return Vec::new();
            }
        };
        let ibans: HashMap<String, String> = accounts
            .into_iter()
            .filter_map(|account| Some((account.provider_account_id, account.iban?)))
            .collect();
        links
            .into_iter()
            .filter_map(|(provider_account_id, account_id)| {
                Some((account_id, ibans.get(&provider_account_id)?.clone()))
            })
            .collect()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn update_binding(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
        write_mode: BindingWriteModeDto,
        status: BindingUpdateStatusDto,
    ) -> anyhow::Result<()> {
        let query = connector_queries::update_connector_binding(
            user_id,
            binding_id,
            Some(write_mode.as_str().to_string()),
            status.as_str().to_string(),
        );
        if self.db.execute_with_rows_affected(query).await? == 0 {
            self.get_binding(user_id, binding_id).await?;
            return Err(waiting_for_import().into());
        }

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn delete_binding(&self, user_id: Uuid, binding_id: Uuid) -> anyhow::Result<()> {
        let query = connector_queries::update_connector_binding(
            user_id,
            binding_id,
            None,
            BindingStatusDto::Revoked.as_str().to_string(),
        );
        self.db.start_transaction().await?;
        if self.db.execute_with_rows_affected(query).await? == 0 {
            self.db.rollback_transaction().await?;
            self.get_binding(user_id, binding_id).await?;
            return Err(waiting_for_import().into());
        }
        self.db
            .execute(connector_queries::delete_held_transactions(
                DeleteHeldTransactionsParams::by_binding(user_id, binding_id),
            ))
            .await?;
        self.db.commit_transaction().await?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn get_bindings(
        &self,
        user_id: Uuid,
        binding_ids: Vec<Uuid>,
    ) -> anyhow::Result<Vec<ConnectorBindingDto>> {
        let rows: Vec<ConnectorBindingRow> = self
            .db
            .fetch_all(connector_queries::get_connector_bindings(
                GetConnectorBindingsParams::by_ids(user_id, binding_ids),
            ))
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn delete_pending_bindings(
        &self,
        user_id: Uuid,
        binding_ids: Vec<Uuid>,
    ) -> anyhow::Result<()> {
        self.db
            .execute(connector_queries::delete_pending_bindings(
                user_id,
                binding_ids,
            ))
            .await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn list_aspsps(
        &self,
        user_id: Uuid,
        provider_kind: &str,
        country: &str,
    ) -> anyhow::Result<Vec<connectors::models::Aspsp>> {
        let kind: ProviderKind = provider_kind.parse()?;
        let store = BusinessConnectorStore::for_provider_kind(
            self.db.clone(),
            self.secret_provider.clone(),
            self.redis.clone(),
            kind,
        );

        kind.provider()
            .list_aspsps(&store, country)
            .await
            .map_err(|e| {
                anyhow::Error::new(BusinessBadRequestError {
                    message: e.to_string(),
                })
            })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn begin_oauth_session(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        redirect_uri: Option<String>,
        options: BeginOauthOptionsDto,
    ) -> anyhow::Result<OAuthSessionStartDto> {
        let connection = self.get_connection(user_id, connection_id).await?;
        let store = self.store_for_connection(&connection)?;

        let session_id = Uuid::new_v4().to_string();
        let state = Uuid::new_v4().to_string();

        let auth_url = store
            .provider_kind()
            .provider()
            .begin_oauth(
                &store,
                &state,
                redirect_uri.as_deref(),
                BeginOauthOptions {
                    bank_name: options.bank_name,
                    bank_country: options.bank_country,
                },
            )
            .await
            .map_err(|e| {
                anyhow::Error::new(BusinessBadRequestError {
                    message: e.to_string(),
                })
            })?;

        let key = format!("oauth:state:{}:{}", user_id, session_id);
        let session_state = serde_json::to_string(&OAuthSessionStateDto {
            state: state.clone(),
            redirect_uri,
        })?;
        self.redis.set_string_ex(&key, &session_state, 300).await;

        Ok(OAuthSessionStartDto {
            session_id,
            auth_url,
            state,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn complete_oauth(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        code: &str,
        redirect_uri: Option<&str>,
    ) -> anyhow::Result<()> {
        let connection = self.get_connection(user_id, connection_id).await?;
        let store = self.store_for_connection(&connection)?;

        let consent_expires_at = store
            .provider_kind()
            .provider()
            .complete_oauth(&store, code, redirect_uri)
            .await?;

        let query = connector_queries::activate_connector_connection(
            user_id,
            connection_id,
            consent_expires_at,
        );
        self.db.execute(query).await?;
        self.enqueue_attended_backfill(user_id, connection_id).await;

        Ok(())
    }

    async fn enqueue_attended_backfill(&self, user_id: Uuid, connection_id: Uuid) {
        let queued: anyhow::Result<()> = async {
            let accounts = self.list_provider_accounts(user_id, connection_id).await?;
            if accounts.is_empty() {
                return Ok(());
            }
            self.db.start_transaction().await?;
            self.db
                .execute(connector_queries::get_or_create_provider_accounts(
                    connection_id,
                    accounts
                        .into_iter()
                        .map(|account| account.provider_account_id)
                        .collect(),
                ))
                .await?;
            self.db
                .push(
                    &self.queue,
                    vec![BackfillConnectionJob {
                        connection_id,
                        user_id,
                    }],
                )
                .await?;
            Ok(self.db.commit_transaction().await?)
        }
        .await;
        if let Err(error) = queued {
            tracing::warn!(%connection_id, error = ?error, "queueing the history fetch of a new connection failed");
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, session_id = %session_id))]
    pub async fn consume_oauth_state(
        &self,
        user_id: Uuid,
        session_id: &str,
        state: &str,
    ) -> Option<OAuthSessionStateDto> {
        let key = format!("oauth:state:{}:{}", user_id, session_id);
        let stored = self.redis.get_string(&key).await?;
        self.redis.del(&key).await;
        let parsed: OAuthSessionStateDto = serde_json::from_str(&stored).ok()?;
        (parsed.state == state).then_some(parsed)
    }
}

fn waiting_for_import() -> BusinessConflictError {
    BusinessConflictError {
        message: "This account is waiting for its import. Finish or delete the import first."
            .to_string(),
    }
}
