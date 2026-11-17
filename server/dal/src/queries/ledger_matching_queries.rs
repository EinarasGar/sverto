use sea_query::extension::postgres::PgFunc;
use sea_query::*;
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;
use time::{Date, OffsetDateTime};

use crate::idens::connector_idens::ConnectorTransactionIden;
use crate::idens::entries_idens::EntryIden;
use crate::idens::transaction_idens::{
    TransactionDescriptionsIden, TransactionIden, TransactionSourcesIden,
};
use crate::idens::ArrayFunc;
use crate::models::ledger_matching_models::TransactionSourceInsert;

use super::DbQueryWithValues;

#[macros::named_query]
pub fn get_ledger_movements(
    user_id: Uuid,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> DbQueryWithValues {
    let connector_bindings = Query::select()
        .column(ConnectorTransactionIden::BindingId)
        .from(ConnectorTransactionIden::Table)
        .and_where(
            Expr::col((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::TransactionId,
            ))
            .equals((TransactionIden::Table, TransactionIden::Id)),
        )
        .to_owned();
    Query::select()
        .column((TransactionIden::Table, TransactionIden::Id))
        .column((TransactionIden::Table, TransactionIden::GroupId))
        .column((TransactionIden::Table, TransactionIden::TypeId))
        .column((TransactionIden::Table, TransactionIden::DateTransacted))
        .column((EntryIden::Table, EntryIden::AccountId))
        .column((EntryIden::Table, EntryIden::AssetId))
        .expr_as(
            Expr::col((EntryIden::Table, EntryIden::Quantity)).sum(),
            Alias::new("quantity"),
        )
        .column((
            TransactionDescriptionsIden::Table,
            TransactionDescriptionsIden::Description,
        ))
        .expr_as(
            Func::cust(ArrayFunc).arg(SimpleExpr::SubQuery(
                None,
                Box::new(connector_bindings.into_sub_query_statement()),
            )),
            Alias::new("connector_binding_ids"),
        )
        .from(TransactionIden::Table)
        .inner_join(
            EntryIden::Table,
            Expr::col((EntryIden::Table, EntryIden::TransactionId))
                .equals((TransactionIden::Table, TransactionIden::Id)),
        )
        .left_join(
            TransactionDescriptionsIden::Table,
            Expr::col((
                TransactionDescriptionsIden::Table,
                TransactionDescriptionsIden::TransactionId,
            ))
            .equals((TransactionIden::Table, TransactionIden::Id)),
        )
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::UserId)).eq(user_id))
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::DateTransacted)).gte(from))
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::DateTransacted)).lt(to))
        .group_by_col((TransactionIden::Table, TransactionIden::Id))
        .group_by_col((EntryIden::Table, EntryIden::AccountId))
        .group_by_col((EntryIden::Table, EntryIden::AssetId))
        .group_by_col((
            TransactionDescriptionsIden::Table,
            TransactionDescriptionsIden::Description,
        ))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_ledger_totals_before(user_id: Uuid, before: OffsetDateTime) -> DbQueryWithValues {
    Query::select()
        .column((EntryIden::Table, EntryIden::AccountId))
        .column((EntryIden::Table, EntryIden::AssetId))
        .expr_as(
            Expr::col((EntryIden::Table, EntryIden::Quantity)).sum(),
            Alias::new("quantity"),
        )
        .from(EntryIden::Table)
        .inner_join(
            TransactionIden::Table,
            Expr::col((TransactionIden::Table, TransactionIden::Id))
                .equals((EntryIden::Table, EntryIden::TransactionId)),
        )
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::UserId)).eq(user_id))
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::DateTransacted)).lt(before))
        .group_by_col((EntryIden::Table, EntryIden::AccountId))
        .group_by_col((EntryIden::Table, EntryIden::AssetId))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_transaction_sources(user_id: Uuid, from: Date, to: Date) -> DbQueryWithValues {
    Query::select()
        .columns([
            TransactionSourcesIden::TransactionId,
            TransactionSourcesIden::AccountId,
            TransactionSourcesIden::AssetId,
            TransactionSourcesIden::OccurredOn,
            TransactionSourcesIden::Description,
            TransactionSourcesIden::Kind,
            TransactionSourcesIden::Stream,
            TransactionSourcesIden::ExternalRef,
            TransactionSourcesIden::ItemisedFee,
        ])
        .from(TransactionSourcesIden::Table)
        .and_where(Expr::col(TransactionSourcesIden::UserId).eq(user_id))
        .and_where(Expr::col(TransactionSourcesIden::OccurredOn).between(from, to))
        .order_by(TransactionSourcesIden::OccurredOn, Order::Asc)
        .order_by(TransactionSourcesIden::Stream, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_transaction_sources(rows: &[TransactionSourceInsert]) -> DbQueryWithValues {
    let mut query = Query::insert();
    query.into_table(TransactionSourcesIden::Table).columns([
        TransactionSourcesIden::UserId,
        TransactionSourcesIden::TransactionId,
        TransactionSourcesIden::AccountId,
        TransactionSourcesIden::AssetId,
        TransactionSourcesIden::OccurredOn,
        TransactionSourcesIden::Description,
        TransactionSourcesIden::Kind,
        TransactionSourcesIden::Stream,
        TransactionSourcesIden::ExternalRef,
        TransactionSourcesIden::ItemisedFee,
    ]);
    for row in rows {
        query.values_panic([
            row.user_id.into(),
            row.transaction_id.into(),
            row.account_id.into(),
            row.asset_id.into(),
            row.occurred_on.into(),
            row.description.clone().into(),
            row.kind.into(),
            row.stream.clone().into(),
            row.external_ref.clone().into(),
            row.itemised_fee.into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_recorded_counts(user_id: Uuid, streams: Vec<String>) -> DbQueryWithValues {
    Query::select()
        .column(TransactionSourcesIden::Stream)
        .expr_as(
            Func::count_distinct(Expr::col(TransactionSourcesIden::TransactionId)),
            Alias::new("transactions"),
        )
        .from(TransactionSourcesIden::Table)
        .and_where(Expr::col(TransactionSourcesIden::UserId).eq(user_id))
        .and_where(Expr::col(TransactionSourcesIden::Stream).eq(PgFunc::any(Expr::val(streams))))
        .group_by_col(TransactionSourcesIden::Stream)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}
