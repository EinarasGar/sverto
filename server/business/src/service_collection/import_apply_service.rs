use std::collections::HashMap;

#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::import_models::{ImportSessionLifecycle, ImportSessionModel};
use dal::queries::import_queries;
use dal::query_params::import_params::GetImportSessionsParams;
use uuid::Uuid;

use super::connector_import_service::ConnectorImportService;
use super::import_file_service::ImportFileService;
use super::import_link_service::ImportLinkService;
use super::import_staging_service::ImportStagingService;
use super::ledger_matching_service::LedgerMatchingService;
use super::transaction_group_service::TransactionGroupService;
use super::transaction_management_service::TransactionManagementService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_file_dto::ImportFileStatusDto;
use crate::dtos::imports::import_staging_dto::ImportRowKindDto;
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::entities::imports::apply::ImportApply;

const INSERT_CHUNK: usize = 500;

pub struct ImportApplyService {
    db: MyraDb,
    connector_import_service: ConnectorImportService,
    file_service: ImportFileService,
    link_service: ImportLinkService,
    staging_service: ImportStagingService,
    transaction_service: TransactionManagementService,
    transaction_group_service: TransactionGroupService,
    ledger_matching_service: LedgerMatchingService,
}

impl ImportApplyService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            connector_import_service: ConnectorImportService::new(providers),
            file_service: ImportFileService::new(providers),
            link_service: ImportLinkService::new(providers),
            staging_service: ImportStagingService::new(providers),
            transaction_service: TransactionManagementService::new(providers),
            transaction_group_service: TransactionGroupService::new(providers),
            ledger_matching_service: LedgerMatchingService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn apply(&self, session_id: Uuid, user_id: Uuid, seen: Uuid) -> anyhow::Result<()> {
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
            return Err(conflict("This import has already been applied."));
        }
        let files = self.file_service.get_files(session_id, user_id).await?;
        if files
            .iter()
            .any(|file| file.status != ImportFileStatusDto::Parsed)
        {
            return Err(conflict(
                "Every file has to be read before the import can be applied.",
            ));
        }
        let links = self.link_service.links(session_id, user_id).await?;
        if !links.asks.is_empty() {
            return Err(conflict("Answer the open questions first."));
        }
        let Some(proposal) = self
            .staging_service
            .proposal(session_id, user_id, &links)
            .await?
        else {
            return Err(conflict("The import isn't ready for review yet."));
        };
        if proposal.rows.iter().any(|row| !row.near_miss.is_empty()) {
            return Err(conflict("Settle the possible duplicates first."));
        }
        if proposal
            .rows
            .iter()
            .any(|row| matches!(row.kind, ImportRowKindDto::StartingBalance(_)) && !row.verb_locked)
        {
            return Err(conflict("Decide the starting balances first."));
        }
        if proposal.fingerprint != seen {
            return Err(conflict(
                "The import changed since you looked at it. Review it again.",
            ));
        }
        let mut file_ids: Vec<Uuid> = files.iter().map(|file| file.file_id).collect();
        file_ids.sort();
        let apply = ImportApply::new(proposal);
        if let Some(contradiction) = apply.contradiction() {
            return Err(conflict(&format!(
                "{contradiction} Settle it before applying."
            )));
        }

        let amended = apply.amended()?;
        let (row_ids, mut transactions) = apply.inserts(user_id)?;
        let ibans = self
            .connector_import_service
            .linked_account_ibans(user_id, session_id)
            .await?;

        self.db.start_transaction().await?;
        for deletes in apply.deleted().chunks(INSERT_CHUNK) {
            self.transaction_service
                .delete_transactions_inner(user_id, deletes.to_vec())
                .await?;
        }

        for (transaction_id, transaction) in amended {
            self.transaction_service
                .update_individual_transaction_inner(user_id, transaction_id, transaction)
                .await?;
        }

        for parts in apply.groups() {
            self.transaction_group_service
                .group_existing_transactions_inner(
                    user_id,
                    parts.transaction_ids.clone(),
                    parts.description.clone(),
                    parts.category_id,
                    None,
                )
                .await?;
        }

        let mut created: HashMap<Uuid, Uuid> = HashMap::new();
        for (row_ids, transactions) in row_ids
            .chunks(INSERT_CHUNK)
            .zip(transactions.chunks_mut(INSERT_CHUNK))
        {
            self.transaction_service
                .add_transactions(transactions)
                .await?;
            for (row_id, transaction) in row_ids.iter().zip(transactions.iter()) {
                let transaction_id = transaction
                    .get_transaction_id()
                    .expect("an inserted transaction has its id");
                created.insert(*row_id, transaction_id);
            }
        }

        self.ledger_matching_service
            .record_sources(user_id, apply.sources(&created))
            .await?;
        self.connector_import_service
            .record_applied(user_id, session_id, apply.external_lines(&created), ibans)
            .await?;

        let completed = self
            .db
            .execute_with_rows_affected(import_queries::complete_session(
                session_id, user_id, file_ids,
            ))
            .await?;
        if completed == 0 {
            return Err(conflict(
                "The import changed while it was being applied. Review it again.",
            ));
        }
        Ok(self.db.commit_transaction().await?)
    }
}

fn conflict(message: &str) -> anyhow::Error {
    BusinessConflictError {
        message: message.to_string(),
    }
    .into()
}
