#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::import_models::{
    ImportAccountMappingModel, ImportAssertionInsert, ImportAssertionKind, ImportAssetMappingModel,
    ImportBankAccountInsert, ImportFileStatus, ImportParsedLegInsert, ImportParsedLineInsert,
    ImportSessionModel, ImportSessionSummaryModel, ImportSourceKind, ImportSourceModel,
};
use dal::queries::import_queries;
use dal::query_params::import_params::{DeleteImportSourcesParams, GetImportSessionsParams};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

use super::ai_conversation_service::AiConversationService;
use super::asset_service::AssetsService;
use super::connector_service::ConnectorService;
use super::import_classify_service::ImportClassifyService;
use super::import_file_service::ImportFileService;
use super::import_link_service::ImportLinkService;
use super::import_staging_service::ImportStagingService;
use super::ledger_matching_service::LedgerMatchingService;
use super::user_service::UsersService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::connectors::ConnectorBindingDto;
use crate::dtos::imports::import_ask_dto::{ImportAskDto, ImportOpenAskDto, NoCheckpointAskDto};
use crate::dtos::imports::import_classification_dto::{
    ImportClassificationDto, ImportClassificationStatusDto,
};
use crate::dtos::imports::import_session_dto::{
    ImportSessionCountsDto, ImportSessionDetailDto, ImportSessionDto,
};
use crate::dtos::imports::import_source_dto::{
    AddImportBankSourceDto, ImportBankSourceDto, ImportSourceDto,
};
use crate::dtos::imports::import_staging_dto::ImportVerbDto;
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::imports::linking::SessionLinks;
use crate::entities::imports::reconciliation::statement_stream;
use crate::entities::imports::session_status::ImportSessionStatusInputs;

const INSERT_CHUNK: usize = 1000;

pub struct ImportSessionService {
    db: MyraDb,
    conversation_service: AiConversationService,
    assets_service: AssetsService,
    connector_service: ConnectorService,
    classify_service: ImportClassifyService,
    import_file_service: ImportFileService,
    link_service: ImportLinkService,
    staging_service: ImportStagingService,
    ledger_matching_service: LedgerMatchingService,
    users_service: UsersService,
}

impl ImportSessionService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            conversation_service: AiConversationService::new(providers),
            assets_service: AssetsService::new(providers),
            connector_service: ConnectorService::new(providers),
            classify_service: ImportClassifyService::new(providers),
            import_file_service: ImportFileService::new(providers),
            link_service: ImportLinkService::new(providers),
            staging_service: ImportStagingService::new(providers),
            ledger_matching_service: LedgerMatchingService::new(providers),
            users_service: UsersService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_session(&self, user_id: Uuid) -> anyhow::Result<ImportSessionDto> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        self.db.start_transaction().await?;
        let session = self.create_session_inner(user_id).await?;
        self.db.commit_transaction().await?;
        Ok(session)
    }

    pub(crate) async fn create_session_inner(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<ImportSessionDto> {
        let conversation_id = self
            .conversation_service
            .create_conversation(user_id)
            .await?;
        let model: ImportSessionModel = self
            .db
            .fetch_one(import_queries::create_session(user_id, conversation_id))
            .await?;

        let status = ImportSessionStatusInputs::new(model.session_lifecycle()?, false).status();
        Ok(ImportSessionDto {
            id: model.id,
            conversation_id: model.conversation_id,
            accounts: Vec::new(),
            status,
            classification: ImportClassificationDto {
                status: ImportClassificationStatusDto::NotStarted,
                error_message: None,
            },
            created_at: model.created_at,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id, count = sources.len()))]
    pub(crate) async fn add_bank_sources(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        sources: Vec<AddImportBankSourceDto>,
    ) -> anyhow::Result<()> {
        let currencies: HashSet<String> = sources
            .iter()
            .flat_map(|source| {
                source.lines.iter().map(|line| line.currency.clone()).chain(
                    source
                        .closing_balance
                        .iter()
                        .map(|balance| balance.currency.clone()),
                )
            })
            .collect();
        let currency_assets = self
            .assets_service
            .resolve_currency_tickers(user_id, currencies)
            .await?;

        let source_ids: Vec<Uuid> = sources.iter().map(|_| Uuid::now_v7()).collect();
        let mut bank_accounts = Vec::with_capacity(sources.len());
        let mut account_answers = Vec::with_capacity(sources.len());
        let mut lines = Vec::new();
        let mut legs = Vec::new();
        let mut assertions = Vec::new();
        for (&source_id, source) in source_ids.iter().zip(sources) {
            bank_accounts.push(ImportBankAccountInsert {
                session_id,
                source_id,
                binding_id: source.binding_id,
                cutoff_page_id: source.cutoff_page_id,
            });
            account_answers.push(ImportAccountMappingModel {
                source_id,
                account_ref: source.account_ref.clone(),
                account_id: Some(source.account_id),
            });
            for line in source.lines {
                let id = Uuid::now_v7();
                legs.push(ImportParsedLegInsert {
                    parsed_line_id: id,
                    asset_ref: line.currency,
                    quantity: line.amount,
                    fee_type: None,
                });
                lines.push(ImportParsedLineInsert {
                    id,
                    session_id,
                    source_id,
                    transaction_key: None,
                    account_ref: source.account_ref.clone(),
                    date_transacted: line.date.midnight().assume_utc(),
                    description: line.description,
                    related_asset_ref: None,
                    external_id: Some(line.external_id),
                });
            }
            assertions.extend(source.closing_balance.map(|balance| ImportAssertionInsert {
                session_id,
                source_id,
                account_ref: source.account_ref,
                asset_ref: balance.currency,
                kind: ImportAssertionKind::Closing,
                as_of_date: balance.as_of_date,
                quantity: balance.amount,
            }));
        }

        let expected = source_ids.len() as u64;
        let added = self
            .db
            .execute_with_rows_affected(import_queries::insert_sources(
                session_id,
                user_id,
                ImportSourceKind::BankAccount,
                source_ids,
            ))
            .await?;
        if added != expected {
            return Err(BusinessConflictError {
                message: "Import session can no longer be changed.".to_string(),
            }
            .into());
        }
        self.db
            .execute(import_queries::insert_bank_accounts(&bank_accounts))
            .await?;
        self.db
            .execute(import_queries::insert_account_answers(
                session_id,
                user_id,
                account_answers,
            ))
            .await?;
        if !currency_assets.is_empty() {
            self.db
                .execute(import_queries::insert_asset_answers(
                    session_id,
                    user_id,
                    currency_assets
                        .into_iter()
                        .map(|(asset_ref, asset_id)| ImportAssetMappingModel {
                            asset_ref,
                            asset_id: Some(asset_id),
                        })
                        .collect(),
                ))
                .await?;
        }
        for chunk in lines.chunks(INSERT_CHUNK) {
            self.db
                .execute(import_queries::insert_parsed_lines(chunk))
                .await?;
        }
        for chunk in legs.chunks(INSERT_CHUNK) {
            self.db
                .execute(import_queries::insert_parsed_legs(chunk))
                .await?;
        }
        for chunk in assertions.chunks(INSERT_CHUNK) {
            self.db
                .execute(import_queries::insert_assertions(chunk))
                .await?;
        }
        self.classify_service.open_run(session_id).await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub(crate) async fn bank_sources(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<ImportBankSourceDto>> {
        let mut bank_sources = Vec::new();
        for source in self
            .db
            .fetch_all::<ImportSourceModel>(import_queries::get_session_sources(
                session_id, user_id,
            ))
            .await?
        {
            if source.source_kind()? == ImportSourceKind::BankAccount {
                bank_sources.push(ImportBankSourceDto {
                    id: source.id,
                    binding_id: source.binding_id,
                    cutoff_page_id: source.cutoff_page_id,
                });
            }
        }
        Ok(bank_sources)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn get_sessions(&self, user_id: Uuid) -> anyhow::Result<Vec<ImportSessionDto>> {
        let models: Vec<ImportSessionSummaryModel> = self
            .db
            .fetch_all(import_queries::get_session_summaries(
                GetImportSessionsParams::all(user_id),
            ))
            .await?;
        let mut sessions = Vec::with_capacity(models.len());
        for model in models {
            let links = self.link_service.links(model.id, user_id).await?;
            sessions.push(Self::to_session_dto(model, &links)?);
        }
        Ok(sessions)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub async fn get_session(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<ImportSessionDetailDto> {
        let model: ImportSessionSummaryModel = self
            .db
            .fetch_optional(import_queries::get_session_summaries(
                GetImportSessionsParams::by_id(session_id, user_id),
            ))
            .await?
            .ok_or_else(|| BusinessNotFoundError {
                message: "Import session not found.".to_string(),
            })?;
        let links = self.link_service.links(session_id, user_id).await?;
        let pending = links
            .pending_lines(
                self.classify_service
                    .parsed_lines(session_id, user_id)
                    .await?,
            )
            .len() as i64;
        let mut counts = ImportSessionCountsDto {
            pending,
            staged_available: true,
            ..Default::default()
        };
        let mut balance_asks = Vec::new();
        match self
            .staging_service
            .proposal(session_id, user_id, &links)
            .await
        {
            Ok(Some(proposal)) => {
                for row in &proposal.rows {
                    match row.verb {
                        ImportVerbDto::Add => counts.add += 1,
                        ImportVerbDto::Amend => counts.amend += 1,
                        ImportVerbDto::Delete => counts.delete += 1,
                        ImportVerbDto::Skip => counts.skip += 1,
                    }
                }
                counts.unstageable = proposal.unstageable.len() as i64;
                balance_asks.extend(
                    proposal
                        .balance_checks
                        .into_iter()
                        .flat_map(|checks| checks.enterable)
                        .filter(|enterable| enterable.entered.is_none())
                        .map(|enterable| ImportOpenAskDto {
                            ask_key: enterable.ask_key,
                            ask: ImportAskDto::NoCheckpoint(NoCheckpointAskDto {
                                account_id: enterable.account_id,
                                asset_id: enterable.asset_id,
                                as_of_date: enterable.as_of_date,
                            }),
                        }),
                );
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(error = ?error, error.type = "import_proposal", "building the import proposal failed");
                counts.staged_available = false;
            }
        }
        let sources: Vec<ImportSourceModel> = self
            .db
            .fetch_all(import_queries::get_session_sources(session_id, user_id))
            .await?;
        let recorded = self
            .ledger_matching_service
            .recorded_counts(
                user_id,
                sources
                    .iter()
                    .map(|source| statement_stream(source.id))
                    .collect(),
            )
            .await?;
        let bindings: HashMap<Uuid, ConnectorBindingDto> = self
            .connector_service
            .get_bindings(
                user_id,
                sources
                    .iter()
                    .filter_map(|source| source.binding_id)
                    .collect(),
            )
            .await?
            .into_iter()
            .map(|binding| (binding.id, binding))
            .collect();
        let mut provider_accounts = HashMap::new();
        for connection_id in bindings
            .values()
            .map(|binding| binding.connection_id)
            .collect::<BTreeSet<_>>()
        {
            match self
                .connector_service
                .list_provider_accounts(user_id, connection_id)
                .await
            {
                Ok(accounts) => provider_accounts.extend(accounts.into_iter().map(|account| {
                    (
                        (connection_id, account.provider_account_id.clone()),
                        account,
                    )
                })),
                Err(error) => {
                    tracing::warn!(error = ?error, %connection_id, "listing the accounts of an imported connection failed")
                }
            }
        }
        let sources = sources
            .into_iter()
            .map(|source| {
                let recorded = recorded
                    .get(&statement_stream(source.id))
                    .copied()
                    .unwrap_or(0);
                let binding = source
                    .binding_id
                    .and_then(|binding_id| bindings.get(&binding_id));
                let provider_account = binding.and_then(|binding| {
                    provider_accounts
                        .get(&(binding.connection_id, binding.provider_account_id.clone()))
                });
                ImportSourceDto::new(source, recorded, binding, provider_account)
            })
            .collect::<anyhow::Result<_>>()?;

        let session = Self::to_session_dto(model, &links)?;
        let mut open_asks = links.asks;
        open_asks.extend(balance_asks);
        Ok(ImportSessionDetailDto {
            session,
            sources,
            counts,
            open_asks,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub async fn delete_session(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<()> {
        let session: Option<ImportSessionModel> = self
            .db
            .fetch_optional(import_queries::get_session(GetImportSessionsParams::by_id(
                session_id, user_id,
            )))
            .await?;
        let Some(session) = session else {
            return Err(BusinessNotFoundError {
                message: "Import session not found.".to_string(),
            }
            .into());
        };
        let binding_ids: Vec<Uuid> = self
            .bank_sources(session_id, user_id)
            .await?
            .into_iter()
            .filter_map(|source| source.binding_id)
            .collect();
        self.db.start_transaction().await?;

        let upload_ids = self
            .import_file_service
            .delete_session_files(session_id, user_id)
            .await?;
        self.connector_service
            .delete_pending_bindings(user_id, binding_ids)
            .await?;
        self.db
            .execute(import_queries::delete_sources(
                DeleteImportSourcesParams::all(session_id, user_id),
            ))
            .await?;
        let mut conversation_ids: Vec<Uuid> = self
            .db
            .fetch_all_scalar(import_queries::get_classification_conversations(
                session_id, user_id,
            ))
            .await?;
        conversation_ids.push(session.conversation_id);
        self.conversation_service
            .delete_conversations(conversation_ids, user_id)
            .await?;
        self.db.commit_transaction().await?;

        self.import_file_service
            .delete_uploads(user_id, upload_ids)
            .await;
        Ok(())
    }

    fn to_session_dto(
        model: ImportSessionSummaryModel,
        links: &SessionLinks,
    ) -> anyhow::Result<ImportSessionDto> {
        let mut status_inputs = ImportSessionStatusInputs::new(
            model.session_lifecycle()?,
            model.classified_at.is_some(),
        );
        status_inputs.add_files(ImportFileStatus::Pending, model.pending_files);
        status_inputs.add_files(ImportFileStatus::Processing, model.processing_files);
        status_inputs.add_files(ImportFileStatus::Failed, model.failed_files);
        status_inputs.add_files(ImportFileStatus::Parsed, model.parsed_files);
        status_inputs.add_bank_accounts(model.bank_accounts);
        status_inputs.add_open_asks(links.asks.len() as i64);

        let classification = ImportClassificationDto::from(&model);
        Ok(ImportSessionDto {
            id: model.id,
            conversation_id: model.conversation_id,
            accounts: links
                .account_mappings
                .iter()
                .filter_map(|mapping| mapping.account_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            status: status_inputs.status(),
            classification,
            created_at: model.created_at,
        })
    }
}
