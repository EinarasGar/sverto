use std::collections::HashMap;

use anyhow::Context;
use connectors::models::sync::RawPage;
use connectors::provider::{map_pages, ProviderKind};
#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::connector_models::{
    AddConnectorBindingModel, ConnectorBindingRow, ConnectorRawPageRow,
    InsertedConnectorBindingRow, ProviderAccountRefRow,
};
use dal::queries::connector_queries;
use dal::query_params::connector_params::{GetConnectorBindingsParams, GetRawPagesParams};
use itertools::Itertools;
use uuid::Uuid;

use super::accounts_service::AccountsService;
use super::asset_service::AssetsService;
use super::connector_service::ConnectorService;
use super::entries_service::EntriesService;
use super::import_classify_service::ImportClassifyService;
use super::import_session_service::ImportSessionService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::connectors::{
    BindingStatusDto, BindingWriteModeDto, ConnectorImportFromDto, CreateConnectorImportDto,
    HistoryStartDto, ProviderAccountDto, ProviderAccountHistoryDto,
};
use crate::dtos::imports::import_source_dto::{AddImportBankSourceDto, ImportExternalLineDto};
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::entities::connectors::bank_account_history;
use crate::entities::connectors::connector_transaction_batch::ConnectorTransactionBatch;

const INSERT_CHUNK: usize = 1000;

pub struct ConnectorImportService {
    db: MyraDb,
    accounts: AccountsService,
    assets_service: AssetsService,
    connector_service: ConnectorService,
    entries_service: EntriesService,
    import_session_service: ImportSessionService,
    classify_service: ImportClassifyService,
}

impl ConnectorImportService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            accounts: AccountsService::new(providers),
            assets_service: AssetsService::new(providers),
            connector_service: ConnectorService::new(providers),
            entries_service: EntriesService::new(providers),
            import_session_service: ImportSessionService::new(providers),
            classify_service: ImportClassifyService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, connection_id = %connection_id))]
    pub async fn create_import(
        &self,
        user_id: Uuid,
        connection_id: Uuid,
        request: CreateConnectorImportDto,
    ) -> anyhow::Result<Uuid> {
        let connection = self
            .connector_service
            .get_connection(user_id, connection_id)
            .await?;
        if !connection.links_by_import()? {
            return Err(conflict("This connection links accounts directly."));
        }
        let kind: ProviderKind = connection.provider_kind.parse()?;
        let provider_accounts: HashMap<String, ProviderAccountDto> = self
            .connector_service
            .list_provider_accounts(user_id, connection_id)
            .await?
            .into_iter()
            .map(|account| (account.provider_account_id.clone(), account))
            .collect();
        if let Some(index) = request
            .accounts
            .iter()
            .position(|account| !provider_accounts.contains_key(&account.provider_account_id))
        {
            return Err(invalid(
                format!("accounts[{index}].provider_account_id"),
                "This account isn't part of the connection.",
            )
            .into());
        }
        let histories: Vec<ProviderAccountHistoryDto> = request
            .accounts
            .iter()
            .map(|account| provider_accounts[&account.provider_account_id].history)
            .collect();
        if histories.contains(&ProviderAccountHistoryDto::Fetching) {
            return Err(conflict(
                "This account's history is still being fetched. Try again in a minute.",
            ));
        }
        if histories.iter().any(|history| {
            matches!(
                history,
                ProviderAccountHistoryDto::Failed | ProviderAccountHistoryDto::NotFetched
            )
        }) {
            return Err(conflict(
                "This account's full history couldn't be fetched. Reconnect to fetch it again.",
            ));
        }
        let latest_entries = match request.import_from {
            ConnectorImportFromDto::AfterLastEntry => {
                self.entries_service
                    .get_latest_entry_dates(
                        user_id,
                        request
                            .accounts
                            .iter()
                            .map(|account| account.sverto_account_id)
                            .collect(),
                    )
                    .await?
            }
            ConnectorImportFromDto::All | ConnectorImportFromDto::Date(_) => HashMap::new(),
        };
        let currency_assets = match request.import_from {
            ConnectorImportFromDto::AfterLastEntry => {
                self.assets_service
                    .resolve_currency_tickers(
                        user_id,
                        request
                            .accounts
                            .iter()
                            .filter_map(|account| {
                                provider_accounts[&account.provider_account_id]
                                    .currency
                                    .clone()
                            })
                            .collect(),
                    )
                    .await?
            }
            ConnectorImportFromDto::All | ConnectorImportFromDto::Date(_) => HashMap::new(),
        };
        let latest_entry = |account_id: Uuid, currency: Option<&String>| -> Option<time::Date> {
            match currency.and_then(|currency| currency_assets.get(currency)) {
                Some(asset_id) => latest_entries.get(&(account_id, *asset_id)).copied(),
                None => latest_entries
                    .iter()
                    .filter(|((account, _), _)| *account == account_id)
                    .map(|(_, date)| *date)
                    .max(),
            }
        };

        self.db.start_transaction().await?;
        let session = self
            .import_session_service
            .create_session_inner(user_id)
            .await?;
        let provider_account_refs: HashMap<String, Uuid> = self
            .db
            .fetch_all::<ProviderAccountRefRow>(connector_queries::get_or_create_provider_accounts(
                connection_id,
                request
                    .accounts
                    .iter()
                    .map(|account| account.provider_account_id.clone())
                    .collect(),
            ))
            .await?
            .into_iter()
            .map(|row| (row.external_account_id, row.id))
            .collect();
        let binding_ids: HashMap<Uuid, Uuid> = self
            .db
            .fetch_all::<InsertedConnectorBindingRow>(connector_queries::insert_connector_bindings(
                user_id,
                request
                    .accounts
                    .iter()
                    .map(|account| AddConnectorBindingModel {
                        provider_account_ref: provider_account_refs[&account.provider_account_id],
                        sverto_account_id: account.sverto_account_id,
                        write_mode: BindingWriteModeDto::Ghost.as_str().to_string(),
                        status: BindingStatusDto::Pending.as_str().to_string(),
                        history_from: None,
                        history_from_last_entry: false,
                    })
                    .collect(),
            ))
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation())
                {
                    anyhow::Error::new(BusinessConflictError {
                        message: "This bank account is already linked to that account.".to_string(),
                    })
                } else {
                    anyhow::Error::new(error)
                }
            })?
            .into_iter()
            .map(|row| (row.provider_account_id, row.id))
            .collect();
        let mut pages = archived_pages(
            self.db
                .fetch_all(connector_queries::get_connector_raw_pages(
                    GetRawPagesParams::by_provider_account_refs(
                        provider_account_refs.values().copied().collect(),
                    ),
                ))
                .await?,
        );

        let mut sources = Vec::with_capacity(request.accounts.len());
        for (index, account) in request.accounts.into_iter().enumerate() {
            let provider_account_ref = provider_account_refs[&account.provider_account_id];
            let Some(&binding_id) = binding_ids.get(&provider_account_ref) else {
                return Err(invalid(
                    format!("accounts[{index}].sverto_account_id"),
                    "Pick one of your accounts.",
                )
                .into());
            };
            let account_pages = pages.remove(&provider_account_ref).unwrap_or_default();
            let provider_account = &provider_accounts[&account.provider_account_id];
            sources.push(AddImportBankSourceDto {
                binding_id,
                cutoff_page_id: account_pages.last().map(|page| page.id),
                lines: bank_account_history::lines(
                    &archived_transactions(kind, account_pages),
                    bank_account_history::history_start(
                        request.import_from,
                        latest_entry(
                            account.sverto_account_id,
                            provider_account.currency.as_ref(),
                        ),
                    )
                    .map(HistoryStartDto::day),
                ),
                closing_balance: provider_account.booked_balance.clone().map(Into::into),
                account_ref: account.provider_account_id,
                account_id: account.sverto_account_id,
            });
        }
        self.import_session_service
            .add_bank_sources(session.id, user_id, sources)
            .await?;
        self.db.commit_transaction().await?;

        if let Err(error) = self
            .classify_service
            .start_if_ready(session.id, user_id)
            .await
        {
            tracing::error!(error = ?error, session_id = %session.id, "queueing the classification of a bank import failed; retrying the classification queues it again");
        }
        Ok(session.id)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, session_id = %session_id))]
    pub async fn linked_account_ibans(
        &self,
        user_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<(Uuid, String)>> {
        let binding_ids: Vec<Uuid> = self
            .import_session_service
            .bank_sources(session_id, user_id)
            .await?
            .into_iter()
            .filter_map(|source| source.binding_id)
            .collect();
        if binding_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut ibans = Vec::new();
        for (connection_id, links) in self
            .connector_service
            .get_bindings(user_id, binding_ids)
            .await?
            .into_iter()
            .map(|binding| {
                (
                    binding.connection_id,
                    (binding.provider_account_id, binding.sverto_account_id),
                )
            })
            .into_group_map()
        {
            ibans.extend(
                self.connector_service
                    .linked_account_ibans(user_id, connection_id, links)
                    .await,
            );
        }
        Ok(ibans)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, session_id = %session_id))]
    pub async fn record_applied(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        lines: Vec<ImportExternalLineDto>,
        ibans: Vec<(Uuid, String)>,
    ) -> anyhow::Result<()> {
        let sources = self
            .import_session_service
            .bank_sources(session_id, user_id)
            .await?;
        if sources.is_empty() {
            return Ok(());
        }
        let binding_ids = sources
            .iter()
            .map(|source| {
                source
                    .binding_id
                    .context("a bank account source of an applied import has its binding")
            })
            .collect::<anyhow::Result<Vec<Uuid>>>()?;
        let bindings: HashMap<Uuid, ConnectorBindingRow> = self
            .db
            .fetch_all::<ConnectorBindingRow>(connector_queries::get_connector_bindings(
                GetConnectorBindingsParams::by_ids(user_id, binding_ids.clone()),
            ))
            .await?
            .into_iter()
            .map(|binding| (binding.id, binding))
            .collect();
        let mut pages = archived_pages(
            self.db
                .fetch_all(connector_queries::get_connector_raw_pages(
                    GetRawPagesParams::by_provider_account_refs(
                        bindings
                            .values()
                            .map(|binding| binding.provider_account_ref)
                            .collect(),
                    ),
                ))
                .await?,
        );

        let mut recorded = Vec::new();
        let mut cutoffs = Vec::with_capacity(sources.len());
        for (source, binding_id) in sources.iter().zip(binding_ids) {
            cutoffs.push((binding_id, source.cutoff_page_id));
            let binding = bindings
                .get(&binding_id)
                .context("the binding of a bank account source exists")?;
            let Some(cutoff_page_id) = source.cutoff_page_id else {
                continue;
            };
            let mut account_pages = pages
                .remove(&binding.provider_account_ref)
                .unwrap_or_default();
            account_pages.retain(|page| page.id <= cutoff_page_id);
            let transaction_ids: HashMap<&str, Option<Uuid>> = lines
                .iter()
                .filter(|line| line.source_id == source.id)
                .map(|line| (line.external_id.as_str(), line.transaction_id))
                .collect();
            recorded.extend(
                archived_transactions(binding.provider_kind.parse()?, account_pages)
                    .connector_transactions(binding_id, &transaction_ids),
            );
        }
        for chunk in recorded.chunks(INSERT_CHUNK) {
            self.db
                .execute(connector_queries::insert_connector_transactions(chunk))
                .await?;
        }
        let expected = cutoffs.len() as u64;
        let activated = self
            .db
            .execute_with_rows_affected(connector_queries::activate_pending_bindings(
                user_id, cutoffs,
            ))
            .await?;
        if activated != expected {
            return Err(conflict(
                "A linked bank account changed while the import was being applied. Review it again.",
            ));
        }
        self.accounts.add_ibans(user_id, ibans).await
    }
}

fn archived_pages(rows: Vec<ConnectorRawPageRow>) -> HashMap<Uuid, Vec<ConnectorRawPageRow>> {
    let mut pages: HashMap<Uuid, Vec<ConnectorRawPageRow>> = HashMap::new();
    for row in rows {
        pages.entry(row.provider_account_id).or_default().push(row);
    }
    pages
}

fn archived_transactions(
    kind: ProviderKind,
    pages: Vec<ConnectorRawPageRow>,
) -> ConnectorTransactionBatch {
    let pages: Vec<RawPage> = pages
        .into_iter()
        .map(|page| RawPage {
            stream: page.stream,
            payload: page.payload.0,
        })
        .collect();
    ConnectorTransactionBatch::from_mapped(map_pages(kind, &pages))
}

fn conflict(message: &str) -> anyhow::Error {
    BusinessConflictError {
        message: message.to_string(),
    }
    .into()
}

fn invalid(field: String, message: &str) -> BusinessValidationErrorDto {
    BusinessValidationErrorDto {
        errors: vec![BusinessFieldErrorDto {
            field,
            message: message.to_string(),
        }],
    }
}
