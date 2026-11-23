use sea_query::extension::postgres::PgFunc;
use sea_query::{
    Alias, CommonTableExpression, Expr, ExprTrait, IntoIden, IntoValueTuple, OnConflict,
    PostgresQueryBuilder, Query, QueryStatementBuilder, SelectStatement, SimpleExpr, TableRef,
};
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;

use super::DbQueryWithValues;
use crate::{
    enums::transaction_types::DatabaseTransactionTypes,
    idens::account_idens::AccountIden,
    idens::asset_idens::AssetsIden,
    idens::connector_idens::{
        ConnectorBindingIden, ConnectorConnectionIden, ConnectorProviderAccountIden,
        ConnectorProviderIden, ConnectorRawPageIden, ConnectorTransactionIden,
        HeldTransactionsIden,
    },
    idens::entries_idens::EntryIden,
    idens::import_idens::ImportBankAccountsIden,
    idens::transaction_idens::{TransactionDescriptionsIden, TransactionIden},
    models::connector_models::{
        AddConnectorBindingModel, AddConnectorConnectionModel, AddConnectorRawPageModel,
        AddConnectorTransactionModel, AddHeldTransactionModel,
        UpdateProviderAccountSyncResultModel,
    },
    query_params::connector_params::{
        DeleteHeldTransactionsParams, DeleteHeldTransactionsParamsSearchType,
        GetConnectorBindingsParams, GetConnectorBindingsParamsSearchType,
        GetConnectorConnectionsParams, GetConnectorConnectionsParamsSearchType,
        GetConnectorTransactionsParams, GetHeldTransactionsParams,
        GetHeldTransactionsParamsSearchType, GetRawPagesParams, GetRawPagesParamsSearchType,
    },
};

#[macros::named_query]
pub fn get_connector_provider_id_by_kind(kind: String) -> DbQueryWithValues {
    Query::select()
        .column((ConnectorProviderIden::Table, ConnectorProviderIden::Id))
        .from(ConnectorProviderIden::Table)
        .and_where(Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Kind)).eq(kind))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

/// Bindings owned by `user_id`, resolved through `binding → provider_account → connection`.
fn owned_binding_ids_subquery(user_id: Uuid) -> SelectStatement {
    Query::select()
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
        .from(ConnectorBindingIden::Table)
        .inner_join(
            ConnectorProviderAccountIden::Table,
            Expr::col((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::ProviderAccountId,
            ))
            .equals((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            )),
        )
        .inner_join(
            ConnectorConnectionIden::Table,
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            ))
            .equals((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)),
        )
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::UserId,
            ))
            .eq(user_id),
        )
        .to_owned()
}

#[macros::named_query]
pub fn insert_connector_connection(model: AddConnectorConnectionModel) -> DbQueryWithValues {
    Query::insert()
        .into_table(ConnectorConnectionIden::Table)
        .columns(vec![
            ConnectorConnectionIden::UserId,
            ConnectorConnectionIden::ProviderId,
            ConnectorConnectionIden::CredentialMode,
            ConnectorConnectionIden::ProviderKeyId,
            ConnectorConnectionIden::Status,
            ConnectorConnectionIden::ConsentExpiresAt,
        ])
        .values_panic([
            model.user_id.into(),
            model.provider_id.into(),
            model.credential_mode.into(),
            model.provider_key_id.into(),
            model.status.into(),
            model.consent_expires_at.into(),
        ])
        .returning_col(ConnectorConnectionIden::Id)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_connector_connection_ids_by_user(user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .column((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id))
        .from(ConnectorConnectionIden::Table)
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::UserId,
            ))
            .eq(user_id),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_connector_connections(params: GetConnectorConnectionsParams) -> DbQueryWithValues {
    let mut query = Query::select()
        .column((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::UserId,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::ProviderId,
        ))
        .expr_as(
            Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Kind)),
            Alias::new("provider_kind"),
        )
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::CredentialMode,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::ProviderKeyId,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::Status,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::ConsentExpiresAt,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::CreatedAt,
        ))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::UpdatedAt,
        ))
        .from(ConnectorConnectionIden::Table)
        .inner_join(
            ConnectorProviderIden::Table,
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::ProviderId,
            ))
            .equals((ConnectorProviderIden::Table, ConnectorProviderIden::Id)),
        )
        .to_owned();

    query.and_where(
        Expr::col((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::UserId,
        ))
        .eq(params.user_id),
    );

    match params.search_type {
        GetConnectorConnectionsParamsSearchType::ById(id) => {
            query.and_where(
                Expr::col((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)).eq(id),
            );
        }
        GetConnectorConnectionsParamsSearchType::All => {}
    };

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn update_connector_connection_status(
    user_id: Uuid,
    connection_id: Uuid,
    status: String,
) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorConnectionIden::Table)
        .value(ConnectorConnectionIden::Status, status)
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::UserId,
            ))
            .eq(user_id),
        )
        .and_where(
            Expr::col((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id))
                .eq(connection_id),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn activate_connector_connection(
    user_id: Uuid,
    connection_id: Uuid,
    consent_expires_at: Option<time::OffsetDateTime>,
) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorConnectionIden::Table)
        .value(ConnectorConnectionIden::Status, "active")
        .value(
            ConnectorConnectionIden::ConsentExpiresAt,
            consent_expires_at,
        )
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::UserId,
            ))
            .eq(user_id),
        )
        .and_where(
            Expr::col((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id))
                .eq(connection_id),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_or_create_provider_accounts(
    connection_id: Uuid,
    external_account_ids: Vec<String>,
) -> DbQueryWithValues {
    let mut query = Query::insert();
    query
        .into_table(ConnectorProviderAccountIden::Table)
        .columns(vec![
            ConnectorProviderAccountIden::ConnectionId,
            ConnectorProviderAccountIden::ExternalAccountId,
        ])
        .on_conflict(
            OnConflict::columns([
                ConnectorProviderAccountIden::ConnectionId,
                ConnectorProviderAccountIden::ExternalAccountId,
            ])
            .value(ConnectorProviderAccountIden::UpdatedAt, Expr::cust("now()"))
            .to_owned(),
        )
        .returning(Query::returning().columns([
            ConnectorProviderAccountIden::Id,
            ConnectorProviderAccountIden::ExternalAccountId,
        ]));
    for external_account_id in external_account_ids {
        query.values_panic([connection_id.into(), external_account_id.into()]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_provider_account_sync_states(user_id: Uuid, connection_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            (
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ExternalAccountId,
            ),
            (
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::SyncedThrough,
            ),
            (
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::LastSyncStatus,
            ),
        ])
        .from(ConnectorProviderAccountIden::Table)
        .inner_join(
            ConnectorConnectionIden::Table,
            Expr::col((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)).equals((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            )),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            ))
            .eq(connection_id),
        )
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::UserId,
            ))
            .eq(user_id),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_connector_bindings(
    user_id: Uuid,
    bindings: Vec<AddConnectorBindingModel>,
) -> DbQueryWithValues {
    let binding = Alias::new("binding");
    let owned_accounts = Query::select()
        .column((binding.clone(), Alias::new("column1")))
        .column((AccountIden::Table, AccountIden::Id))
        .column((binding.clone(), Alias::new("column3")))
        .column((binding.clone(), Alias::new("column4")))
        .column((binding.clone(), Alias::new("column5")))
        .column((binding.clone(), Alias::new("column6")))
        .from_values(
            bindings.into_iter().map(|binding| {
                (
                    binding.provider_account_ref,
                    binding.sverto_account_id,
                    binding.write_mode,
                    binding.status,
                    binding.history_from,
                    binding.history_from_last_entry,
                )
            }),
            binding.clone(),
        )
        .inner_join(
            AccountIden::Table,
            Expr::col((AccountIden::Table, AccountIden::Id))
                .equals((binding, Alias::new("column2"))),
        )
        .and_where(Expr::col((AccountIden::Table, AccountIden::UserId)).eq(user_id))
        .and_where(Expr::col((AccountIden::Table, AccountIden::Active)).eq(true))
        .to_owned();
    Query::insert()
        .into_table(ConnectorBindingIden::Table)
        .columns(vec![
            ConnectorBindingIden::ProviderAccountId,
            ConnectorBindingIden::SvertoAccountId,
            ConnectorBindingIden::WriteMode,
            ConnectorBindingIden::Status,
            ConnectorBindingIden::HistoryFrom,
            ConnectorBindingIden::HistoryFromLastEntry,
        ])
        .select_from(owned_accounts)
        .expect("the select yields one value per inserted column")
        .returning(Query::returning().columns([
            ConnectorBindingIden::Id,
            ConnectorBindingIden::ProviderAccountId,
        ]))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn activate_pending_bindings(
    user_id: Uuid,
    cutoffs: Vec<(Uuid, Option<Uuid>)>,
) -> DbQueryWithValues {
    let cutoff = Alias::new("cutoff");
    Query::update()
        .table(ConnectorBindingIden::Table)
        .value(ConnectorBindingIden::Status, "active")
        .value(
            ConnectorBindingIden::ProjectedPageId,
            Expr::col((cutoff.clone(), Alias::new("column2"))),
        )
        .from(TableRef::ValuesList(
            cutoffs
                .into_iter()
                .map(IntoValueTuple::into_value_tuple)
                .collect(),
            cutoff.clone().into_iden(),
        ))
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .equals((cutoff, Alias::new("column1"))),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .in_subquery(owned_binding_ids_subquery(user_id)),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Status)).eq("pending"),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_pending_bindings(user_id: Uuid, binding_ids: Vec<Uuid>) -> DbQueryWithValues {
    Query::delete()
        .from_table(ConnectorBindingIden::Table)
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .eq(PgFunc::any(Expr::val(binding_ids))),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .in_subquery(owned_binding_ids_subquery(user_id)),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Status)).eq("pending"),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_connector_bindings(params: GetConnectorBindingsParams) -> DbQueryWithValues {
    let mut query = Query::select()
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
        .expr_as(
            Expr::col((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::ProviderAccountId,
            )),
            Alias::new("provider_account_ref"),
        )
        .column((
            ConnectorProviderAccountIden::Table,
            ConnectorProviderAccountIden::ConnectionId,
        ))
        .column((
            ConnectorBindingIden::Table,
            ConnectorBindingIden::SvertoAccountId,
        ))
        .expr_as(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ExternalAccountId,
            )),
            Alias::new("provider_account_id"),
        )
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::WriteMode))
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::Status))
        .column((
            ConnectorProviderAccountIden::Table,
            ConnectorProviderAccountIden::SyncedThrough,
        ))
        .column((
            ConnectorBindingIden::Table,
            ConnectorBindingIden::ProjectedPageId,
        ))
        .column((
            ConnectorBindingIden::Table,
            ConnectorBindingIden::HistoryFrom,
        ))
        .column((
            ConnectorBindingIden::Table,
            ConnectorBindingIden::HistoryFromLastEntry,
        ))
        .column((
            ConnectorProviderAccountIden::Table,
            ConnectorProviderAccountIden::LastSyncAt,
        ))
        .column((
            ConnectorProviderAccountIden::Table,
            ConnectorProviderAccountIden::LastSyncStatus,
        ))
        .column((
            ConnectorProviderAccountIden::Table,
            ConnectorProviderAccountIden::LastSyncError,
        ))
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::CreatedAt))
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::UpdatedAt))
        .expr_as(
            Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Kind)),
            Alias::new("provider_kind"),
        )
        .expr_as(
            SimpleExpr::SubQuery(
                None,
                Box::new(
                    Query::select()
                        .column((
                            ImportBankAccountsIden::Table,
                            ImportBankAccountsIden::SessionId,
                        ))
                        .from(ImportBankAccountsIden::Table)
                        .and_where(
                            Expr::col((
                                ImportBankAccountsIden::Table,
                                ImportBankAccountsIden::BindingId,
                            ))
                            .equals((ConnectorBindingIden::Table, ConnectorBindingIden::Id)),
                        )
                        .to_owned()
                        .into_sub_query_statement(),
                ),
            ),
            Alias::new("import_session_id"),
        )
        .from(ConnectorBindingIden::Table)
        .inner_join(
            ConnectorProviderAccountIden::Table,
            Expr::col((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::ProviderAccountId,
            ))
            .equals((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            )),
        )
        .inner_join(
            ConnectorConnectionIden::Table,
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            ))
            .equals((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)),
        )
        .inner_join(
            ConnectorProviderIden::Table,
            Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Id)).equals((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::ProviderId,
            )),
        )
        .to_owned();

    query.and_where(
        Expr::col((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::UserId,
        ))
        .eq(params.user_id),
    );

    match params.search_type {
        GetConnectorBindingsParamsSearchType::ById(id) => {
            query.and_where(
                Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id)).eq(id),
            );
        }
        GetConnectorBindingsParamsSearchType::ByIds(ids) => {
            query.and_where(
                Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                    .eq(PgFunc::any(Expr::val(ids))),
            );
        }
        GetConnectorBindingsParamsSearchType::ByConnection(connection_id) => {
            query.and_where(
                Expr::col((
                    ConnectorProviderAccountIden::Table,
                    ConnectorProviderAccountIden::ConnectionId,
                ))
                .eq(connection_id),
            );
        }
        GetConnectorBindingsParamsSearchType::All => {}
    };

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_active_stored_bindings(limit: i64) -> DbQueryWithValues {
    Query::select()
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
        .column((
            ConnectorConnectionIden::Table,
            ConnectorConnectionIden::UserId,
        ))
        .from(ConnectorBindingIden::Table)
        .inner_join(
            ConnectorProviderAccountIden::Table,
            Expr::col((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::ProviderAccountId,
            ))
            .equals((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            )),
        )
        .inner_join(
            ConnectorConnectionIden::Table,
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            ))
            .equals((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)),
        )
        .and_where(
            Expr::col((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::CredentialMode,
            ))
            .eq("stored"),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Status)).eq("active"),
        )
        .order_by_with_nulls(
            (
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::LastSyncAt,
            ),
            sea_query::Order::Asc,
            sea_query::NullOrdering::First,
        )
        .limit(limit as u64)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn update_connector_binding(
    user_id: Uuid,
    binding_id: Uuid,
    write_mode: Option<String>,
    status: String,
) -> DbQueryWithValues {
    let mut query = Query::update();
    query
        .table(ConnectorBindingIden::Table)
        .value(ConnectorBindingIden::Status, status)
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .in_subquery(owned_binding_ids_subquery(user_id)),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id)).eq(binding_id),
        )
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Status)).ne("pending"),
        );

    if let Some(write_mode) = write_mode {
        query.value(ConnectorBindingIden::WriteMode, write_mode);
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

/// Records a fetch outcome on the provider account (the fetch unit) and releases its claim.
#[macros::named_query]
pub fn update_provider_account_sync_result(
    model: UpdateProviderAccountSyncResultModel,
) -> DbQueryWithValues {
    let mut query = Query::update();
    query
        .table(ConnectorProviderAccountIden::Table)
        .value(
            ConnectorProviderAccountIden::LastSyncStatus,
            model.last_sync_status,
        )
        .value(
            ConnectorProviderAccountIden::LastSyncError,
            model.last_sync_error,
        )
        .value(ConnectorProviderAccountIden::LastSyncAt, model.last_sync_at)
        .value(
            ConnectorProviderAccountIden::SyncClaimedAt,
            sea_query::Value::from(None::<time::OffsetDateTime>),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            ))
            .eq(model.provider_account_ref),
        );

    if let Some(synced_through) = model.synced_through {
        query.value(ConnectorProviderAccountIden::SyncedThrough, synced_through);
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

/// Advances a binding's projection checkpoint after a committed projection.
#[macros::named_query]
pub fn update_binding_projection(
    binding_id: Uuid,
    projected_page_id: Option<Uuid>,
) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorBindingIden::Table)
        .value(ConnectorBindingIden::ProjectedPageId, projected_page_id)
        .and_where(
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id)).eq(binding_id),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

/// Claims a provider account for a fetch walk (single-flight). Zero rows affected means
/// another fetch holds the claim; stale claims expire after `expiry_minutes`.
#[macros::named_query]
pub fn claim_provider_account_for_fetch(
    provider_account_ref: Uuid,
    expiry_minutes: i64,
) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorProviderAccountIden::Table)
        .value(
            ConnectorProviderAccountIden::SyncClaimedAt,
            Expr::cust("now()"),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            ))
            .eq(provider_account_ref),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::SyncClaimedAt,
            ))
            .is_null()
            .or(Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::SyncClaimedAt,
            ))
            .lt(Expr::cust_with_values(
                "now() - make_interval(mins => $1::int)",
                [expiry_minutes],
            ))),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn release_provider_account_fetch_claim(provider_account_ref: Uuid) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorProviderAccountIden::Table)
        .value(
            ConnectorProviderAccountIden::SyncClaimedAt,
            sea_query::Value::from(None::<time::OffsetDateTime>),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            ))
            .eq(provider_account_ref),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_connector_transactions(params: GetConnectorTransactionsParams) -> DbQueryWithValues {
    let mut query = Query::select()
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::BindingId,
        ))
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::TransactionId,
        ))
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::ExternalId,
        ))
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::ExternalHash,
        ))
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::EditedByUser,
        ))
        .from(ConnectorTransactionIden::Table)
        .to_owned();
    match params {
        GetConnectorTransactionsParams::ByExternalIds {
            binding_id,
            external_ids,
        } => {
            query
                .and_where(
                    Expr::col((
                        ConnectorTransactionIden::Table,
                        ConnectorTransactionIden::BindingId,
                    ))
                    .eq(binding_id),
                )
                .and_where(
                    Expr::col((
                        ConnectorTransactionIden::Table,
                        ConnectorTransactionIden::ExternalId,
                    ))
                    .is_in(external_ids),
                );
        }
        GetConnectorTransactionsParams::ByTransactionIds {
            user_id,
            transaction_ids,
        } => {
            query
                .inner_join(
                    TransactionIden::Table,
                    Expr::col((TransactionIden::Table, TransactionIden::Id)).equals((
                        ConnectorTransactionIden::Table,
                        ConnectorTransactionIden::TransactionId,
                    )),
                )
                .and_where(Expr::col((TransactionIden::Table, TransactionIden::UserId)).eq(user_id))
                .and_where(
                    Expr::col((
                        ConnectorTransactionIden::Table,
                        ConnectorTransactionIden::TransactionId,
                    ))
                    .eq(PgFunc::any(Expr::val(transaction_ids))),
                );
        }
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn mark_connector_transactions_edited(transaction_ids: Vec<Uuid>) -> DbQueryWithValues {
    Query::update()
        .table(ConnectorTransactionIden::Table)
        .value(ConnectorTransactionIden::EditedByUser, true)
        .and_where(
            Expr::col((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::TransactionId,
            ))
            .is_in(transaction_ids),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_connector_transactions(models: &[AddConnectorTransactionModel]) -> DbQueryWithValues {
    let mut query = Query::insert()
        .into_table(ConnectorTransactionIden::Table)
        .columns(vec![
            ConnectorTransactionIden::BindingId,
            ConnectorTransactionIden::TransactionId,
            ConnectorTransactionIden::ExternalId,
            ConnectorTransactionIden::ExternalHash,
        ])
        .to_owned();
    for model in models {
        query.values_panic([
            model.binding_id.into(),
            model.transaction_id.into(),
            model.external_id.clone().into(),
            model.external_hash.clone().into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn insert_raw_page(model: AddConnectorRawPageModel) -> DbQueryWithValues {
    let refresh_claim = Query::update()
        .table(ConnectorProviderAccountIden::Table)
        .value(
            ConnectorProviderAccountIden::SyncClaimedAt,
            Expr::cust("now()"),
        )
        .and_where(
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            ))
            .eq(model.provider_account_ref),
        )
        .to_owned();
    Query::insert()
        .with_cte(
            CommonTableExpression::new()
                .query(refresh_claim)
                .table_name(Alias::new("refreshed_claim"))
                .to_owned(),
        )
        .into_table(ConnectorRawPageIden::Table)
        .columns(vec![
            ConnectorRawPageIden::ProviderAccountId,
            ConnectorRawPageIden::Stream,
            ConnectorRawPageIden::Payload,
            ConnectorRawPageIden::CursorAfter,
            ConnectorRawPageIden::PayloadHash,
        ])
        .values_panic([
            model.provider_account_ref.into(),
            model.stream.into(),
            model.payload.into(),
            model.cursor_after.into(),
            model.payload_hash.into(),
        ])
        .returning_col(ConnectorRawPageIden::Id)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_latest_raw_page_cursor(provider_account_ref: Uuid) -> DbQueryWithValues {
    Query::select()
        .column((
            ConnectorRawPageIden::Table,
            ConnectorRawPageIden::CursorAfter,
        ))
        .from(ConnectorRawPageIden::Table)
        .and_where(
            Expr::col((
                ConnectorRawPageIden::Table,
                ConnectorRawPageIden::ProviderAccountId,
            ))
            .eq(provider_account_ref),
        )
        .order_by(
            (ConnectorRawPageIden::Table, ConnectorRawPageIden::FetchedAt),
            sea_query::Order::Desc,
        )
        .order_by(
            (ConnectorRawPageIden::Table, ConnectorRawPageIden::Id),
            sea_query::Order::Desc,
        )
        .limit(1)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_connector_raw_pages(params: GetRawPagesParams) -> DbQueryWithValues {
    let mut query = Query::select();
    query
        .column((ConnectorRawPageIden::Table, ConnectorRawPageIden::Id))
        .column((
            ConnectorRawPageIden::Table,
            ConnectorRawPageIden::ProviderAccountId,
        ))
        .column((ConnectorRawPageIden::Table, ConnectorRawPageIden::Stream))
        .column((ConnectorRawPageIden::Table, ConnectorRawPageIden::Payload))
        .from(ConnectorRawPageIden::Table)
        .order_by(
            (ConnectorRawPageIden::Table, ConnectorRawPageIden::Id),
            sea_query::Order::Asc,
        );

    match params.search_type {
        GetRawPagesParamsSearchType::ByProviderAccountRef(provider_account_ref) => {
            query.and_where(
                Expr::col((
                    ConnectorRawPageIden::Table,
                    ConnectorRawPageIden::ProviderAccountId,
                ))
                .eq(provider_account_ref),
            );
        }
        GetRawPagesParamsSearchType::ByProviderAccountRefs(provider_account_refs) => {
            query.and_where(
                Expr::col((
                    ConnectorRawPageIden::Table,
                    ConnectorRawPageIden::ProviderAccountId,
                ))
                .eq(PgFunc::any(Expr::val(provider_account_refs))),
            );
        }
        GetRawPagesParamsSearchType::ByExternalAccount {
            connection_id,
            external_account_id,
        } => {
            query
                .inner_join(
                    ConnectorProviderAccountIden::Table,
                    Expr::col((
                        ConnectorRawPageIden::Table,
                        ConnectorRawPageIden::ProviderAccountId,
                    ))
                    .equals((
                        ConnectorProviderAccountIden::Table,
                        ConnectorProviderAccountIden::Id,
                    )),
                )
                .and_where(
                    Expr::col((
                        ConnectorProviderAccountIden::Table,
                        ConnectorProviderAccountIden::ConnectionId,
                    ))
                    .eq(connection_id),
                )
                .and_where(
                    Expr::col((
                        ConnectorProviderAccountIden::Table,
                        ConnectorProviderAccountIden::ExternalAccountId,
                    ))
                    .eq(external_account_id),
                );
        }
    }

    if let Some(after) = params.after_page_id {
        query.and_where(
            Expr::col((ConnectorRawPageIden::Table, ConnectorRawPageIden::Id)).gt(after),
        );
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_held_transactions(params: GetHeldTransactionsParams) -> DbQueryWithValues {
    let mut query = Query::select()
        .column((HeldTransactionsIden::Table, HeldTransactionsIden::Id))
        .column((HeldTransactionsIden::Table, HeldTransactionsIden::BindingId))
        .expr_as(
            Expr::col((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::SvertoAccountId,
            )),
            Alias::new("account_id"),
        )
        .column((ConnectorBindingIden::Table, ConnectorBindingIden::WriteMode))
        .expr_as(
            Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Kind)),
            Alias::new("provider_kind"),
        )
        .column((
            HeldTransactionsIden::Table,
            HeldTransactionsIden::ExternalId,
        ))
        .column((
            HeldTransactionsIden::Table,
            HeldTransactionsIden::ExternalHash,
        ))
        .column((HeldTransactionsIden::Table, HeldTransactionsIden::AssetId))
        .column((HeldTransactionsIden::Table, HeldTransactionsIden::Quantity))
        .column((
            HeldTransactionsIden::Table,
            HeldTransactionsIden::DateTransacted,
        ))
        .column((
            HeldTransactionsIden::Table,
            HeldTransactionsIden::Description,
        ))
        .column((
            HeldTransactionsIden::Table,
            HeldTransactionsIden::CandidateIds,
        ))
        .from(HeldTransactionsIden::Table)
        .inner_join(
            ConnectorBindingIden::Table,
            Expr::col((ConnectorBindingIden::Table, ConnectorBindingIden::Id))
                .equals((HeldTransactionsIden::Table, HeldTransactionsIden::BindingId)),
        )
        .inner_join(
            ConnectorProviderAccountIden::Table,
            Expr::col((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::Id,
            ))
            .equals((
                ConnectorBindingIden::Table,
                ConnectorBindingIden::ProviderAccountId,
            )),
        )
        .inner_join(
            ConnectorConnectionIden::Table,
            Expr::col((ConnectorConnectionIden::Table, ConnectorConnectionIden::Id)).equals((
                ConnectorProviderAccountIden::Table,
                ConnectorProviderAccountIden::ConnectionId,
            )),
        )
        .inner_join(
            ConnectorProviderIden::Table,
            Expr::col((ConnectorProviderIden::Table, ConnectorProviderIden::Id)).equals((
                ConnectorConnectionIden::Table,
                ConnectorConnectionIden::ProviderId,
            )),
        )
        .and_where(
            Expr::col((HeldTransactionsIden::Table, HeldTransactionsIden::UserId))
                .eq(params.user_id),
        )
        .order_by(
            (
                HeldTransactionsIden::Table,
                HeldTransactionsIden::DateTransacted,
            ),
            sea_query::Order::Desc,
        )
        .order_by(
            (HeldTransactionsIden::Table, HeldTransactionsIden::Id),
            sea_query::Order::Asc,
        )
        .to_owned();

    match params.search_type {
        GetHeldTransactionsParamsSearchType::All => {}
        GetHeldTransactionsParamsSearchType::ById(id) => {
            query.and_where(
                Expr::col((HeldTransactionsIden::Table, HeldTransactionsIden::Id)).eq(id),
            );
        }
        GetHeldTransactionsParamsSearchType::ByExternalIds {
            binding_id,
            external_ids,
        } => {
            query
                .and_where(
                    Expr::col((HeldTransactionsIden::Table, HeldTransactionsIden::BindingId))
                        .eq(binding_id),
                )
                .and_where(
                    Expr::col((
                        HeldTransactionsIden::Table,
                        HeldTransactionsIden::ExternalId,
                    ))
                    .eq(PgFunc::any(Expr::val(external_ids))),
                );
        }
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn insert_held_transactions(models: &[AddHeldTransactionModel]) -> DbQueryWithValues {
    let held = Alias::new("held");
    let column = |index: usize| (held.clone(), Alias::new(format!("column{index}")));
    let recorded = Query::select()
        .expr(Expr::val(1))
        .from(ConnectorTransactionIden::Table)
        .and_where(
            Expr::col((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::BindingId,
            ))
            .equals(column(2)),
        )
        .and_where(
            Expr::col((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::ExternalId,
            ))
            .equals(column(3)),
        )
        .to_owned();
    let new_rows = Query::select()
        .columns((1..=9).map(column))
        .from_values(
            models.iter().map(|model| {
                (
                    model.user_id,
                    model.binding_id,
                    model.external_id.clone(),
                    model.external_hash.clone(),
                    model.asset_id,
                    model.quantity,
                    model.date_transacted,
                    model.description.clone(),
                    model.candidate_ids.clone(),
                )
            }),
            held.clone(),
        )
        .and_where(Expr::exists(recorded).not())
        .to_owned();
    Query::insert()
        .into_table(HeldTransactionsIden::Table)
        .columns([
            HeldTransactionsIden::UserId,
            HeldTransactionsIden::BindingId,
            HeldTransactionsIden::ExternalId,
            HeldTransactionsIden::ExternalHash,
            HeldTransactionsIden::AssetId,
            HeldTransactionsIden::Quantity,
            HeldTransactionsIden::DateTransacted,
            HeldTransactionsIden::Description,
            HeldTransactionsIden::CandidateIds,
        ])
        .select_from(new_rows)
        .expect("the select yields one value per inserted column")
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_held_transactions(params: DeleteHeldTransactionsParams) -> DbQueryWithValues {
    let mut query = Query::delete();
    query
        .from_table(HeldTransactionsIden::Table)
        .and_where(Expr::col(HeldTransactionsIden::UserId).eq(params.user_id));
    match params.search_type {
        DeleteHeldTransactionsParamsSearchType::ById(id) => {
            query.and_where(Expr::col(HeldTransactionsIden::Id).eq(id));
        }
        DeleteHeldTransactionsParamsSearchType::ByBinding(binding_id) => {
            query.and_where(Expr::col(HeldTransactionsIden::BindingId).eq(binding_id));
        }
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

fn unedited_synced_regular_transactions(user_id: Uuid) -> SelectStatement {
    Query::select()
        .column((
            ConnectorTransactionIden::Table,
            ConnectorTransactionIden::TransactionId,
        ))
        .from(ConnectorTransactionIden::Table)
        .inner_join(
            TransactionIden::Table,
            Expr::col((TransactionIden::Table, TransactionIden::Id)).equals((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::TransactionId,
            )),
        )
        .and_where(Expr::col((TransactionIden::Table, TransactionIden::UserId)).eq(user_id))
        .and_where(
            Expr::col((TransactionIden::Table, TransactionIden::TypeId))
                .eq(DatabaseTransactionTypes::RegularTransaction as i32),
        )
        .and_where(
            Expr::col((
                ConnectorTransactionIden::Table,
                ConnectorTransactionIden::EditedByUser,
            ))
            .eq(false),
        )
        .to_owned()
}

#[macros::named_query]
pub fn get_synced_transactions_to_categorize(
    user_id: Uuid,
    transaction_ids: Vec<Uuid>,
    category_id: i32,
) -> DbQueryWithValues {
    Query::select()
        .expr_as(
            Expr::col((EntryIden::Table, EntryIden::TransactionId)),
            Alias::new("transaction_id"),
        )
        .column((
            TransactionDescriptionsIden::Table,
            TransactionDescriptionsIden::Description,
        ))
        .column((EntryIden::Table, EntryIden::Quantity))
        .column((AssetsIden::Table, AssetsIden::Ticker))
        .column((AccountIden::Table, AccountIden::AccountName))
        .from(EntryIden::Table)
        .inner_join(
            AssetsIden::Table,
            Expr::col((AssetsIden::Table, AssetsIden::Id))
                .equals((EntryIden::Table, EntryIden::AssetId)),
        )
        .inner_join(
            AccountIden::Table,
            Expr::col((AccountIden::Table, AccountIden::Id))
                .equals((EntryIden::Table, EntryIden::AccountId)),
        )
        .left_join(
            TransactionDescriptionsIden::Table,
            Expr::col((
                TransactionDescriptionsIden::Table,
                TransactionDescriptionsIden::TransactionId,
            ))
            .equals((EntryIden::Table, EntryIden::TransactionId)),
        )
        .and_where(
            Expr::col((EntryIden::Table, EntryIden::TransactionId))
                .in_subquery(unedited_synced_regular_transactions(user_id)),
        )
        .and_where(
            Expr::col((EntryIden::Table, EntryIden::TransactionId))
                .eq(PgFunc::any(Expr::val(transaction_ids))),
        )
        .and_where(Expr::col((EntryIden::Table, EntryIden::CategoryId)).eq(category_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}
