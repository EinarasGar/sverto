use std::collections::{HashMap, HashSet};

use anyhow::Context;
#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::job_queue::JobQueueHandle;
use dal::models::connector_models::{
    ConnectorTransactionRow, HeldTransactionRow, SyncedTransactionLineRow,
};
use dal::queries::connector_queries;
use dal::query_params::connector_params::{
    DeleteHeldTransactionsParams, GetConnectorTransactionsParams, GetHeldTransactionsParams,
};
use itertools::Itertools;
use time::Duration;
use uuid::Uuid;

use super::accounts_service::AccountsService;
use super::entries_service::EntriesService;
use super::ledger_matching_service::LedgerMatchingService;
use super::transaction_management_service::TransactionManagementService;
use super::user_service::UsersService;
use super::ServiceProviders;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::connectors::{
    HeldDecisionDto, HeldTransactionDto, HeldTransactionWithCandidatesDto, SyncedTransactionLineDto,
};
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::dtos::transaction_dto::TransactionDto;
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::entities::connectors::bank_ingestion::{
    synced_visibility, BankIngestionWrites, BankRow, SameDecision,
};
use crate::entities::connectors::provider_transaction_import::IMPORT_CATEGORY_ID;
use crate::entities::ledger_matching::WINDOW_DAYS;
use crate::entities::transactions::transaction::Transaction;
use crate::jobs::CategorizeSyncedTransactionsJob;

pub struct BankIngestionService {
    db: MyraDb,
    queue: JobQueueHandle,
    transaction_management: TransactionManagementService,
    ledger_matching: LedgerMatchingService,
    accounts: AccountsService,
    entries: EntriesService,
    users: UsersService,
}

impl BankIngestionService {
    pub fn new(providers: &ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            queue: providers.job_queue.clone(),
            transaction_management: TransactionManagementService::new(providers),
            ledger_matching: LedgerMatchingService::new(providers),
            accounts: AccountsService::new(providers),
            entries: EntriesService::new(providers),
            users: UsersService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, binding_id = %binding_id))]
    pub async fn held_external_ids(
        &self,
        user_id: Uuid,
        binding_id: Uuid,
        external_ids: Vec<String>,
    ) -> anyhow::Result<HashSet<String>> {
        if external_ids.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(self
            .db
            .fetch_all::<HeldTransactionRow>(connector_queries::get_held_transactions(
                GetHeldTransactionsParams::by_external_ids(user_id, binding_id, external_ids),
            ))
            .await?
            .into_iter()
            .map(|row| row.external_id)
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub(crate) async fn pair_transfers(
        &self,
        user_id: Uuid,
        writes: &mut BankIngestionWrites,
    ) -> anyhow::Result<()> {
        let ibans = writes.counterparty_ibans();
        if ibans.is_empty() {
            return Ok(());
        }
        let own_accounts = self.accounts.accounts_by_iban(user_id, ibans).await?;
        let Some((from, to)) = writes.transfer_window(&own_accounts) else {
            return Ok(());
        };
        let movements = self
            .ledger_matching
            .activity(user_id, from, to)
            .await?
            .movements;
        let candidate_ids = writes.transfer_candidates(&own_accounts, &movements);
        if candidate_ids.is_empty() {
            return Ok(());
        }
        let candidates = self
            .transaction_management
            .get_ungrouped_transactions_by_ids(user_id, candidate_ids.clone())
            .await?;
        let links = self
            .db
            .fetch_all::<ConnectorTransactionRow>(connector_queries::get_connector_transactions(
                GetConnectorTransactionsParams::by_transaction_ids(user_id, candidate_ids),
            ))
            .await?;
        writes.pair_transfers(&own_accounts, candidates, &links);
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub(crate) async fn write(
        &self,
        user_id: Uuid,
        mut writes: BankIngestionWrites,
    ) -> anyhow::Result<Vec<Uuid>> {
        let (rows, mut transactions): (Vec<BankRow>, Vec<Transaction>) =
            std::mem::take(&mut writes.rows).into_iter().unzip();
        if !transactions.is_empty() {
            self.transaction_management
                .add_transactions(&mut transactions)
                .await?;
        }
        if !writes.others.is_empty() {
            self.transaction_management
                .add_transactions(&mut writes.others)
                .await?;
        }
        let written = rows
            .into_iter()
            .zip(&transactions)
            .map(|(row, transaction)| {
                let transaction_id = transaction
                    .get_transaction_id()
                    .context("an inserted transaction has its id")?;
                Ok((row, transaction_id))
            })
            .collect::<anyhow::Result<Vec<(BankRow, Uuid)>>>()?;
        for transfer in std::mem::take(&mut writes.transfers) {
            let transaction_id = transfer
                .transaction_id
                .context("a transfer's other leg is a ledger transaction")?;
            self.transaction_management
                .update_individual_transaction_inner(user_id, transaction_id, transfer)
                .await?;
        }
        if !writes.links.is_empty() {
            self.db
                .execute(connector_queries::insert_connector_transactions(
                    &writes.links,
                ))
                .await?;
        }
        if !writes.held.is_empty() {
            self.db
                .execute(connector_queries::insert_held_transactions(&writes.held))
                .await?;
        }
        self.ledger_matching
            .record_sources(user_id, writes.sources(&written))
            .await?;
        Ok(written
            .into_iter()
            .map(|(_, transaction_id)| transaction_id)
            .collect())
    }

    pub(crate) async fn queue_categorization(&self, user_id: Uuid, transaction_ids: Vec<Uuid>) {
        if transaction_ids.is_empty() {
            return;
        }
        let queued = async {
            if !self.users.is_ai_features_enabled(user_id).await? {
                return Ok(());
            }
            self.queue
                .push(CategorizeSyncedTransactionsJob {
                    user_id,
                    transaction_ids,
                })
                .await
        }
        .await;
        if let Err(error) = queued {
            tracing::error!(error = ?error, "queueing the categorisation of synced transactions failed; they keep the default category");
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn held_transactions(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<HeldTransactionWithCandidatesDto>> {
        let held: Vec<HeldTransactionDto> = self
            .db
            .fetch_all::<HeldTransactionRow>(connector_queries::get_held_transactions(
                GetHeldTransactionsParams::all(user_id),
            ))
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let candidate_ids: Vec<Uuid> = held
            .iter()
            .flat_map(|held| held.candidate_ids.iter().copied())
            .collect::<HashSet<Uuid>>()
            .into_iter()
            .collect();
        let linked_bindings: HashMap<Uuid, Vec<Uuid>> = self
            .db
            .fetch_all::<ConnectorTransactionRow>(connector_queries::get_connector_transactions(
                GetConnectorTransactionsParams::by_transaction_ids(user_id, candidate_ids.clone()),
            ))
            .await?
            .into_iter()
            .filter_map(|link| Some((link.transaction_id?, link.binding_id)))
            .into_group_map();
        let candidates: HashMap<Uuid, TransactionDto> = self
            .transaction_management
            .get_transactions_by_ids(user_id, candidate_ids)
            .await?
            .into_iter()
            .filter_map(|transaction| Some((transaction.transaction_id?, transaction)))
            .collect();
        Ok(held
            .into_iter()
            .map(|held| {
                let row = BankRow::from(&held);
                HeldTransactionWithCandidatesDto {
                    candidates: held
                        .candidate_ids
                        .iter()
                        .filter(|id| {
                            row.can_be_recorded_on(
                                linked_bindings.get(id).map_or(&[], Vec::as_slice),
                            )
                        })
                        .filter_map(|id| candidates.get(id).cloned())
                        .collect(),
                    held,
                }
            })
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, held_id = %held_id))]
    pub async fn decide(
        &self,
        user_id: Uuid,
        held_id: Uuid,
        decision: HeldDecisionDto,
    ) -> anyhow::Result<()> {
        let held: HeldTransactionDto = self
            .db
            .fetch_optional::<HeldTransactionRow>(connector_queries::get_held_transactions(
                GetHeldTransactionsParams::by_id(user_id, held_id),
            ))
            .await?
            .ok_or_else(|| BusinessNotFoundError {
                message: "Held transaction not found.".to_string(),
            })?
            .into();
        let row = BankRow::from(&held);
        if let HeldDecisionDto::Same { transaction_id } = decision {
            if !held.candidate_ids.contains(&transaction_id) {
                return Err(BusinessValidationErrorDto {
                    errors: vec![BusinessFieldErrorDto {
                        field: "transaction_id".to_string(),
                        message: "Pick one of the transactions offered for this bank row."
                            .to_string(),
                    }],
                }
                .into());
            }
        }

        self.db.start_transaction().await?;
        self.remove_held(user_id, held_id).await?;
        let recorded = !self
            .db
            .fetch_all::<ConnectorTransactionRow>(connector_queries::get_connector_transactions(
                GetConnectorTransactionsParams::by_external_ids(
                    row.binding_id,
                    vec![row.external_id.clone()],
                ),
            ))
            .await?
            .is_empty();
        if recorded {
            self.db.commit_transaction().await?;
            return Err(BusinessConflictError {
                message: "This bank row is already in your ledger.".to_string(),
            }
            .into());
        }
        let categorize = match decision {
            HeldDecisionDto::Same { transaction_id } => {
                let target = self
                    .transaction_management
                    .get_transactions_by_ids(user_id, vec![transaction_id])
                    .await?
                    .pop()
                    .ok_or_else(|| BusinessNotFoundError {
                        message: "Transaction not found.".to_string(),
                    })?;
                let around = target.date.date();
                let ledger = self
                    .ledger_matching
                    .activity(
                        user_id,
                        around - Duration::days(WINDOW_DAYS),
                        around + Duration::days(WINDOW_DAYS),
                    )
                    .await?;
                match row.same_as(target, ledger, user_id)? {
                    SameDecision::Refused => {
                        return Err(BusinessConflictError {
                            message: "That transaction already has another payment from this bank account. Pick another one, or add this row as a transaction of its own.".to_string(),
                        }
                        .into());
                    }
                    SameDecision::Attach => {}
                    SameDecision::Amend(amended) => {
                        self.transaction_management
                            .update_individual_transaction_inner(user_id, transaction_id, amended)
                            .await?;
                    }
                }
                self.db
                    .execute(connector_queries::insert_connector_transactions(&[
                        row.link(transaction_id)
                    ]))
                    .await?;
                self.ledger_matching
                    .record_sources(user_id, vec![row.source(transaction_id)])
                    .await?;
                Vec::new()
            }
            HeldDecisionDto::Different => {
                let mut transactions =
                    vec![row.transaction(user_id, synced_visibility(&held.write_mode))?];
                self.transaction_management
                    .add_transactions(&mut transactions)
                    .await?;
                let transaction_id = transactions[0]
                    .get_transaction_id()
                    .context("an inserted transaction has its id")?;
                self.ledger_matching
                    .record_sources(user_id, vec![row.source(transaction_id)])
                    .await?;
                vec![transaction_id]
            }
        };
        self.db.commit_transaction().await?;
        self.queue_categorization(user_id, categorize).await;
        Ok(())
    }

    async fn remove_held(&self, user_id: Uuid, held_id: Uuid) -> anyhow::Result<()> {
        let removed = self
            .db
            .execute_with_rows_affected(connector_queries::delete_held_transactions(
                DeleteHeldTransactionsParams::by_id(user_id, held_id),
            ))
            .await?;
        if removed == 0 {
            return Err(BusinessConflictError {
                message: "This bank row was already decided.".to_string(),
            }
            .into());
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn synced_transactions_to_categorize(
        &self,
        user_id: Uuid,
        transaction_ids: Vec<Uuid>,
    ) -> anyhow::Result<Vec<SyncedTransactionLineDto>> {
        Ok(self
            .db
            .fetch_all::<SyncedTransactionLineRow>(
                connector_queries::get_synced_transactions_to_categorize(
                    user_id,
                    transaction_ids,
                    IMPORT_CATEGORY_ID,
                ),
            )
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn categorize_synced_transactions(
        &self,
        user_id: Uuid,
        categories: Vec<(Uuid, i32)>,
    ) -> anyhow::Result<u64> {
        self.entries
            .replace_categories(user_id, IMPORT_CATEGORY_ID, categories)
            .await
    }
}
