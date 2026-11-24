#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::job_queue::JobQueueHandle;
use dal::models::ai_conversation_models::QuickUploadModel;
use dal::pg_notify_connection::{PgNotifyConnection, PgNotifyEvent};
use dal::queries::{ai_conversation_queries, ai_quick_upload_queries};
use dal::query_params::ai_conversation_params::{
    GetQuickUploadsParams, ProposalType, QuickUploadStatus, UpdateConversationErrorParams,
};

use anyhow::Context;
use itertools::Itertools;
use rust_decimal::Decimal;
use time::format_description::well_known::Iso8601;
use time::Date;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::dtos::ai_error_dto::AiErrorDto;
use crate::dtos::ai_quick_upload_dto::{
    MergeQuickUploadDto, QuickUploadDto, QuickUploadNotification, ReceiptProposalDto,
};
use crate::dtos::entry_dto::EntryDto;
use crate::dtos::ledger_matching_dto::{
    IncomingMovementDto, LedgerMatchBatchDto, MatchOutcomeDto, SourceKindDto, TransactionSourceDto,
};
use crate::dtos::transaction_dto::{
    RegularTransactionMetadataDto, TransactionDto, TransactionTypeDto,
};
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::jobs::QuickUploadJob;
use crate::service_collection::ledger_matching_service::LedgerMatchingService;
use crate::service_collection::transaction_group_service::TransactionGroupService;
use crate::service_collection::transaction_management_service::TransactionManagementService;
use crate::service_collection::user_service::UsersService;

const RECEIPT_STREAM: &str = "receipt";

pub struct AiQuickUploadService {
    db: MyraDb,
    pg_notify: PgNotifyConnection,
    queue: JobQueueHandle,
    users_service: UsersService,
    ledger_matching_service: LedgerMatchingService,
    management_service: TransactionManagementService,
    group_service: TransactionGroupService,
}

impl AiQuickUploadService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            pg_notify: providers.pg_notify.clone(),
            queue: providers.job_queue.clone(),
            users_service: UsersService::new(providers),
            ledger_matching_service: LedgerMatchingService::new(providers),
            management_service: TransactionManagementService::new(providers),
            group_service: TransactionGroupService::new(providers),
        }
    }

    pub async fn create_quick_upload(
        &self,
        user_id: Uuid,
        source_file_id: Uuid,
    ) -> anyhow::Result<QuickUploadDto> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;

        let conv_query = ai_conversation_queries::create_conversation(user_id);
        let conv_id: Uuid = self.db.fetch_one_scalar(conv_query).await?;

        let qu_query = ai_quick_upload_queries::create_quick_upload(conv_id, source_file_id);
        let qu_id: Uuid = self.db.fetch_one_scalar(qu_query).await?;

        self.queue
            .push(QuickUploadJob::Process {
                quick_upload_id: qu_id,
                user_id,
            })
            .await?;

        Ok(QuickUploadDto {
            id: qu_id,
            conversation_id: conv_id,
            status: QuickUploadStatus::Pending,
            source_file_id,
            proposal_type: None,
            proposal_data: None,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
        })
    }

    pub async fn get_quick_upload(
        &self,
        quick_upload_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<QuickUploadDto> {
        let query = ai_quick_upload_queries::get_quick_uploads(GetQuickUploadsParams::by_id(
            quick_upload_id,
            user_id,
        ));
        let model: QuickUploadModel = self.db.fetch_one(query).await?;
        Ok(model.into())
    }

    pub async fn get_quick_uploads(
        &self,
        user_id: Uuid,
        unfinished_only: bool,
    ) -> anyhow::Result<Vec<QuickUploadDto>> {
        let status_filter = if unfinished_only {
            Some(vec![
                QuickUploadStatus::Pending,
                QuickUploadStatus::Processing,
                QuickUploadStatus::Retrying,
                QuickUploadStatus::ProposalReady,
                QuickUploadStatus::Failed,
            ])
        } else {
            None
        };

        let query = ai_quick_upload_queries::get_quick_uploads(GetQuickUploadsParams::all(
            user_id,
            status_filter,
            0,
            50,
        ));
        let models: Vec<QuickUploadModel> = self.db.fetch_all(query).await?;
        Ok(models.into_iter().map_into().collect())
    }

    pub async fn subscribe(&self, quick_upload_id: Uuid) -> broadcast::Receiver<PgNotifyEvent> {
        let entity_id = quick_upload_id.to_string();
        self.pg_notify.subscribe(&entity_id).await
    }

    pub async fn notify(
        &self,
        quick_upload_id: Uuid,
        event: QuickUploadNotification,
    ) -> anyhow::Result<()> {
        let entity_id = quick_upload_id.to_string();
        self.pg_notify.notify(&entity_id, &event).await?;
        Ok(())
    }

    pub async fn update_status(
        &self,
        quick_upload_id: Uuid,
        status: QuickUploadStatus,
    ) -> anyhow::Result<()> {
        let query = ai_quick_upload_queries::update_quick_upload_status(quick_upload_id, status);
        self.db.execute(query).await?;
        Ok(())
    }

    pub async fn set_proposal(
        &self,
        quick_upload_id: Uuid,
        proposal_type: ProposalType,
        proposal_data: serde_json::Value,
    ) -> anyhow::Result<()> {
        let query = ai_quick_upload_queries::update_quick_upload_proposal(
            quick_upload_id,
            proposal_type.to_string(),
            proposal_data,
        );
        self.db.execute(query).await?;
        Ok(())
    }

    pub async fn complete(
        &self,
        quick_upload_id: Uuid,
        user_id: Uuid,
        accepted: bool,
    ) -> anyhow::Result<()> {
        self.get_quick_upload(quick_upload_id, user_id).await?;
        let status = if accepted {
            QuickUploadStatus::Accepted
        } else {
            QuickUploadStatus::Rejected
        };
        self.update_status(quick_upload_id, status).await?;
        Ok(())
    }

    pub async fn record_conversation_error(
        &self,
        quick_upload_id: Uuid,
        error: &AiErrorDto,
    ) -> anyhow::Result<()> {
        let query = ai_conversation_queries::update_conversation_error(
            UpdateConversationErrorParams::set_for_quick_upload(quick_upload_id, error),
        );
        self.db.execute(query).await?;
        Ok(())
    }

    pub async fn retry(&self, quick_upload_id: Uuid, user_id: Uuid) -> anyhow::Result<bool> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        self.get_quick_upload(quick_upload_id, user_id).await?;
        let query = ai_quick_upload_queries::try_reset_for_manual_retry(quick_upload_id);
        let rows = self.db.execute_with_rows_affected(query).await?;
        if rows == 0 {
            return Ok(false);
        }
        self.queue
            .push(QuickUploadJob::Retry {
                quick_upload_id,
                user_id,
            })
            .await?;
        Ok(true)
    }

    pub async fn enqueue_correction(
        &self,
        quick_upload_id: Uuid,
        user_id: Uuid,
        message: String,
    ) -> anyhow::Result<()> {
        self.users_service
            .ensure_ai_features_enabled(user_id)
            .await?;
        self.get_quick_upload(quick_upload_id, user_id).await?;

        self.update_status(quick_upload_id, QuickUploadStatus::Processing)
            .await?;

        self.queue
            .push(QuickUploadJob::Correction {
                quick_upload_id,
                user_id,
                message,
            })
            .await?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(quick_upload_id = %quick_upload_id, user_id = %user_id))]
    pub async fn ledger_matches(
        &self,
        quick_upload_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<TransactionDto>> {
        let quick_upload = self.get_quick_upload(quick_upload_id, user_id).await?;
        let (Some(proposal_type), Some(data)) =
            (quick_upload.proposal_type, quick_upload.proposal_data)
        else {
            return Ok(Vec::new());
        };
        let receipt: ReceiptProposalDto =
            serde_json::from_value(data).context("the quick upload's proposal can't be read")?;
        let lines: Vec<(Option<Decimal>, Option<i32>)> = match proposal_type {
            ProposalType::TransactionGroup => receipt
                .transactions
                .unwrap_or_default()
                .into_iter()
                .map(|line| (line.amount, line.asset_id))
                .collect(),
            ProposalType::Transaction => vec![(receipt.amount, receipt.asset_id)],
        };
        let total: Option<Decimal> = lines.iter().map(|(amount, _)| *amount).sum();
        let asset_id = lines
            .iter()
            .map(|(_, asset_id)| *asset_id)
            .all_equal_value()
            .ok()
            .flatten();
        let date = receipt
            .date
            .and_then(|date| Date::parse(&date, &Iso8601::DATE).ok());
        let (Some(total), Some(asset_id), Some(date)) = (total, asset_id, date) else {
            return Ok(Vec::new());
        };

        let receipt_movement = IncomingMovementDto {
            id: quick_upload_id,
            kind: SourceKindDto::Receipt,
            stream: RECEIPT_STREAM.to_string(),
            external_ref: Some(quick_upload_id.to_string()),
            account_id: None,
            asset_id,
            quantity: total,
            date,
            description: receipt.description,
            itemised_fee: None,
            is_fee: false,
        };
        let outcomes = self
            .ledger_matching_service
            .match_batches(
                user_id,
                &[LedgerMatchBatchDto {
                    incoming: vec![receipt_movement],
                    settled: Vec::new(),
                }],
            )
            .await?;
        let matched_ids: Vec<Uuid> = outcomes
            .into_iter()
            .flatten()
            .flat_map(|outcome| match outcome {
                MatchOutcomeDto::Same { targets } => targets,
                MatchOutcomeDto::Ambiguous { ranked } => ranked,
                MatchOutcomeDto::Recorded { .. }
                | MatchOutcomeDto::Parts { .. }
                | MatchOutcomeDto::New => Vec::new(),
            })
            .filter(|candidate| candidate.in_ledger && !candidate.several_transactions)
            .map(|candidate| candidate.id)
            .unique()
            .collect();
        if matched_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut matches = self
            .management_service
            .get_ungrouped_transactions_by_ids(user_id, matched_ids.clone())
            .await?;
        matches.retain(|transaction| {
            matches!(&transaction.transaction_type,
                TransactionTypeDto::Regular(regular) if regular.entry.quantity == total)
        });
        matches.sort_by_key(|transaction| {
            matched_ids
                .iter()
                .position(|id| Some(*id) == transaction.transaction_id)
        });
        Ok(matches)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(quick_upload_id = %quick_upload_id, user_id = %user_id))]
    pub async fn merge(
        &self,
        quick_upload_id: Uuid,
        user_id: Uuid,
        merge: MergeQuickUploadDto,
    ) -> anyhow::Result<()> {
        self.get_quick_upload(quick_upload_id, user_id).await?;
        let Some(TransactionDto {
            transaction_type: TransactionTypeDto::Regular(existing),
            visibility,
            mut fee_entries,
            ..
        }) = self
            .management_service
            .get_ungrouped_transactions_by_ids(user_id, vec![merge.transaction_id])
            .await?
            .pop()
        else {
            return Err(merge_error(
                "transaction_id",
                "Only a spending transaction outside a group can take a receipt",
            ));
        };
        let total: Decimal = merge.lines.iter().map(|line| line.amount).sum();
        if total != existing.entry.quantity {
            return Err(merge_error(
                "lines",
                "The receipt lines must add up exactly to the transaction's amount",
            ));
        }
        let account_id = existing.entry.account_id;
        let asset_id = existing.entry.asset_id;
        let lines: Vec<TransactionDto> = merge
            .lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let first = index == 0;
                TransactionDto {
                    transaction_id: first.then_some(merge.transaction_id),
                    date: merge.date,
                    visibility: visibility.clone(),
                    fee_entries: if first {
                        std::mem::take(&mut fee_entries)
                    } else {
                        Vec::new()
                    },
                    transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                        description: line.description,
                        entry: EntryDto {
                            entry_id: existing.entry.entry_id.filter(|_| first),
                            ..EntryDto::new(asset_id, account_id, line.amount)
                        },
                        category_id: line.category_id,
                    }),
                }
            })
            .collect();

        self.db.start_transaction().await?;
        match <[TransactionDto; 1]>::try_from(lines) {
            Ok([line]) => {
                self.management_service
                    .update_individual_transaction_inner(user_id, merge.transaction_id, line)
                    .await?;
            }
            Err(lines) => {
                let description = merge
                    .description
                    .clone()
                    .context("a merge request with several lines has a description")?;
                self.group_service
                    .group_individual_transactions_inner(
                        user_id,
                        description,
                        merge.category_id,
                        merge.date,
                        lines,
                    )
                    .await?;
            }
        }
        self.ledger_matching_service
            .record_sources(
                user_id,
                vec![TransactionSourceDto {
                    transaction_id: merge.transaction_id,
                    account_id,
                    asset_id,
                    date: merge.date.date(),
                    description: merge.description,
                    kind: SourceKindDto::Receipt,
                    stream: RECEIPT_STREAM.to_string(),
                    external_ref: Some(quick_upload_id.to_string()),
                    itemised_fee: None,
                }],
            )
            .await?;
        self.update_status(quick_upload_id, QuickUploadStatus::Accepted)
            .await?;
        self.db.commit_transaction().await?;
        Ok(())
    }
}

fn merge_error(field: &str, message: &str) -> anyhow::Error {
    BusinessValidationErrorDto {
        errors: vec![BusinessFieldErrorDto {
            field: field.to_string(),
            message: message.to_string(),
        }],
    }
    .into()
}
