use std::collections::HashMap;

#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::job_queue::JobQueueHandle;
use dal::models::file_models::FileStatus;
use dal::models::import_models::{
    ImportDateOrder, ImportFileModel, ImportSessionLifecycle, ImportSessionModel,
};
use dal::queries::import_queries;
use dal::query_params::import_params::{GetImportFilesParams, GetImportSessionsParams};
use itertools::Itertools;
use uuid::Uuid;

use super::ai_conversation_service::AiConversationService;
use super::file_service::FileService;
use super::import_classify_service::ImportClassifyService;
use super::ledger_matching_service::LedgerMatchingService;
use super::user_service::UsersService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_file_dto::{ImportDateOrderDto, ImportFileDto};
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::imports::reconciliation::statement_stream;
use crate::jobs::ImportParseJob;

pub struct ImportFileService {
    db: MyraDb,
    conversation_service: AiConversationService,
    file_service: FileService,
    classify_service: ImportClassifyService,
    ledger_matching_service: LedgerMatchingService,
    queue: JobQueueHandle,
    users_service: UsersService,
}

impl ImportFileService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            conversation_service: AiConversationService::new(providers),
            file_service: FileService::new(providers),
            classify_service: ImportClassifyService::new(providers),
            ledger_matching_service: LedgerMatchingService::new(providers),
            queue: providers.job_queue.clone(),
            users_service: UsersService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id, count = file_ids.len()))]
    pub async fn attach_files(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        file_ids: Vec<Uuid>,
    ) -> anyhow::Result<Vec<ImportFileDto>> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        let file_ids: Vec<Uuid> = file_ids.into_iter().unique().collect();

        self.ensure_active(session_id, user_id).await?;
        self.db.start_transaction().await?;

        let uploads = self
            .file_service
            .get_files(user_id, file_ids.clone())
            .await?;
        let upload_statuses: HashMap<Uuid, String> = uploads
            .into_iter()
            .map(|upload| (upload.id, upload.status))
            .collect();
        for file_id in &file_ids {
            let Some(status) = upload_statuses.get(file_id) else {
                self.db.rollback_transaction().await?;
                return Err(BusinessNotFoundError {
                    message: format!("File {file_id} not found."),
                }
                .into());
            };
            let status = FileStatus::try_from(status.as_str()).map_err(anyhow::Error::msg)?;
            if !matches!(status, FileStatus::Processing | FileStatus::Ready) {
                self.db.rollback_transaction().await?;
                return Err(BusinessConflictError {
                    message: format!("File {file_id} has not finished uploading."),
                }
                .into());
            }
        }

        let mut attached: HashMap<Uuid, ImportFileModel> = self
            .db
            .fetch_all::<ImportFileModel>(import_queries::get_session_files(
                GetImportFilesParams::all(session_id, user_id),
            ))
            .await?
            .into_iter()
            .map(|file| (file.file_id, file))
            .collect();
        let mut newly_attached = Vec::new();
        for file_id in file_ids.iter().sorted() {
            if attached.contains_key(file_id) {
                continue;
            }
            let conversation_id = self
                .conversation_service
                .create_conversation(user_id)
                .await?;
            let file: Option<ImportFileModel> = self
                .db
                .fetch_optional(import_queries::attach_file(
                    session_id,
                    user_id,
                    *file_id,
                    conversation_id,
                ))
                .await
                .map_err(|error| {
                    if error
                        .as_database_error()
                        .is_some_and(|error| error.is_unique_violation())
                    {
                        anyhow::Error::new(BusinessConflictError {
                            message: "The file is already part of an import.".to_string(),
                        })
                    } else {
                        anyhow::Error::new(error)
                    }
                })?;
            let Some(file) = file else {
                self.db.rollback_transaction().await?;
                return Err(not_mutable().into());
            };
            attached.insert(*file_id, file);
            newly_attached.push(*file_id);
        }
        self.db.commit_transaction().await.map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(|error| error.is_foreign_key_violation())
            {
                anyhow::Error::new(BusinessNotFoundError {
                    message: "Import session not found.".to_string(),
                })
            } else {
                anyhow::Error::new(error)
            }
        })?;
        for file_id in newly_attached {
            self.queue
                .push_with_max_attempts(
                    ImportParseJob {
                        session_id,
                        file_id,
                        user_id,
                    },
                    60,
                )
                .await?;
        }

        file_ids
            .iter()
            .filter_map(|file_id| attached.remove(file_id))
            .map(|file| ImportFileDto::new(file, 0))
            .collect()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub async fn get_files(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<ImportFileDto>> {
        let files: Vec<ImportFileModel> = self
            .db
            .fetch_all(import_queries::get_session_files(
                GetImportFilesParams::all(session_id, user_id),
            ))
            .await?;
        if files.is_empty() {
            let session: Option<ImportSessionModel> = self
                .db
                .fetch_optional(import_queries::get_session(GetImportSessionsParams::by_id(
                    session_id, user_id,
                )))
                .await?;
            if session.is_none() {
                return Err(BusinessNotFoundError {
                    message: "Import session not found.".to_string(),
                }
                .into());
            }
        }
        let counts = self
            .ledger_matching_service
            .recorded_counts(
                user_id,
                files
                    .iter()
                    .map(|file| statement_stream(file.file_id))
                    .collect(),
            )
            .await?;
        files
            .into_iter()
            .map(|file| {
                let recorded = counts
                    .get(&statement_stream(file.file_id))
                    .copied()
                    .unwrap_or(0);
                ImportFileDto::new(file, recorded)
            })
            .collect()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, user_id = %user_id))]
    pub async fn delete_session_files(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<Uuid>> {
        let files: Vec<ImportFileModel> = self
            .db
            .fetch_all(import_queries::get_session_files(
                GetImportFilesParams::all(session_id, user_id),
            ))
            .await?;
        if files.is_empty() {
            return Ok(Vec::new());
        }
        self.conversation_service
            .delete_conversations(
                files.iter().map(|file| file.conversation_id).collect(),
                user_id,
            )
            .await?;
        Ok(files.into_iter().map(|file| file.file_id).collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, count = file_ids.len()))]
    pub async fn delete_uploads(&self, user_id: Uuid, file_ids: Vec<Uuid>) {
        for file_id in file_ids {
            if let Err(err) = self.file_service.delete_file(user_id, file_id).await {
                tracing::warn!(file_id = %file_id, error = %err, "Failed to delete import upload");
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id, user_id = %user_id))]
    pub async fn detach_file(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        self.ensure_active(session_id, user_id).await?;
        self.db.start_transaction().await?;
        let conversation_ids: Vec<Uuid> = self
            .db
            .fetch_all_scalar(import_queries::detach_file(session_id, file_id, user_id))
            .await?;
        if conversation_ids.is_empty() {
            self.db.rollback_transaction().await?;
            return Err(not_attached().into());
        }
        self.conversation_service
            .delete_conversations(conversation_ids, user_id)
            .await?;
        self.db.commit_transaction().await?;

        self.delete_uploads(user_id, vec![file_id]).await;
        self.classify_service
            .start_if_ready(session_id, user_id)
            .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id, user_id = %user_id))]
    pub async fn retry_file(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        self.read_again(session_id, file_id, user_id, None).await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id, user_id = %user_id))]
    pub async fn read_dates(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        date_order: ImportDateOrderDto,
    ) -> anyhow::Result<()> {
        self.read_again(session_id, file_id, user_id, Some(date_order))
            .await
    }

    async fn read_again(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        date_order: Option<ImportDateOrderDto>,
    ) -> anyhow::Result<()> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        self.ensure_active(session_id, user_id).await?;
        let requeued = self
            .db
            .execute_with_rows_affected(import_queries::requeue_file(
                session_id,
                file_id,
                user_id,
                date_order.map(|date_order| match date_order {
                    ImportDateOrderDto::DayFirst => ImportDateOrder::DayFirst,
                    ImportDateOrderDto::MonthFirst => ImportDateOrder::MonthFirst,
                }),
            ))
            .await?;
        if requeued == 0 {
            let file: Option<ImportFileModel> = self
                .db
                .fetch_optional(import_queries::get_session_files(
                    GetImportFilesParams::by_id(session_id, file_id, user_id),
                ))
                .await?;
            let refused = match date_order {
                None => "Only a file that could not be read can be retried.",
                Some(_) => "Only a read file whose dates read both ways can be read the other way.",
            };
            return Err(match file {
                None => not_attached().into(),
                Some(_) => BusinessConflictError {
                    message: refused.to_string(),
                }
                .into(),
            });
        }

        self.queue
            .push_with_max_attempts(
                ImportParseJob {
                    session_id,
                    file_id,
                    user_id,
                },
                60,
            )
            .await?;
        Ok(())
    }

    async fn ensure_active(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<()> {
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
        if session.session_lifecycle()? != ImportSessionLifecycle::Active {
            return Err(not_mutable().into());
        }
        Ok(())
    }
}

fn not_attached() -> BusinessNotFoundError {
    BusinessNotFoundError {
        message: "File is not attached to the import session.".to_string(),
    }
}

fn not_mutable() -> BusinessConflictError {
    BusinessConflictError {
        message: "Import session can no longer be changed.".to_string(),
    }
}
