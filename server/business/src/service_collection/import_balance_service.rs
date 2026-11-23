#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::import_models::ImportAssertionModel;
use dal::queries::import_queries;
use rust_decimal::Decimal;
use time::Duration;
use uuid::Uuid;

use super::ledger_matching_service::LedgerMatchingService;
use crate::dtos::imports::import_balance_dto::{
    ImportAssertionDto, ImportEnterableBalanceDto, ImportEnteredBalanceDto,
};
use crate::dtos::imports::import_staging_dto::ImportProposalRowDto;
use crate::dtos::ledger_matching_dto::{LedgerActivityDto, LedgerTotalDto};
use crate::entities::imports::balances::SessionBalances;
use crate::entities::imports::linking::SessionLinks;
use crate::entities::ledger_matching::WINDOW_DAYS;

pub struct ImportBalanceService {
    db: MyraDb,
    ledger_matching_service: LedgerMatchingService,
}

impl ImportBalanceService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
            ledger_matching_service: LedgerMatchingService::new(providers),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn assertions(
        &self,
        session_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<(Vec<ImportAssertionDto>, Vec<ImportEnteredBalanceDto>)> {
        let (printed, entered): (Vec<ImportAssertionModel>, Vec<ImportAssertionModel>) = self
            .db
            .fetch_all::<ImportAssertionModel>(import_queries::get_assertions(session_id, user_id))
            .await?
            .into_iter()
            .partition(|assertion| assertion.source_id.is_some());
        Ok((
            printed
                .into_iter()
                .map(ImportAssertionDto::try_from)
                .collect::<anyhow::Result<_>>()?,
            entered
                .into_iter()
                .map(ImportEnteredBalanceDto::try_from)
                .collect::<anyhow::Result<_>>()?,
        ))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn read(
        &self,
        user_id: Uuid,
        printed: Vec<ImportAssertionDto>,
        entered: Vec<ImportEnteredBalanceDto>,
        rows: &[ImportProposalRowDto],
        links: &SessionLinks,
    ) -> anyhow::Result<(SessionBalances, Vec<LedgerTotalDto>, LedgerActivityDto)> {
        let mut balances = SessionBalances::new(printed);
        balances.add_entered_balances(entered);
        balances.add_account_mappings(&links.account_mappings);
        balances.add_asset_mappings(&links.asset_mappings);
        balances.add_rows(rows);

        let Some((earliest, latest)) = balances.date_range() else {
            return Ok((balances, Vec::new(), LedgerActivityDto::default()));
        };
        let from = earliest - Duration::days(WINDOW_DAYS);
        let to = latest + Duration::days(WINDOW_DAYS);
        let totals = self
            .ledger_matching_service
            .totals_before(user_id, from)
            .await?;
        let activity = self
            .ledger_matching_service
            .activity(user_id, from, to)
            .await?;
        Ok((balances, totals, activity))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(session_id = %session_id))]
    pub async fn enter_balance(
        &self,
        session_id: Uuid,
        user_id: Uuid,
        enterable: &ImportEnterableBalanceDto,
        quantity: Decimal,
    ) -> anyhow::Result<u64> {
        Ok(self
            .db
            .execute_with_rows_affected(import_queries::upsert_entered_balance(
                session_id,
                user_id,
                enterable.account_id,
                enterable.asset_id,
                enterable.as_of_date,
                quantity,
            ))
            .await?)
    }
}
