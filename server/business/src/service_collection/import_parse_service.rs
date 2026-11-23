#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::import_models::{
    ImportAssertionInsert, ImportAssertionKind, ImportDateOrder, ImportFeeType, ImportFileModel,
    ImportFileStatus, ImportParsedLegInsert, ImportParsedLineInsert,
};
use dal::queries::import_queries;
use dal::query_params::import_params::GetImportFilesParams;
use extraction::models::accepted::{AcceptedExtraction, DateOrder};
use extraction::models::output::{BalanceKind, ExtractedFeeType};
use time::{OffsetDateTime, Time};
use uuid::Uuid;

use super::import_classify_service::ImportClassifyService;
use crate::dtos::imports::import_file_dto::ImportProcessingFileDto;

const INSERT_CHUNK: usize = 1000;

pub struct ImportParseService {
    db: MyraDb,
    classify_service: ImportClassifyService,
}

impl ImportParseService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            classify_service: ImportClassifyService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id))]
    pub async fn start(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<()> {
        self.db
            .execute(import_queries::start_file_parse(
                session_id, file_id, user_id,
            ))
            .await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id))]
    pub async fn pause(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        until: OffsetDateTime,
    ) -> anyhow::Result<()> {
        self.db
            .execute(import_queries::pause_file_parse(
                session_id, file_id, user_id, until,
            ))
            .await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id))]
    pub async fn processing_file(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Option<ImportProcessingFileDto>> {
        let file: Option<ImportFileModel> = self
            .db
            .fetch_optional(import_queries::get_session_files(
                GetImportFilesParams::by_id(session_id, file_id, user_id),
            ))
            .await?;
        let Some(file) =
            file.filter(|file| matches!(file.file_status(), Ok(ImportFileStatus::Processing)))
        else {
            return Ok(None);
        };
        Ok(Some(ImportProcessingFileDto {
            conversation_id: file.conversation_id,
            date_order: file.file_date_order()?.map(Into::into),
        }))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id))]
    pub async fn save(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        accepted: &AcceptedExtraction,
    ) -> anyhow::Result<()> {
        self.db.start_transaction().await?;
        if let Err(error) = self.write(session_id, file_id, user_id, accepted).await {
            let _ = self.db.rollback_transaction().await;
            return Err(error);
        }
        self.db.commit_transaction().await?;
        self.classify_service
            .start_if_ready(session_id, user_id)
            .await
    }

    async fn write(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        accepted: &AcceptedExtraction,
    ) -> anyhow::Result<()> {
        let date_order = accepted
            .unsettled_date_order
            .map(|date_order| match date_order {
                DateOrder::DayFirst => ImportDateOrder::DayFirst,
                DateOrder::MonthFirst => ImportDateOrder::MonthFirst,
            });
        let claimed: Vec<Uuid> = self
            .db
            .fetch_all_scalar(import_queries::finish_file_parse(
                session_id, file_id, user_id, date_order,
            ))
            .await?;
        let Some(&source_id) = claimed.first() else {
            tracing::info!("discarded the parse of a file that changed while it was read");
            return Ok(());
        };

        let mut lines = Vec::with_capacity(accepted.lines.len());
        let mut legs = Vec::new();
        for line in &accepted.lines {
            let id = Uuid::now_v7();
            legs.extend(line.legs.iter().map(|leg| ImportParsedLegInsert {
                parsed_line_id: id,
                asset_ref: leg.asset_ref.clone(),
                quantity: leg.quantity,
                fee_type: leg.fee_type.map(|fee_type| match fee_type {
                    ExtractedFeeType::Transaction => ImportFeeType::Transaction,
                    ExtractedFeeType::Exchange => ImportFeeType::Exchange,
                    ExtractedFeeType::WithholdingTax => ImportFeeType::WithholdingTax,
                }),
            }));
            lines.push(ImportParsedLineInsert {
                id,
                session_id,
                source_id,
                transaction_key: line.transaction_key.clone(),
                account_ref: line.account_ref.clone(),
                date_transacted: line
                    .date
                    .with_time(line.time.unwrap_or(Time::MIDNIGHT))
                    .assume_utc(),
                description: line.description.clone(),
                related_asset_ref: line.related_asset_ref.clone(),
                external_id: line.external_id.clone(),
            });
        }
        let assertions: Vec<ImportAssertionInsert> = accepted
            .assertions
            .iter()
            .chain(&accepted.end_of_day_balances)
            .map(|assertion| ImportAssertionInsert {
                session_id,
                source_id,
                account_ref: assertion.account_ref.clone(),
                asset_ref: assertion.asset_ref.clone(),
                kind: match assertion.kind {
                    BalanceKind::Opening => ImportAssertionKind::Opening,
                    BalanceKind::Closing => ImportAssertionKind::Closing,
                    BalanceKind::Snapshot => ImportAssertionKind::Snapshot,
                    BalanceKind::ValueTotal => ImportAssertionKind::ValueTotal,
                },
                as_of_date: assertion.as_of_date,
                quantity: assertion.quantity,
            })
            .collect();
        self.db
            .execute(import_queries::delete_source_parsed_lines(
                session_id, source_id, user_id,
            ))
            .await?;
        self.db
            .execute(import_queries::delete_source_assertions(
                session_id, source_id, user_id,
            ))
            .await?;
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

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, file_id = %file_id))]
    pub async fn fail(
        &self,
        session_id: Uuid,
        file_id: Uuid,
        user_id: Uuid,
        message: String,
    ) -> anyhow::Result<()> {
        self.db
            .execute(import_queries::fail_file_parse(
                session_id, file_id, user_id, message,
            ))
            .await?;
        self.classify_service
            .start_if_ready(session_id, user_id)
            .await
    }
}
