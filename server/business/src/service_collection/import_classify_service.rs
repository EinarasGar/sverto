use std::collections::HashMap;

#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::job_queue::JobQueueHandle;
use dal::models::import_models::{
    ImportParsedLegModel, ImportParsedLineModel, ImportSessionLifecycle, ImportSessionSummaryModel,
};
use dal::queries::import_queries;
use dal::query_params::import_params::{ClassificationRunOutcome, GetImportSessionsParams};
use uuid::Uuid;

use super::ai_conversation_service::AiConversationService;
use super::user_service::UsersService;
use crate::dtos::ai_features_disabled_error_dto::BusinessAiFeaturesDisabledError;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_classification_dto::{
    ImportClassificationModeDto, ImportClassificationStatusDto, ImportLineTypingDto,
    ImportParsedLegDto, ImportParsedLineDto,
};
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::imports::classification_mode::classification_mode;
use crate::jobs::ImportClassifyJob;

pub struct ImportClassifyService {
    db: MyraDb,
    queue: JobQueueHandle,
    conversation_service: AiConversationService,
    users_service: UsersService,
}

impl ImportClassifyService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            queue: providers.job_queue.clone(),
            conversation_service: AiConversationService::new(providers),
            users_service: UsersService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn open_run(&self, session_id: Uuid) -> anyhow::Result<()> {
        self.db
            .execute(import_queries::insert_classification_run(session_id))
            .await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn start_if_ready(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<()> {
        let Some(summary) = self.summary(session_id, user_id).await? else {
            return Ok(());
        };
        let Some(run_id) = summary.classification_run_id else {
            return Ok(());
        };
        if summary.session_lifecycle()? != ImportSessionLifecycle::Active
            || summary.pending_files + summary.processing_files > 0
            || summary.classified_at.is_some()
            || summary.classification_error.is_some()
        {
            return Ok(());
        }
        self.queue
            .push_once(
                ImportClassifyJob {
                    session_id,
                    run_id,
                    user_id,
                },
                &run_id.to_string(),
            )
            .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub async fn retry(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<()> {
        let Some(summary) = self.summary(session_id, user_id).await? else {
            return Err(BusinessNotFoundError {
                message: "Import session not found.".to_string(),
            }
            .into());
        };
        let ai_features_enabled = self.users_service.is_ai_features_enabled(user_id).await?;
        if classification_mode(&summary, ai_features_enabled)
            == ImportClassificationModeDto::RefuseWithoutAi
        {
            return Err(BusinessAiFeaturesDisabledError.into());
        }
        let status = ImportClassificationStatusDto::from(&summary);
        if summary.session_lifecycle()? != ImportSessionLifecycle::Active
            || summary.parsed_files + summary.bank_accounts == 0
            || !matches!(
                status,
                ImportClassificationStatusDto::NotStarted
                    | ImportClassificationStatusDto::Running
                    | ImportClassificationStatusDto::Failed
            )
        {
            return Err(BusinessConflictError {
                message:
                    "Only a classification that failed, stalled or never started can be retried."
                        .to_string(),
            }
            .into());
        }
        if status != ImportClassificationStatusDto::Running {
            self.open_run(session_id).await?;
        }
        self.start_if_ready(session_id, user_id).await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn mode(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<ImportClassificationModeDto> {
        let summary =
            self.summary(session_id, user_id)
                .await?
                .ok_or_else(|| BusinessNotFoundError {
                    message: "Import session not found.".to_string(),
                })?;
        Ok(classification_mode(
            &summary,
            self.users_service.is_ai_features_enabled(user_id).await?,
        ))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn parsed_lines(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<ImportParsedLineDto>> {
        let lines: Vec<ImportParsedLineModel> = self
            .db
            .fetch_all(import_queries::get_parsed_lines(session_id, user_id))
            .await?;
        let mut legs: HashMap<Uuid, Vec<ImportParsedLegDto>> = HashMap::new();
        for leg in self
            .db
            .fetch_all::<ImportParsedLegModel>(import_queries::get_parsed_legs(session_id, user_id))
            .await?
        {
            legs.entry(leg.parsed_line_id)
                .or_default()
                .push(leg.try_into()?);
        }
        Ok(lines
            .into_iter()
            .map(|line| ImportParsedLineDto {
                legs: legs.remove(&line.id).unwrap_or_default(),
                id: line.id,
                source_id: line.source_id,
                transaction_key: line.transaction_key,
                account_ref: line.account_ref,
                date: line.date_transacted,
                description: line.description,
                related_asset_ref: line.related_asset_ref,
                external_id: line.external_id,
            })
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn classified_run(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let summary =
            self.summary(session_id, user_id)
                .await?
                .ok_or_else(|| BusinessNotFoundError {
                    message: "Import session not found.".to_string(),
                })?;
        if summary.session_lifecycle()? != ImportSessionLifecycle::Active
            || summary.pending_files + summary.processing_files > 0
        {
            return Ok(None);
        }
        Ok(summary.classified_at.and(summary.classification_run_id))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, run_id = %run_id))]
    pub async fn open_conversation(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let current = match self.summary(session_id, user_id).await? {
            Some(summary) => {
                summary.session_lifecycle()? == ImportSessionLifecycle::Active
                    && summary.classification_run_id == Some(run_id)
                    && matches!(
                        ImportClassificationStatusDto::from(&summary),
                        ImportClassificationStatusDto::Waiting
                            | ImportClassificationStatusDto::Running
                    )
            }
            None => false,
        };
        if !current {
            return Ok(None);
        }
        let conversation_id = self
            .conversation_service
            .create_conversation(user_id)
            .await?;
        self.db
            .execute(import_queries::insert_classification_conversation(
                run_id,
                conversation_id,
            ))
            .await?;
        Ok(Some(conversation_id))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, run_id = %run_id))]
    pub async fn save(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_id: Uuid,
        typings: Vec<ImportLineTypingDto>,
    ) -> anyhow::Result<()> {
        self.db.start_transaction().await?;
        for typing in typings {
            self.db
                .execute(import_queries::insert_line_classifications(
                    session_id,
                    run_id,
                    user_id,
                    typing.line_type.into(),
                    typing.category_id,
                    typing.line_ids,
                ))
                .await?;
        }
        self.finish(
            session_id,
            run_id,
            user_id,
            ClassificationRunOutcome::Classified,
        )
        .await?;
        Ok(self.db.commit_transaction().await?)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, run_id = %run_id))]
    pub async fn fail(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_id: Uuid,
        message: String,
    ) -> anyhow::Result<()> {
        self.finish(
            session_id,
            run_id,
            user_id,
            ClassificationRunOutcome::Failed(message),
        )
        .await
    }

    async fn finish(
        &self,
        session_id: Uuid,
        run_id: Uuid,
        user_id: Uuid,
        outcome: ClassificationRunOutcome,
    ) -> anyhow::Result<()> {
        self.db
            .execute(import_queries::finish_classification_run(
                session_id, run_id, user_id, outcome,
            ))
            .await?;
        Ok(())
    }

    async fn summary(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Option<ImportSessionSummaryModel>> {
        Ok(self
            .db
            .fetch_optional(import_queries::get_session_summaries(
                GetImportSessionsParams::by_id(session_id, user_id),
            ))
            .await?)
    }
}
