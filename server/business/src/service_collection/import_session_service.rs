#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::import_models::{ImportFileStatus, ImportSessionModel, ImportSessionSummaryModel};
use dal::queries::import_queries;
use dal::query_params::import_params::GetImportSessionsParams;
use uuid::Uuid;

use super::ai_conversation_service::AiConversationService;
use super::import_classify_service::ImportClassifyService;
use super::import_file_service::ImportFileService;
use super::import_link_service::ImportLinkService;
use super::import_staging_service::ImportStagingService;
use super::user_service::UsersService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_ask_dto::{ImportAskDto, ImportOpenAskDto, NoCheckpointAskDto};
use crate::dtos::imports::import_classification_dto::{
    ImportClassificationDto, ImportClassificationStatusDto,
};
use crate::dtos::imports::import_session_dto::{
    ImportSessionCountsDto, ImportSessionDetailDto, ImportSessionDto,
};
use crate::dtos::imports::import_staging_dto::ImportVerbDto;
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::imports::linking::SessionLinks;
use crate::entities::imports::session_status::ImportSessionStatusInputs;

pub struct ImportSessionService {
    db: MyraDb,
    conversation_service: AiConversationService,
    classify_service: ImportClassifyService,
    import_file_service: ImportFileService,
    link_service: ImportLinkService,
    staging_service: ImportStagingService,
    users_service: UsersService,
}

impl ImportSessionService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            conversation_service: AiConversationService::new(providers),
            classify_service: ImportClassifyService::new(providers),
            import_file_service: ImportFileService::new(providers),
            link_service: ImportLinkService::new(providers),
            staging_service: ImportStagingService::new(providers),
            users_service: UsersService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_session(&self, user_id: Uuid) -> anyhow::Result<ImportSessionDto> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        self.db.start_transaction().await?;
        let conversation_id = self
            .conversation_service
            .create_conversation(user_id)
            .await?;
        let model: ImportSessionModel = self
            .db
            .fetch_one(import_queries::create_session(user_id, conversation_id))
            .await?;
        self.db.commit_transaction().await?;

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
        let files = self
            .import_file_service
            .get_files(session_id, user_id)
            .await?;

        let session = Self::to_session_dto(model, &links)?;
        let mut open_asks = links.asks;
        open_asks.extend(balance_asks);
        Ok(ImportSessionDetailDto {
            session,
            files,
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
        self.db.start_transaction().await?;

        let upload_ids = self
            .import_file_service
            .delete_session_files(session_id, user_id)
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
        self.db.commit_transaction().await.map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(|error| error.is_foreign_key_violation())
            {
                anyhow::Error::new(BusinessConflictError {
                    message: "A file was added while the import was being deleted. Try again."
                        .to_string(),
                })
            } else {
                anyhow::Error::new(error)
            }
        })?;

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
        status_inputs.add_open_asks(links.asks.len() as i64);

        let classification = ImportClassificationDto::from(&model);
        Ok(ImportSessionDto {
            id: model.id,
            conversation_id: model.conversation_id,
            accounts: links
                .account_mappings
                .iter()
                .filter_map(|mapping| mapping.account_id)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            status: status_inputs.status(),
            classification,
            created_at: model.created_at,
        })
    }
}
