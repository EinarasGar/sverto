#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::base::Count;
use dal::models::import_models::{
    ImportAssertionModel, ImportSessionLifecycle, ImportSessionModel,
};
use dal::queries::{asset_queries, import_queries};
use dal::query_params::get_asset_refs_params::GetAssetRefsParams;
use dal::query_params::import_params::GetImportSessionsParams;
use uuid::Uuid;

use super::accounts_service::AccountsService;
use super::asset_service::AssetsService;
use super::import_classify_service::ImportClassifyService;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::imports::import_ask_dto::{
    AccountMappingAnswerDto, AssetMappingAnswerDto, ImportAnswerDto, ImportAskChoiceDto,
    ImportAskDto,
};
use crate::dtos::imports::import_balance_dto::ImportAssertionDto;
use crate::dtos::imports::import_pending_dto::ImportPendingLineDto;
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::entities::imports::linking::{SessionLinking, SessionLinks, ASSET_SUGGESTIONS};

pub struct ImportLinkService {
    db: MyraDb,
    accounts_service: AccountsService,
    assets_service: AssetsService,
    classify_service: ImportClassifyService,
}

impl ImportLinkService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            accounts_service: AccountsService::new(providers),
            assets_service: AssetsService::new(providers),
            classify_service: ImportClassifyService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn links(&self, session_id: Uuid, user_id: Uuid) -> anyhow::Result<SessionLinks> {
        let mut linking = SessionLinking::new();
        linking.add_accounts(
            self.accounts_service
                .get_user_accounts_with_metadata(user_id)
                .await?,
        );
        linking.add_account_refs(
            self.db
                .fetch_all(import_queries::get_session_account_refs(
                    session_id, user_id,
                ))
                .await?,
        );
        linking.add_account_answers(
            self.db
                .fetch_all(import_queries::get_account_mappings(session_id, user_id))
                .await?,
        );
        linking.add_asset_refs(
            self.db
                .fetch_all(import_queries::get_session_asset_refs(session_id, user_id))
                .await?,
        );
        linking.add_asset_answers(
            self.db
                .fetch_all(import_queries::get_asset_mappings(session_id, user_id))
                .await?,
        );

        if linking.repeats_an_answered_name() {
            linking.add_balances(
                self.db
                    .fetch_all::<ImportAssertionModel>(import_queries::get_assertions(
                        session_id, user_id,
                    ))
                    .await?
                    .into_iter()
                    .filter(|assertion| assertion.file_id.is_some())
                    .map(ImportAssertionDto::try_from)
                    .collect::<anyhow::Result<_>>()?,
            );
        }
        linking.link_accounts();
        let search_terms = linking.asset_search_terms();
        if !search_terms.is_empty() {
            linking.add_asset_matches(
                self.db
                    .fetch_all(asset_queries::get_asset_refs(
                        GetAssetRefsParams::by_tickers_symbols_isins_or_ids(
                            user_id,
                            search_terms,
                            linking.answered_asset_ids(),
                        ),
                    ))
                    .await?,
            );
        }
        let exchanges = linking.exchanges_of_candidates();
        if !exchanges.is_empty() {
            linking.add_exchange_currencies(
                self.db
                    .fetch_all(asset_queries::get_exchange_currencies(exchanges))
                    .await?,
            );
        }
        for asset_ref in linking.assets_needing_suggestions() {
            let assets = self
                .assets_service
                .search_visible_assets(user_id, asset_ref.clone(), ASSET_SUGGESTIONS as u64)
                .await?;
            linking.add_asset_suggestions(asset_ref, assets);
        }
        Ok(linking.link())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn pending(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<ImportPendingLineDto>> {
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
        let links = self.links(session_id, user_id).await?;
        let lines = self
            .classify_service
            .parsed_lines(session_id, user_id)
            .await?;
        Ok(links.pending_lines(lines))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id, ask_key = %ask_key))]
    pub async fn answer_ask(
        &self,
        session_id: Uuid,
        ask_key: &str,
        user_id: Uuid,
        answer: ImportAnswerDto,
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
        let links = self.links(session_id, user_id).await?;
        let Some(open) = links.asks.iter().find(|open| open.ask_key == ask_key) else {
            return Err(not_open().into());
        };

        let query = match (&open.ask, answer) {
            (ImportAskDto::AccountMapping(ask), ImportAnswerDto::AccountMapping(choice)) => {
                let account_id = match choice {
                    AccountMappingAnswerDto::Map { account_id } => {
                        let accounts = self
                            .accounts_service
                            .get_user_accounts_with_metadata(user_id)
                            .await?;
                        if !accounts.iter().any(|account| account.id == account_id) {
                            return Err(invalid("account_id", "Pick one of your accounts.").into());
                        }
                        Some(account_id)
                    }
                    AccountMappingAnswerDto::Exclude => None,
                };
                import_queries::insert_account_answers(
                    session_id,
                    user_id,
                    links.files_answered_with(ask.file_id, &ask.raw_account),
                    ask.raw_account.clone(),
                    account_id,
                )
            }
            (ImportAskDto::AssetMapping(ask), ImportAnswerDto::AssetMapping(choice)) => {
                let asset_id = match choice {
                    AssetMappingAnswerDto::Map { asset_id } => {
                        let visible: Count = self
                            .db
                            .fetch_one(asset_queries::assets_count_by_ids_and_access(
                                vec![asset_id],
                                user_id,
                            ))
                            .await?;
                        if visible.count != 1 {
                            return Err(invalid("asset_id", "Pick an existing asset.").into());
                        }
                        Some(asset_id)
                    }
                    AssetMappingAnswerDto::Exclude => {
                        if !ask.choices.contains(&ImportAskChoiceDto::Exclude) {
                            return Err(invalid(
                                "choice",
                                "This question can't be answered by leaving the asset out.",
                            )
                            .into());
                        }
                        None
                    }
                };
                import_queries::insert_asset_answer(session_id, user_id, ask.raw.clone(), asset_id)
            }
            _ => {
                return Err(
                    invalid("kind", "The answer is for a different kind of question.").into(),
                )
            }
        };
        let written = self
            .db
            .execute_with_rows_affected(query)
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
        if written == 0 {
            return Err(not_open().into());
        }
        Ok(())
    }
}

fn not_open() -> BusinessNotFoundError {
    BusinessNotFoundError {
        message: "Ask is not open or does not exist.".to_string(),
    }
}

fn invalid(field: &str, message: &str) -> BusinessValidationErrorDto {
    BusinessValidationErrorDto {
        errors: vec![BusinessFieldErrorDto {
            field: field.to_string(),
            message: message.to_string(),
        }],
    }
}
