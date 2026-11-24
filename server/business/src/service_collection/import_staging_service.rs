use std::collections::{BTreeSet, HashSet};

#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::enums::asset_types::DatabaseAssetTypes;
use dal::models::import_models::{
    ImportLineClassificationModel, ImportSessionLifecycle, ImportSessionModel, ImportSourceKind,
    ImportSourceModel, ImportStagedDecisionModel, ImportVerb,
};
use dal::queries::import_queries;
use dal::query_params::import_params::GetImportSessionsParams;
use uuid::Uuid;

use super::asset_service::AssetsService;
use super::import_balance_service::ImportBalanceService;
use super::import_classify_service::ImportClassifyService;
use super::import_link_service::ImportLinkService;
use super::ledger_matching_service::LedgerMatchingService;
use super::transaction_management_service::TransactionManagementService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_ask_dto::{
    ImportAnswerDto, NoCheckpointAnswerDto, NotOnStatementAnswerDto,
};
use crate::dtos::imports::import_balance_dto::ImportBalanceChecksDto;
use crate::dtos::imports::import_staging_dto::{
    ImportLineClassificationDto, ImportProposalDto, ImportRowKindDto, ImportStagedDecisionDto,
    ImportVerbDto, ImportVerbPinDto,
};
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::dynamic_enums::{asset_types::AssetTypes, DynamicEnum};
use crate::entities::imports::balances;
use crate::entities::imports::linking::SessionLinks;
use crate::entities::imports::orientation;
use crate::entities::imports::proposal;
use crate::entities::imports::reconciliation::{parts_ask_key, SessionReconciliation};
use crate::entities::imports::staging::SessionStaging;
use crate::entities::imports::transfers;

pub struct ImportStagingService {
    db: MyraDb,
    classify_service: ImportClassifyService,
    link_service: ImportLinkService,
    assets_service: AssetsService,
    transaction_service: TransactionManagementService,
    ledger_matching_service: LedgerMatchingService,
    balance_service: ImportBalanceService,
}

impl ImportStagingService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            classify_service: ImportClassifyService::new(providers),
            link_service: ImportLinkService::new(providers),
            assets_service: AssetsService::new(providers),
            transaction_service: TransactionManagementService::new(providers),
            ledger_matching_service: LedgerMatchingService::new(providers),
            balance_service: ImportBalanceService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn proposal(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        links: &SessionLinks,
    ) -> anyhow::Result<Option<ImportProposalDto>> {
        if self
            .classify_service
            .classified_run(session_id, user_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let mut lines = self
            .classify_service
            .parsed_lines(session_id, user_id)
            .await?;
        let Some(run_id) = self
            .classify_service
            .classified_run(session_id, user_id)
            .await?
        else {
            return Ok(None);
        };
        let mut classifications: Vec<ImportLineClassificationDto> = self
            .db
            .fetch_all::<ImportLineClassificationModel>(import_queries::get_line_classifications(
                session_id, run_id, user_id,
            ))
            .await?
            .into_iter()
            .map(ImportLineClassificationDto::try_from)
            .collect::<anyhow::Result<_>>()?;
        let (mut printed, entered) = self.balance_service.assertions(session_id, user_id).await?;
        let mut file_sources = HashSet::new();
        for source in self
            .db
            .fetch_all::<ImportSourceModel>(import_queries::get_session_sources(
                session_id, user_id,
            ))
            .await?
        {
            if source.source_kind()? == ImportSourceKind::File {
                file_sources.insert(source.id);
            }
        }
        let owed = orientation::read_as_owed(
            &lines,
            &printed,
            &links.account_mappings,
            &links.account_types,
            &file_sources,
        );
        orientation::read_lines_as_owed(&mut lines, &mut classifications, &owed);
        orientation::read_assertions_as_owed(&mut printed, &owed);
        let currency_type = AssetTypes::try_into_dynamic_enum(DatabaseAssetTypes::Currency)?;
        let currency_assets: HashSet<i32> = self
            .assets_service
            .get_assets(
                links
                    .asset_mappings
                    .iter()
                    .filter_map(|mapping| mapping.asset_id)
                    .collect(),
            )
            .await?
            .into_iter()
            .filter(|asset| asset.asset_type.id == currency_type)
            .map(|asset| asset.id.0)
            .collect();

        let mut staging = SessionStaging::new(user_id);
        staging.add_lines(lines);
        staging.add_account_mappings(&links.account_mappings);
        staging.add_asset_mappings(&links.asset_mappings);
        staging.add_classifications(classifications);
        staging.add_currency_assets(currency_assets.iter().copied());
        let mut stage = staging.stage()?;

        let decisions: Vec<ImportStagedDecisionDto> = self
            .db
            .fetch_all::<ImportStagedDecisionModel>(import_queries::get_staged_decisions(
                session_id, user_id,
            ))
            .await?
            .into_iter()
            .map(ImportStagedDecisionDto::try_from)
            .collect::<anyhow::Result<_>>()?;
        let target_ids: Vec<Uuid> = decisions
            .iter()
            .filter(|decision| {
                matches!(decision.verb, ImportVerbDto::Delete | ImportVerbDto::Amend)
            })
            .filter_map(|decision| decision.transaction_id)
            .collect();
        let targets = self
            .transaction_service
            .get_transactions_by_ids(user_id, target_ids)
            .await?;
        transfers::join_decided(&mut stage, &decisions);
        let reconciliation = SessionReconciliation::new(&stage.staged, &decisions);
        let outcomes = self
            .ledger_matching_service
            .match_batches(user_id, reconciliation.batches())
            .await?;
        let matches = reconciliation.resolve(outcomes);
        transfers::join_new(&mut stage, &matches, &decisions);
        let mut proposal = proposal::merge(stage, decisions.clone(), targets, matches, user_id);
        proposal.read_as_owed = links
            .account_mappings
            .iter()
            .filter(|mapping| owed.contains(&(mapping.source_id, mapping.account_ref.clone())))
            .filter_map(|mapping| mapping.account_id)
            .collect::<BTreeSet<Uuid>>()
            .into_iter()
            .collect();

        if !links.asks.is_empty() {
            return Ok(Some(proposal));
        }
        let (mut balances, totals, ledger) = self
            .balance_service
            .read(user_id, printed, entered, &proposal.rows, links)
            .await?;
        let session: Option<ImportSessionModel> = self
            .db
            .fetch_optional(import_queries::get_session(GetImportSessionsParams::by_id(
                session_id, user_id,
            )))
            .await?;
        if session.is_some_and(|session| {
            matches!(
                session.session_lifecycle(),
                Ok(ImportSessionLifecycle::Active)
            )
        }) {
            proposal = proposal::add_starting_balances(
                proposal,
                session_id,
                balances.starting_balances(&totals, &ledger),
                &decisions,
                &currency_assets,
            );
            balances.add_rows(
                proposal
                    .rows
                    .iter()
                    .filter(|row| matches!(row.kind, ImportRowKindDto::StartingBalance(_))),
            );
            proposal = proposal::add_adjustments(
                proposal,
                session_id,
                balances::adjustments(&balances.check(&totals, &ledger)),
                &decisions,
                &currency_assets,
            );
            balances.add_rows(
                proposal
                    .rows
                    .iter()
                    .filter(|row| matches!(row.kind, ImportRowKindDto::Adjustment(_))),
            );
        }
        proposal.balance_checks = Some(balances.check(&totals, &ledger));
        Ok(Some(proposal))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, ask_key = %ask_key))]
    pub async fn answer_ask(
        &self,
        session_id: Uuid,
        ask_key: &str,
        user_id: Uuid,
        answer: ImportAnswerDto,
    ) -> anyhow::Result<()> {
        let not_open = || BusinessNotFoundError {
            message: "Ask is not open or does not exist.".to_string(),
        };
        match answer {
            ImportAnswerDto::NoCheckpoint(NoCheckpointAnswerDto::EnterBalance { quantity }) => {
                let enterable = self
                    .balance_checks(session_id, user_id)
                    .await?
                    .enterable
                    .into_iter()
                    .find(|enterable| enterable.ask_key == ask_key)
                    .ok_or_else(not_open)?;
                if self
                    .balance_service
                    .enter_balance(session_id, user_id, &enterable, quantity)
                    .await?
                    == 0
                {
                    return Err(not_open().into());
                }
                Ok(())
            }
            ImportAnswerDto::NotOnStatement(NotOnStatementAnswerDto::Delete) => {
                let mut transaction_ids: Vec<Uuid> = self
                    .balance_checks(session_id, user_id)
                    .await?
                    .checks
                    .iter()
                    .flat_map(|check| &check.unlisted)
                    .filter(|unlisted| unlisted.ask_key.as_deref() == Some(ask_key))
                    .map(|unlisted| unlisted.transaction_id)
                    .collect();
                transaction_ids.sort();
                transaction_ids.dedup();
                if transaction_ids.is_empty() {
                    return Err(not_open().into());
                }
                self.db
                    .execute(import_queries::insert_ask_decisions(
                        session_id,
                        user_id,
                        ask_key.to_string(),
                        ImportVerb::Delete,
                        transaction_ids
                            .into_iter()
                            .map(|transaction_id| (Uuid::now_v7(), transaction_id))
                            .collect(),
                    ))
                    .await
                    .map_err(|error| {
                        if error
                            .as_database_error()
                            .is_some_and(|error| error.is_foreign_key_violation())
                        {
                            anyhow::Error::new(not_open())
                        } else {
                            anyhow::Error::new(error)
                        }
                    })?;
                Ok(())
            }
            ImportAnswerDto::NotOnStatement(NotOnStatementAnswerDto::Keep) => {
                self.active_session(session_id, user_id).await?;
                self.db
                    .execute(import_queries::release_staged_decisions(
                        session_id,
                        user_id,
                        Vec::new(),
                        vec![ask_key.to_string()],
                    ))
                    .await?;
                Ok(())
            }
            answer => {
                self.link_service
                    .answer_ask(session_id, ask_key, user_id, answer)
                    .await
            }
        }
    }

    async fn balance_checks(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<ImportBalanceChecksDto> {
        self.active_session(session_id, user_id).await?;
        let links = self.link_service.links(session_id, user_id).await?;
        self.proposal(session_id, user_id, &links)
            .await?
            .and_then(|proposal| proposal.balance_checks)
            .ok_or_else(|| {
                BusinessNotFoundError {
                    message: "Ask is not open or does not exist.".to_string(),
                }
                .into()
            })
    }

    async fn active_session(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<()> {
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
            return Err(BusinessConflictError {
                message: "Import session can no longer be changed.".to_string(),
            }
            .into());
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn pin(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        item_ids: Vec<Uuid>,
        pin: ImportVerbPinDto,
    ) -> anyhow::Result<()> {
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
            return Err(BusinessConflictError {
                message: "Import session can no longer be changed.".to_string(),
            }
            .into());
        }
        let links = self.link_service.links(session_id, user_id).await?;
        let proposal = self
            .proposal(session_id, user_id, &links)
            .await?
            .ok_or_else(|| BusinessConflictError {
                message: "The import isn't ready for review yet.".to_string(),
            })?;
        let pinnable: HashSet<Uuid> = proposal
            .rows
            .iter()
            .filter(|row| row.ask_key.is_none())
            .map(|row| row.id)
            .collect();
        if item_ids.is_empty() || item_ids.iter().any(|id| !pinnable.contains(id)) {
            return Err(BusinessValidationErrorDto {
                errors: vec![BusinessFieldErrorDto {
                    field: "item_ids".to_string(),
                    message: "Every item must be a row of this import that can be pinned."
                        .to_string(),
                }],
            }
            .into());
        }
        let releases = pin == ImportVerbPinDto::Auto;
        let query = match pin {
            ImportVerbPinDto::Skip => import_queries::pin_staged_decisions(
                session_id,
                user_id,
                item_ids,
                ImportVerb::Skip,
                None,
            ),
            ImportVerbPinDto::Add => import_queries::pin_staged_decisions(
                session_id,
                user_id,
                item_ids,
                ImportVerb::Add,
                None,
            ),
            ImportVerbPinDto::Auto => {
                let parts_keys = item_ids.iter().map(|id| parts_ask_key(*id)).collect();
                import_queries::release_staged_decisions(session_id, user_id, item_ids, parts_keys)
            }
            ImportVerbPinDto::Apart => {
                let sides: Option<Vec<[Uuid; 2]>> = item_ids
                    .iter()
                    .map(|item_id| {
                        proposal
                            .rows
                            .iter()
                            .find(|row| row.id == *item_id)
                            .and_then(|row| match row.kind {
                                ImportRowKindDto::JoinedTransfer(sides) => Some(sides),
                                _ => None,
                            })
                    })
                    .collect();
                let Some(sides) = sides else {
                    return Err(BusinessValidationErrorDto {
                        errors: vec![BusinessFieldErrorDto {
                            field: "item_ids".to_string(),
                            message: "Only a transfer joined from two rows can be kept apart."
                                .to_string(),
                        }],
                    }
                    .into());
                };
                self.db.start_transaction().await?;
                self.db
                    .execute(import_queries::release_staged_decisions(
                        session_id,
                        user_id,
                        item_ids,
                        Vec::new(),
                    ))
                    .await?;
                let pinned = self
                    .db
                    .execute_with_rows_affected(import_queries::pin_staged_decisions(
                        session_id,
                        user_id,
                        sides.concat(),
                        ImportVerb::Add,
                        None,
                    ))
                    .await?;
                if pinned == 0 {
                    return Err(BusinessConflictError {
                        message: "Import session can no longer be changed.".to_string(),
                    }
                    .into());
                }
                return Ok(self.db.commit_transaction().await?);
            }
            ImportVerbPinDto::Same { candidate_id } => {
                let candidate = match item_ids[..] {
                    [item_id] => {
                        proposal
                            .rows
                            .iter()
                            .find(|row| row.id == item_id)
                            .and_then(|row| {
                                row.near_miss
                                    .iter()
                                    .find(|near| near.candidate.id == candidate_id)
                            })
                    }
                    _ => None,
                };
                let Some(near) = candidate else {
                    return Err(BusinessValidationErrorDto {
                        errors: vec![BusinessFieldErrorDto {
                            field: "candidate_id".to_string(),
                            message: "Pick one of the row's suggested transactions.".to_string(),
                        }],
                    }
                    .into());
                };
                if !near.parts.is_empty() {
                    let decisions = near
                        .parts
                        .iter()
                        .map(|part| (Uuid::now_v7(), part.id))
                        .collect();
                    let stored = self
                        .db
                        .execute_with_rows_affected(import_queries::insert_ask_decisions(
                            session_id,
                            user_id,
                            parts_ask_key(item_ids[0]),
                            ImportVerb::Skip,
                            decisions,
                        ))
                        .await
                        .map_err(|error| {
                            if error
                                .as_database_error()
                                .is_some_and(|error| error.is_foreign_key_violation())
                            {
                                anyhow::Error::new(BusinessValidationErrorDto {
                                    errors: vec![BusinessFieldErrorDto {
                                        field: "candidate_id".to_string(),
                                        message: "Pick one of the row's suggested transactions."
                                            .to_string(),
                                    }],
                                })
                            } else {
                                anyhow::Error::new(error)
                            }
                        })?;
                    if stored == 0 {
                        return Err(BusinessConflictError {
                            message: "Import session can no longer be changed.".to_string(),
                        }
                        .into());
                    }
                    return Ok(());
                }
                let (verb, transaction_id) = match (near.candidate.in_ledger, near.amend) {
                    (false, _) => (ImportVerb::Skip, None),
                    (true, true) => (ImportVerb::Amend, Some(candidate_id)),
                    (true, false) => (ImportVerb::Skip, Some(candidate_id)),
                };
                import_queries::pin_staged_decisions(
                    session_id,
                    user_id,
                    item_ids,
                    verb,
                    transaction_id,
                )
            }
        };
        if self.db.execute_with_rows_affected(query).await? == 0 && !releases {
            return Err(BusinessConflictError {
                message: "Import session can no longer be changed.".to_string(),
            }
            .into());
        }
        Ok(())
    }
}
