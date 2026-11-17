#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::enums::transaction_types::DatabaseTransactionTypes;
use dal::models::ledger_matching_models::{
    LedgerMovementModel, LedgerTotalModel, RecordedCountModel, TransactionSourceInsert,
    TransactionSourceModel,
};
use dal::queries::ledger_matching_queries;
use std::collections::HashMap;

use time::{Date, Duration};
use uuid::Uuid;

use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::ledger_matching_dto::{
    LedgerActivityDto, LedgerMatchBatchDto, LedgerTotalDto, MatchOutcomeDto, TransactionSourceDto,
};
use crate::entities::ledger_matching::{LedgerMatching, WINDOW_DAYS};

const INSERT_CHUNK: usize = 1000;

pub struct LedgerMatchingService {
    db: MyraDb,
}

impl LedgerMatchingService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn match_batches(
        &self,
        user_id: Uuid,
        batches: &[LedgerMatchBatchDto],
    ) -> anyhow::Result<Vec<Vec<MatchOutcomeDto>>> {
        let dates: Vec<Date> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .incoming
                    .iter()
                    .chain(batch.settled.iter().map(|settled| &settled.movement))
            })
            .map(|movement| movement.date)
            .collect();
        let (Some(first), Some(last)) = (dates.iter().min(), dates.iter().max()) else {
            return Ok(batches.iter().map(|_| Vec::new()).collect());
        };
        let from = *first - Duration::days(WINDOW_DAYS);
        let to = *last + Duration::days(WINDOW_DAYS);

        let (movements, sources) = self.ledger(user_id, from, to).await?;
        let types: HashMap<Uuid, DatabaseTransactionTypes> = movements
            .iter()
            .map(|movement| (movement.id, movement.type_id))
            .collect();

        let mut matching =
            LedgerMatching::new(movements.into_iter().map(Into::into).collect(), sources);
        matching.add_transaction_types(&types);
        Ok(matching.match_batches(batches))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn activity(
        &self,
        user_id: Uuid,
        from: Date,
        to: Date,
    ) -> anyhow::Result<LedgerActivityDto> {
        let (movements, sources) = self.ledger(user_id, from, to).await?;
        Ok(LedgerActivityDto {
            movements: movements.into_iter().map(Into::into).collect(),
            sources,
        })
    }

    async fn ledger(
        &self,
        user_id: Uuid,
        from: Date,
        to: Date,
    ) -> anyhow::Result<(Vec<LedgerMovementModel>, Vec<TransactionSourceDto>)> {
        let movements = self
            .db
            .fetch_all::<LedgerMovementModel>(ledger_matching_queries::get_ledger_movements(
                user_id,
                from.midnight().assume_utc(),
                (to + Duration::days(1)).midnight().assume_utc(),
            ))
            .await?;
        let sources = self
            .db
            .fetch_all::<TransactionSourceModel>(ledger_matching_queries::get_transaction_sources(
                user_id, from, to,
            ))
            .await?
            .into_iter()
            .map(TransactionSourceDto::try_from)
            .collect::<anyhow::Result<_>>()?;
        Ok((movements, sources))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn totals_before(
        &self,
        user_id: Uuid,
        before: Date,
    ) -> anyhow::Result<Vec<LedgerTotalDto>> {
        Ok(self
            .db
            .fetch_all::<LedgerTotalModel>(ledger_matching_queries::get_ledger_totals_before(
                user_id,
                before.midnight().assume_utc(),
            ))
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, count = sources.len()))]
    pub async fn record_sources(
        &self,
        user_id: Uuid,
        sources: Vec<TransactionSourceDto>,
    ) -> anyhow::Result<()> {
        let rows: Vec<TransactionSourceInsert> = sources
            .into_iter()
            .map(|source| TransactionSourceInsert {
                user_id,
                transaction_id: source.transaction_id,
                account_id: source.account_id,
                asset_id: source.asset_id,
                occurred_on: source.date,
                description: source.description,
                kind: source.kind.into(),
                stream: source.stream,
                external_ref: source.external_ref,
                itemised_fee: source.itemised_fee,
            })
            .collect();
        for chunk in rows.chunks(INSERT_CHUNK) {
            self.db
                .execute(ledger_matching_queries::insert_transaction_sources(chunk))
                .await
                .map_err(|error| {
                    if error.as_database_error().is_some_and(|error| {
                        error.is_unique_violation() || error.is_foreign_key_violation()
                    }) {
                        anyhow::Error::new(BusinessConflictError {
                            message: "These transactions changed while they were being recorded. Review them again."
                                .to_string(),
                        })
                    } else {
                        anyhow::Error::new(error)
                    }
                })?;
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn recorded_counts(
        &self,
        user_id: Uuid,
        streams: Vec<String>,
    ) -> anyhow::Result<HashMap<String, i64>> {
        Ok(self
            .db
            .fetch_all::<RecordedCountModel>(ledger_matching_queries::get_recorded_counts(
                user_id, streams,
            ))
            .await?
            .into_iter()
            .map(|count| (count.stream, count.transactions))
            .collect())
    }
}
