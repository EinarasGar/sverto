use rust_decimal::Decimal;
use sea_query::extension::postgres::PgFunc;
use sea_query::*;
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;
use time::{Date, OffsetDateTime};

use crate::enums::transaction_types::DatabaseTransactionTypes;
use crate::idens::import_idens::{
    ImportAccountMappingsIden, ImportAssertionsIden, ImportAssetMappingsIden,
    ImportBankAccountsIden, ImportClassificationConversationsIden, ImportClassificationRunsIden,
    ImportFilesIden, ImportLineClassificationsIden, ImportParsedLegsIden, ImportParsedLinesIden,
    ImportSessionsIden, ImportSourcesIden, ImportStagedDecisionsIden,
};
use crate::idens::{ArrayFunc, CommonsIden, Unnest};
use crate::models::import_models::{
    ImportAccountMappingModel, ImportAssertionInsert, ImportAssertionKind, ImportAssetMappingModel,
    ImportBankAccountInsert, ImportDateOrder, ImportFileStatus, ImportParsedLegInsert,
    ImportParsedLineInsert, ImportSessionLifecycle, ImportSourceKind, ImportVerb,
};
use crate::query_params::import_params::{
    ClassificationRunOutcome, DeleteImportSourcesParams, DeleteImportSourcesSearchType,
    GetImportFilesParams, GetImportFilesSearchType, GetImportSessionsParams,
    GetImportSessionsSearchType,
};

use super::DbQueryWithValues;

fn session_columns() -> [ImportSessionsIden; 4] {
    [
        ImportSessionsIden::Id,
        ImportSessionsIden::ConversationId,
        ImportSessionsIden::Lifecycle,
        ImportSessionsIden::CreatedAt,
    ]
}

fn subquery(select: SelectStatement) -> SimpleExpr {
    SimpleExpr::SubQuery(None, Box::new(select.into_sub_query_statement()))
}

fn session_id_matches<T: Iden + 'static>(table: T, session_id_column: T) -> SimpleExpr {
    Expr::col((table, session_id_column))
        .equals((ImportSessionsIden::Table, ImportSessionsIden::Id))
}

fn latest_classification_run(column: ImportClassificationRunsIden) -> SimpleExpr {
    subquery(
        Query::select()
            .column(column)
            .from(ImportClassificationRunsIden::Table)
            .and_where(session_id_matches(
                ImportClassificationRunsIden::Table,
                ImportClassificationRunsIden::SessionId,
            ))
            .order_by(ImportClassificationRunsIden::CreatedAt, Order::Desc)
            .order_by(ImportClassificationRunsIden::Id, Order::Desc)
            .limit(1)
            .to_owned(),
    )
}

fn file_count(status: ImportFileStatus) -> SimpleExpr {
    subquery(
        Query::select()
            .expr(Expr::cust("COUNT(*)"))
            .from(ImportFilesIden::Table)
            .and_where(session_id_matches(
                ImportFilesIden::Table,
                ImportFilesIden::SessionId,
            ))
            .and_where(Expr::col(ImportFilesIden::Status).eq(status))
            .to_owned(),
    )
}

#[macros::named_query]
pub fn create_session(user_id: Uuid, conversation_id: Uuid) -> DbQueryWithValues {
    Query::insert()
        .into_table(ImportSessionsIden::Table)
        .columns([
            ImportSessionsIden::UserId,
            ImportSessionsIden::ConversationId,
        ])
        .values_panic([user_id.into(), conversation_id.into()])
        .returning(Query::returning().columns(session_columns()))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_session_summaries(params: GetImportSessionsParams) -> DbQueryWithValues {
    let mut query = Query::select();
    query
        .columns(session_columns().map(|column| (ImportSessionsIden::Table, column)))
        .expr_as(
            file_count(ImportFileStatus::Pending),
            Alias::new("pending_files"),
        )
        .expr_as(
            file_count(ImportFileStatus::Processing),
            Alias::new("processing_files"),
        )
        .expr_as(
            file_count(ImportFileStatus::Failed),
            Alias::new("failed_files"),
        )
        .expr_as(
            file_count(ImportFileStatus::Parsed),
            Alias::new("parsed_files"),
        )
        .expr_as(
            subquery(
                Query::select()
                    .expr(Expr::cust("COUNT(*)"))
                    .from(ImportBankAccountsIden::Table)
                    .and_where(session_id_matches(
                        ImportBankAccountsIden::Table,
                        ImportBankAccountsIden::SessionId,
                    ))
                    .to_owned(),
            ),
            Alias::new("bank_accounts"),
        )
        .expr_as(
            latest_classification_run(ImportClassificationRunsIden::Id),
            Alias::new("classification_run_id"),
        )
        .expr_as(
            latest_classification_run(ImportClassificationRunsIden::ClassifiedAt),
            Alias::new("classified_at"),
        )
        .expr_as(
            latest_classification_run(ImportClassificationRunsIden::ErrorMessage),
            Alias::new("classification_error"),
        )
        .from(ImportSessionsIden::Table)
        .and_where(
            Expr::col((ImportSessionsIden::Table, ImportSessionsIden::UserId)).eq(params.user_id),
        );

    match params.search_type {
        GetImportSessionsSearchType::ById(session_id) => {
            query.and_where(
                Expr::col((ImportSessionsIden::Table, ImportSessionsIden::Id)).eq(session_id),
            );
        }
        GetImportSessionsSearchType::All => {
            query
                .order_by(
                    (ImportSessionsIden::Table, ImportSessionsIden::CreatedAt),
                    Order::Desc,
                )
                .order_by(
                    (ImportSessionsIden::Table, ImportSessionsIden::Id),
                    Order::Desc,
                );
        }
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_session(params: GetImportSessionsParams) -> DbQueryWithValues {
    let mut query = Query::select();
    query
        .columns(session_columns())
        .from(ImportSessionsIden::Table)
        .and_where(Expr::col(ImportSessionsIden::UserId).eq(params.user_id));
    match params.search_type {
        GetImportSessionsSearchType::ById(session_id) => {
            query.and_where(Expr::col(ImportSessionsIden::Id).eq(session_id));
        }
        GetImportSessionsSearchType::All => {
            query
                .order_by(ImportSessionsIden::CreatedAt, Order::Desc)
                .order_by(ImportSessionsIden::Id, Order::Desc);
        }
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

fn file_columns() -> [(ImportFilesIden, ImportFilesIden); 8] {
    [
        (ImportFilesIden::Table, ImportFilesIden::SourceId),
        (ImportFilesIden::Table, ImportFilesIden::FileId),
        (ImportFilesIden::Table, ImportFilesIden::ConversationId),
        (ImportFilesIden::Table, ImportFilesIden::Status),
        (ImportFilesIden::Table, ImportFilesIden::ErrorMessage),
        (ImportFilesIden::Table, ImportFilesIden::PausedUntil),
        (ImportFilesIden::Table, ImportFilesIden::DateOrder),
        (ImportFilesIden::Table, ImportFilesIden::CreatedAt),
    ]
}

#[macros::named_query]
pub fn get_session_files(params: GetImportFilesParams) -> DbQueryWithValues {
    let mut query = Query::select();
    query
        .columns(file_columns())
        .from(ImportFilesIden::Table)
        .inner_join(
            ImportSessionsIden::Table,
            session_id_matches(ImportFilesIden::Table, ImportFilesIden::SessionId),
        )
        .and_where(
            Expr::col((ImportSessionsIden::Table, ImportSessionsIden::Id)).eq(params.session_id),
        )
        .and_where(
            Expr::col((ImportSessionsIden::Table, ImportSessionsIden::UserId)).eq(params.user_id),
        );

    match params.search_type {
        GetImportFilesSearchType::ById(file_id) => {
            query.and_where(
                Expr::col((ImportFilesIden::Table, ImportFilesIden::FileId)).eq(file_id),
            );
        }
        GetImportFilesSearchType::All => {
            query
                .order_by(
                    (ImportFilesIden::Table, ImportFilesIden::CreatedAt),
                    Order::Asc,
                )
                .order_by(
                    (ImportFilesIden::Table, ImportFilesIden::FileId),
                    Order::Asc,
                );
        }
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_session_sources(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            (ImportSourcesIden::Table, ImportSourcesIden::Id),
            (ImportSourcesIden::Table, ImportSourcesIden::Kind),
            (ImportSourcesIden::Table, ImportSourcesIden::CreatedAt),
        ])
        .columns([
            (ImportFilesIden::Table, ImportFilesIden::FileId),
            (ImportFilesIden::Table, ImportFilesIden::Status),
            (ImportFilesIden::Table, ImportFilesIden::ErrorMessage),
            (ImportFilesIden::Table, ImportFilesIden::PausedUntil),
            (ImportFilesIden::Table, ImportFilesIden::DateOrder),
        ])
        .columns([
            (
                ImportBankAccountsIden::Table,
                ImportBankAccountsIden::BindingId,
            ),
            (
                ImportBankAccountsIden::Table,
                ImportBankAccountsIden::CutoffPageId,
            ),
        ])
        .from(ImportSourcesIden::Table)
        .left_join(
            ImportFilesIden::Table,
            Expr::col((ImportFilesIden::Table, ImportFilesIden::SourceId))
                .equals((ImportSourcesIden::Table, ImportSourcesIden::Id)),
        )
        .left_join(
            ImportBankAccountsIden::Table,
            Expr::col((
                ImportBankAccountsIden::Table,
                ImportBankAccountsIden::SourceId,
            ))
            .equals((ImportSourcesIden::Table, ImportSourcesIden::Id)),
        )
        .and_where(
            Expr::col((ImportSourcesIden::Table, ImportSourcesIden::SessionId))
                .in_subquery(owned_session(session_id, user_id)),
        )
        .order_by(
            (ImportSourcesIden::Table, ImportSourcesIden::CreatedAt),
            Order::Asc,
        )
        .order_by(
            (ImportSourcesIden::Table, ImportSourcesIden::Id),
            Order::Asc,
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_sources(
    session_id: Uuid,
    user_id: Uuid,
    kind: ImportSourceKind,
    ids: Vec<Uuid>,
) -> DbQueryWithValues {
    let rows = active_session(session_id, user_id)
        .clear_selects()
        .expr(Func::cust(Unnest).arg(Expr::val(ids)))
        .column(ImportSessionsIden::Id)
        .expr(Expr::val(kind))
        .to_owned();
    Query::insert()
        .into_table(ImportSourcesIden::Table)
        .columns([
            ImportSourcesIden::Id,
            ImportSourcesIden::SessionId,
            ImportSourcesIden::Kind,
        ])
        .select_from(rows)
        .expect("the select yields one value per inserted column")
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_bank_accounts(bank_accounts: &[ImportBankAccountInsert]) -> DbQueryWithValues {
    let mut query = Query::insert();
    query.into_table(ImportBankAccountsIden::Table).columns([
        ImportBankAccountsIden::SessionId,
        ImportBankAccountsIden::SourceId,
        ImportBankAccountsIden::BindingId,
        ImportBankAccountsIden::CutoffPageId,
    ]);
    for bank_account in bank_accounts {
        query.values_panic([
            bank_account.session_id.into(),
            bank_account.source_id.into(),
            bank_account.binding_id.into(),
            bank_account.cutoff_page_id.into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn delete_sources(params: DeleteImportSourcesParams) -> DbQueryWithValues {
    let mut query = Query::delete();
    query.from_table(ImportSourcesIden::Table).and_where(
        Expr::col(ImportSourcesIden::SessionId)
            .in_subquery(owned_session(params.session_id, params.user_id)),
    );
    if let DeleteImportSourcesSearchType::ById(source_id) = params.search_type {
        query.and_where(Expr::col(ImportSourcesIden::Id).eq(source_id));
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn attach_file(
    session_id: Uuid,
    user_id: Uuid,
    source_id: Uuid,
    file_id: Uuid,
    conversation_id: Uuid,
) -> DbQueryWithValues {
    let row = active_session(session_id, user_id)
        .expr(Expr::val(source_id))
        .expr(Expr::val(file_id))
        .expr(Expr::val(conversation_id))
        .to_owned();
    Query::insert()
        .into_table(ImportFilesIden::Table)
        .columns([
            ImportFilesIden::SessionId,
            ImportFilesIden::SourceId,
            ImportFilesIden::FileId,
            ImportFilesIden::ConversationId,
        ])
        .select_from(row)
        .expect("the select yields one value per inserted column")
        .returning(Query::returning().columns(file_columns().map(|(_, column)| column)))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

fn owned_session(session_id: Uuid, user_id: Uuid) -> SelectStatement {
    Query::select()
        .column(ImportSessionsIden::Id)
        .from(ImportSessionsIden::Table)
        .and_where(Expr::col(ImportSessionsIden::Id).eq(session_id))
        .and_where(Expr::col(ImportSessionsIden::UserId).eq(user_id))
        .to_owned()
}

fn active_session(session_id: Uuid, user_id: Uuid) -> SelectStatement {
    owned_session(session_id, user_id)
        .and_where(Expr::col(ImportSessionsIden::Lifecycle).eq(ImportSessionLifecycle::Active))
        .to_owned()
}

fn owned_file(session_id: Uuid, file_id: Uuid, user_id: Uuid) -> Condition {
    Cond::all()
        .add(Expr::col(ImportFilesIden::SessionId).eq(session_id))
        .add(Expr::col(ImportFilesIden::FileId).eq(file_id))
        .add(Expr::col(ImportFilesIden::SessionId).in_subquery(owned_session(session_id, user_id)))
}

fn owned_session_lines(session_id: Uuid, user_id: Uuid) -> SimpleExpr {
    Expr::col((
        ImportParsedLinesIden::Table,
        ImportParsedLinesIden::SessionId,
    ))
    .in_subquery(owned_session(session_id, user_id))
}

#[macros::named_query]
pub fn start_file_parse(session_id: Uuid, file_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::update()
        .table(ImportFilesIden::Table)
        .value(ImportFilesIden::Status, ImportFileStatus::Processing)
        .value(ImportFilesIden::ErrorMessage, Option::<String>::None)
        .value(ImportFilesIden::PausedUntil, Option::<OffsetDateTime>::None)
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(
            Expr::col(ImportFilesIden::Status)
                .is_in([ImportFileStatus::Pending, ImportFileStatus::Processing]),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn pause_file_parse(
    session_id: Uuid,
    file_id: Uuid,
    user_id: Uuid,
    until: OffsetDateTime,
) -> DbQueryWithValues {
    Query::update()
        .table(ImportFilesIden::Table)
        .value(ImportFilesIden::PausedUntil, until)
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(Expr::col(ImportFilesIden::Status).eq(ImportFileStatus::Processing))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn fail_file_parse(
    session_id: Uuid,
    file_id: Uuid,
    user_id: Uuid,
    message: String,
) -> DbQueryWithValues {
    Query::update()
        .table(ImportFilesIden::Table)
        .value(ImportFilesIden::Status, ImportFileStatus::Failed)
        .value(ImportFilesIden::ErrorMessage, message)
        .value(ImportFilesIden::PausedUntil, Option::<OffsetDateTime>::None)
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(
            Expr::col(ImportFilesIden::Status)
                .is_in([ImportFileStatus::Pending, ImportFileStatus::Processing]),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn requeue_file(
    session_id: Uuid,
    file_id: Uuid,
    user_id: Uuid,
    date_order: Option<ImportDateOrder>,
) -> DbQueryWithValues {
    let mut query = Query::update();
    query
        .table(ImportFilesIden::Table)
        .value(ImportFilesIden::Status, ImportFileStatus::Pending)
        .value(ImportFilesIden::ErrorMessage, Option::<String>::None)
        .value(ImportFilesIden::PausedUntil, Option::<OffsetDateTime>::None)
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(
            Expr::col(ImportFilesIden::SessionId).in_subquery(active_session(session_id, user_id)),
        );
    match date_order {
        None => {
            query.and_where(Expr::col(ImportFilesIden::Status).eq(ImportFileStatus::Failed));
        }
        Some(date_order) => {
            query
                .value(ImportFilesIden::DateOrder, date_order)
                .and_where(Expr::col(ImportFilesIden::Status).eq(ImportFileStatus::Parsed))
                .and_where(Expr::col(ImportFilesIden::DateOrder).ne(date_order));
        }
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn finish_file_parse(
    session_id: Uuid,
    file_id: Uuid,
    user_id: Uuid,
    date_order: Option<ImportDateOrder>,
) -> DbQueryWithValues {
    Query::update()
        .table(ImportFilesIden::Table)
        .value(ImportFilesIden::Status, ImportFileStatus::Parsed)
        .value(ImportFilesIden::ErrorMessage, Option::<String>::None)
        .value(ImportFilesIden::PausedUntil, Option::<OffsetDateTime>::None)
        .value(
            ImportFilesIden::DateOrder,
            date_order.map(|date_order| date_order.as_str().to_string()),
        )
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(Expr::col(ImportFilesIden::Status).eq(ImportFileStatus::Processing))
        .and_where(
            Expr::col(ImportFilesIden::SessionId).in_subquery(active_session(session_id, user_id)),
        )
        .returning_col(ImportFilesIden::SourceId)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_source_parsed_lines(
    session_id: Uuid,
    source_id: Uuid,
    user_id: Uuid,
) -> DbQueryWithValues {
    Query::delete()
        .from_table(ImportParsedLinesIden::Table)
        .and_where(
            Expr::col(ImportParsedLinesIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .and_where(Expr::col(ImportParsedLinesIden::SourceId).eq(source_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_source_assertions(
    session_id: Uuid,
    source_id: Uuid,
    user_id: Uuid,
) -> DbQueryWithValues {
    Query::delete()
        .from_table(ImportAssertionsIden::Table)
        .and_where(
            Expr::col(ImportAssertionsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .and_where(Expr::col(ImportAssertionsIden::SourceId).eq(source_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_parsed_lines(lines: &[ImportParsedLineInsert]) -> DbQueryWithValues {
    let mut query = Query::insert();
    query.into_table(ImportParsedLinesIden::Table).columns([
        ImportParsedLinesIden::Id,
        ImportParsedLinesIden::SessionId,
        ImportParsedLinesIden::SourceId,
        ImportParsedLinesIden::TransactionKey,
        ImportParsedLinesIden::AccountRef,
        ImportParsedLinesIden::DateTransacted,
        ImportParsedLinesIden::Description,
        ImportParsedLinesIden::RelatedAssetRef,
        ImportParsedLinesIden::ExternalId,
    ]);
    for line in lines {
        query.values_panic([
            line.id.into(),
            line.session_id.into(),
            line.source_id.into(),
            line.transaction_key.clone().into(),
            line.account_ref.clone().into(),
            line.date_transacted.into(),
            line.description.clone().into(),
            line.related_asset_ref.clone().into(),
            line.external_id.clone().into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn insert_parsed_legs(legs: &[ImportParsedLegInsert]) -> DbQueryWithValues {
    let mut query = Query::insert();
    query.into_table(ImportParsedLegsIden::Table).columns([
        ImportParsedLegsIden::ParsedLineId,
        ImportParsedLegsIden::AssetRef,
        ImportParsedLegsIden::Quantity,
        ImportParsedLegsIden::FeeType,
    ]);
    for leg in legs {
        query.values_panic([
            leg.parsed_line_id.into(),
            leg.asset_ref.clone().into(),
            leg.quantity.into(),
            leg.fee_type.map_or(Value::String(None), Value::from).into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn get_assertions(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            ImportAssertionsIden::SourceId,
            ImportAssertionsIden::AccountRef,
            ImportAssertionsIden::AssetRef,
            ImportAssertionsIden::AccountId,
            ImportAssertionsIden::AssetId,
            ImportAssertionsIden::Kind,
            ImportAssertionsIden::AsOfDate,
            ImportAssertionsIden::Quantity,
        ])
        .from(ImportAssertionsIden::Table)
        .and_where(
            Expr::col(ImportAssertionsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_assertions(assertions: &[ImportAssertionInsert]) -> DbQueryWithValues {
    let mut query = Query::insert();
    query.into_table(ImportAssertionsIden::Table).columns([
        ImportAssertionsIden::SessionId,
        ImportAssertionsIden::SourceId,
        ImportAssertionsIden::AccountRef,
        ImportAssertionsIden::AssetRef,
        ImportAssertionsIden::Kind,
        ImportAssertionsIden::AsOfDate,
        ImportAssertionsIden::Quantity,
    ]);
    for assertion in assertions {
        query.values_panic([
            assertion.session_id.into(),
            assertion.source_id.into(),
            assertion.account_ref.clone().into(),
            assertion.asset_ref.clone().into(),
            assertion.kind.into(),
            assertion.as_of_date.into(),
            assertion.quantity.into(),
        ]);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn upsert_entered_balance(
    session_id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    asset_id: i32,
    as_of_date: Date,
    quantity: Decimal,
) -> DbQueryWithValues {
    let row = Query::select()
        .column(ImportSessionsIden::Id)
        .expr(Expr::val(account_id))
        .expr(Expr::val(asset_id))
        .expr(Expr::val(ImportAssertionKind::Closing))
        .expr(Expr::val(as_of_date))
        .expr(Expr::val(quantity))
        .from(ImportSessionsIden::Table)
        .and_where(Expr::col(ImportSessionsIden::Id).eq(session_id))
        .and_where(Expr::col(ImportSessionsIden::UserId).eq(user_id))
        .and_where(Expr::col(ImportSessionsIden::Lifecycle).eq(ImportSessionLifecycle::Active))
        .to_owned();
    Query::insert()
        .into_table(ImportAssertionsIden::Table)
        .columns([
            ImportAssertionsIden::SessionId,
            ImportAssertionsIden::AccountId,
            ImportAssertionsIden::AssetId,
            ImportAssertionsIden::Kind,
            ImportAssertionsIden::AsOfDate,
            ImportAssertionsIden::Quantity,
        ])
        .select_from(row)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::columns([
                ImportAssertionsIden::SessionId,
                ImportAssertionsIden::AccountId,
                ImportAssertionsIden::AssetId,
            ])
            .update_columns([
                ImportAssertionsIden::AsOfDate,
                ImportAssertionsIden::Quantity,
            ])
            .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

fn checked_balances(session_id: Uuid, user_id: Uuid) -> Condition {
    Cond::all()
        .add(
            Expr::col(ImportAssertionsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .add(Expr::col(ImportAssertionsIden::SourceId).is_not_null())
        .add(Expr::col(ImportAssertionsIden::Kind).ne(ImportAssertionKind::ValueTotal))
}

#[macros::named_query]
pub fn get_session_account_refs(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    let stated = Query::select()
        .columns([
            ImportAssertionsIden::SourceId,
            ImportAssertionsIden::AccountRef,
        ])
        .from(ImportAssertionsIden::Table)
        .cond_where(checked_balances(session_id, user_id))
        .to_owned();
    Query::select()
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::SourceId,
        ))
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::AccountRef,
        ))
        .from(ImportParsedLinesIden::Table)
        .and_where(owned_session_lines(session_id, user_id))
        .union(UnionType::Distinct, stated)
        .order_by(ImportParsedLinesIden::SourceId, Order::Asc)
        .order_by(ImportParsedLinesIden::AccountRef, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_session_asset_refs(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    let counter_leg = Alias::new("counter_leg");
    let stated = Query::select()
        .columns([
            ImportAssertionsIden::SourceId,
            ImportAssertionsIden::AccountRef,
            ImportAssertionsIden::AssetRef,
        ])
        .expr_as(
            Expr::val(Option::<String>::None),
            Alias::new("counter_asset_ref"),
        )
        .from(ImportAssertionsIden::Table)
        .cond_where(checked_balances(session_id, user_id))
        .and_where(Expr::col(ImportAssertionsIden::AssetRef).is_not_null())
        .to_owned();
    let legs = Query::select()
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::SourceId,
        ))
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::AccountRef,
        ))
        .expr_as(
            Expr::col((ImportParsedLegsIden::Table, ImportParsedLegsIden::AssetRef)),
            Alias::new("asset_ref"),
        )
        .expr_as(
            Expr::col((counter_leg.clone(), ImportParsedLegsIden::AssetRef)),
            Alias::new("counter_asset_ref"),
        )
        .from(ImportParsedLegsIden::Table)
        .inner_join(
            ImportParsedLinesIden::Table,
            Expr::col((
                ImportParsedLegsIden::Table,
                ImportParsedLegsIden::ParsedLineId,
            ))
            .equals((ImportParsedLinesIden::Table, ImportParsedLinesIden::Id)),
        )
        .join_as(
            JoinType::LeftJoin,
            ImportParsedLegsIden::Table,
            counter_leg.clone(),
            Cond::all()
                .add(
                    Expr::col((counter_leg.clone(), ImportParsedLegsIden::ParsedLineId)).equals((
                        ImportParsedLegsIden::Table,
                        ImportParsedLegsIden::ParsedLineId,
                    )),
                )
                .add(
                    Expr::col((counter_leg.clone(), ImportParsedLegsIden::AssetRef)).ne(Expr::col(
                        (ImportParsedLegsIden::Table, ImportParsedLegsIden::AssetRef),
                    )),
                )
                .add(Expr::col((counter_leg.clone(), ImportParsedLegsIden::FeeType)).is_null())
                .add(
                    Expr::col((ImportParsedLegsIden::Table, ImportParsedLegsIden::FeeType))
                        .is_null(),
                ),
        )
        .and_where(owned_session_lines(session_id, user_id))
        .to_owned();
    let keyed_leg = Alias::new("keyed_leg");
    let line_of_leg = Expr::col((
        ImportParsedLegsIden::Table,
        ImportParsedLegsIden::ParsedLineId,
    ))
    .equals((ImportParsedLinesIden::Table, ImportParsedLinesIden::Id));
    let keyed_legs = Query::select()
        .distinct()
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::SourceId,
        ))
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::TransactionKey,
        ))
        .column((ImportParsedLegsIden::Table, ImportParsedLegsIden::AssetRef))
        .from(ImportParsedLegsIden::Table)
        .inner_join(ImportParsedLinesIden::Table, line_of_leg.clone())
        .and_where(owned_session_lines(session_id, user_id))
        .and_where(
            Expr::col((ImportParsedLegsIden::Table, ImportParsedLegsIden::FeeType)).is_null(),
        )
        .and_where(
            Expr::col((
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::TransactionKey,
            ))
            .is_not_null(),
        )
        .to_owned();
    let keyed = Query::select()
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::SourceId,
        ))
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::AccountRef,
        ))
        .expr_as(
            Expr::col((ImportParsedLegsIden::Table, ImportParsedLegsIden::AssetRef)),
            Alias::new("asset_ref"),
        )
        .expr_as(
            Expr::col((keyed_leg.clone(), ImportParsedLegsIden::AssetRef)),
            Alias::new("counter_asset_ref"),
        )
        .from(ImportParsedLegsIden::Table)
        .inner_join(ImportParsedLinesIden::Table, line_of_leg)
        .join_subquery(
            JoinType::InnerJoin,
            keyed_legs,
            keyed_leg.clone(),
            Cond::all()
                .add(
                    Expr::col((keyed_leg.clone(), ImportParsedLinesIden::SourceId)).equals((
                        ImportParsedLinesIden::Table,
                        ImportParsedLinesIden::SourceId,
                    )),
                )
                .add(
                    Expr::col((keyed_leg.clone(), ImportParsedLinesIden::TransactionKey)).equals((
                        ImportParsedLinesIden::Table,
                        ImportParsedLinesIden::TransactionKey,
                    )),
                )
                .add(
                    Expr::col((keyed_leg, ImportParsedLegsIden::AssetRef)).ne(Expr::col((
                        ImportParsedLegsIden::Table,
                        ImportParsedLegsIden::AssetRef,
                    ))),
                ),
        )
        .and_where(owned_session_lines(session_id, user_id))
        .and_where(
            Expr::col((ImportParsedLegsIden::Table, ImportParsedLegsIden::FeeType)).is_null(),
        )
        .to_owned();
    Query::select()
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::SourceId,
        ))
        .column((
            ImportParsedLinesIden::Table,
            ImportParsedLinesIden::AccountRef,
        ))
        .expr_as(
            Expr::col((
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::RelatedAssetRef,
            )),
            Alias::new("asset_ref"),
        )
        .expr_as(
            Expr::col((counter_leg.clone(), ImportParsedLegsIden::AssetRef)),
            Alias::new("counter_asset_ref"),
        )
        .from(ImportParsedLinesIden::Table)
        .join_as(
            JoinType::LeftJoin,
            ImportParsedLegsIden::Table,
            counter_leg.clone(),
            Cond::all()
                .add(
                    Expr::col((counter_leg.clone(), ImportParsedLegsIden::ParsedLineId))
                        .equals((ImportParsedLinesIden::Table, ImportParsedLinesIden::Id)),
                )
                .add(
                    Expr::col((counter_leg.clone(), ImportParsedLegsIden::AssetRef)).ne(Expr::col(
                        (
                            ImportParsedLinesIden::Table,
                            ImportParsedLinesIden::RelatedAssetRef,
                        ),
                    )),
                )
                .add(Expr::col((counter_leg, ImportParsedLegsIden::FeeType)).is_null()),
        )
        .and_where(owned_session_lines(session_id, user_id))
        .and_where(
            Expr::col((
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::RelatedAssetRef,
            ))
            .is_not_null(),
        )
        .union(UnionType::Distinct, legs)
        .union(UnionType::Distinct, keyed)
        .union(UnionType::Distinct, stated)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_account_mappings(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            ImportAccountMappingsIden::SourceId,
            ImportAccountMappingsIden::AccountRef,
            ImportAccountMappingsIden::AccountId,
        ])
        .from(ImportAccountMappingsIden::Table)
        .and_where(
            Expr::col(ImportAccountMappingsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .order_by(ImportAccountMappingsIden::SourceId, Order::Asc)
        .order_by(ImportAccountMappingsIden::AccountRef, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_account_answers(
    session_id: Uuid,
    user_id: Uuid,
    answers: Vec<ImportAccountMappingModel>,
) -> DbQueryWithValues {
    let answer = Alias::new("answer");
    let rows = Query::select()
        .column((ImportSourcesIden::Table, ImportSourcesIden::SessionId))
        .column((ImportSourcesIden::Table, ImportSourcesIden::Id))
        .column((answer.clone(), Alias::new("column2")))
        .column((answer.clone(), Alias::new("column3")))
        .from_values(
            answers
                .into_iter()
                .map(|answer| (answer.source_id, answer.account_ref, answer.account_id)),
            answer.clone(),
        )
        .inner_join(
            ImportSourcesIden::Table,
            Expr::col((ImportSourcesIden::Table, ImportSourcesIden::Id))
                .equals((answer, Alias::new("column1"))),
        )
        .and_where(
            Expr::col((ImportSourcesIden::Table, ImportSourcesIden::SessionId))
                .in_subquery(active_session(session_id, user_id)),
        )
        .to_owned();
    Query::insert()
        .into_table(ImportAccountMappingsIden::Table)
        .columns([
            ImportAccountMappingsIden::SessionId,
            ImportAccountMappingsIden::SourceId,
            ImportAccountMappingsIden::AccountRef,
            ImportAccountMappingsIden::AccountId,
        ])
        .select_from(rows)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::columns([
                ImportAccountMappingsIden::SessionId,
                ImportAccountMappingsIden::SourceId,
                ImportAccountMappingsIden::AccountRef,
            ])
            .do_nothing()
            .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_asset_mappings(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            ImportAssetMappingsIden::AssetRef,
            ImportAssetMappingsIden::AssetId,
        ])
        .from(ImportAssetMappingsIden::Table)
        .and_where(
            Expr::col(ImportAssetMappingsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_asset_answers(
    session_id: Uuid,
    user_id: Uuid,
    answers: Vec<ImportAssetMappingModel>,
) -> DbQueryWithValues {
    let answer = Alias::new("answer");
    let rows = active_session(session_id, user_id)
        .clear_selects()
        .column((ImportSessionsIden::Table, ImportSessionsIden::Id))
        .column((answer.clone(), Alias::new("column1")))
        .column((answer.clone(), Alias::new("column2")))
        .from_values(
            answers
                .into_iter()
                .map(|answer| (answer.asset_ref, answer.asset_id)),
            answer,
        )
        .to_owned();
    Query::insert()
        .into_table(ImportAssetMappingsIden::Table)
        .columns([
            ImportAssetMappingsIden::SessionId,
            ImportAssetMappingsIden::AssetRef,
            ImportAssetMappingsIden::AssetId,
        ])
        .select_from(rows)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::columns([
                ImportAssetMappingsIden::SessionId,
                ImportAssetMappingsIden::AssetRef,
            ])
            .do_nothing()
            .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_parsed_lines(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            (ImportParsedLinesIden::Table, ImportParsedLinesIden::Id),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::SourceId,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::TransactionKey,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::AccountRef,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::DateTransacted,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::Description,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::RelatedAssetRef,
            ),
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::ExternalId,
            ),
        ])
        .from(ImportParsedLinesIden::Table)
        .and_where(owned_session_lines(session_id, user_id))
        .order_by(
            (
                ImportParsedLinesIden::Table,
                ImportParsedLinesIden::DateTransacted,
            ),
            Order::Asc,
        )
        .order_by(
            (ImportParsedLinesIden::Table, ImportParsedLinesIden::Id),
            Order::Asc,
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_classification_run(session_id: Uuid) -> DbQueryWithValues {
    Query::insert()
        .into_table(ImportClassificationRunsIden::Table)
        .columns([ImportClassificationRunsIden::SessionId])
        .values_panic([session_id.into()])
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_line_classifications(
    session_id: Uuid,
    run_id: Uuid,
    user_id: Uuid,
    type_id: DatabaseTransactionTypes,
    category_id: Option<i32>,
    line_ids: Vec<Uuid>,
) -> DbQueryWithValues {
    let lines = Query::select()
        .expr(Expr::val(run_id))
        .column(ImportParsedLinesIden::Id)
        .expr(Expr::val(type_id as i32))
        .expr(Expr::val(category_id))
        .from(ImportParsedLinesIden::Table)
        .and_where(owned_session_lines(session_id, user_id))
        .and_where(Expr::col(ImportParsedLinesIden::Id).eq(PgFunc::any(Expr::val(line_ids))))
        .to_owned();
    Query::insert()
        .into_table(ImportLineClassificationsIden::Table)
        .columns([
            ImportLineClassificationsIden::RunId,
            ImportLineClassificationsIden::ParsedLineId,
            ImportLineClassificationsIden::TypeId,
            ImportLineClassificationsIden::CategoryId,
        ])
        .select_from(lines)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::columns([
                ImportLineClassificationsIden::RunId,
                ImportLineClassificationsIden::ParsedLineId,
            ])
            .update_columns([
                ImportLineClassificationsIden::TypeId,
                ImportLineClassificationsIden::CategoryId,
            ])
            .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_line_classifications(
    session_id: Uuid,
    run_id: Uuid,
    user_id: Uuid,
) -> DbQueryWithValues {
    Query::select()
        .columns([
            ImportLineClassificationsIden::ParsedLineId,
            ImportLineClassificationsIden::TypeId,
            ImportLineClassificationsIden::CategoryId,
        ])
        .from(ImportLineClassificationsIden::Table)
        .and_where(Expr::col(ImportLineClassificationsIden::RunId).eq(run_id))
        .and_where(
            Expr::col(ImportLineClassificationsIden::RunId).in_subquery(
                Query::select()
                    .column(ImportClassificationRunsIden::Id)
                    .from(ImportClassificationRunsIden::Table)
                    .and_where(
                        Expr::col(ImportClassificationRunsIden::SessionId)
                            .in_subquery(owned_session(session_id, user_id)),
                    )
                    .to_owned(),
            ),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn finish_classification_run(
    session_id: Uuid,
    run_id: Uuid,
    user_id: Uuid,
    outcome: ClassificationRunOutcome,
) -> DbQueryWithValues {
    let mut query = Query::update();
    query.table(ImportClassificationRunsIden::Table);
    match outcome {
        ClassificationRunOutcome::Classified => query.value(
            ImportClassificationRunsIden::ClassifiedAt,
            Expr::cust("now()"),
        ),
        ClassificationRunOutcome::Failed(message) => {
            query.value(ImportClassificationRunsIden::ErrorMessage, message)
        }
    };
    query
        .and_where(Expr::col(ImportClassificationRunsIden::Id).eq(run_id))
        .and_where(Expr::col(ImportClassificationRunsIden::ClassifiedAt).is_null())
        .and_where(Expr::col(ImportClassificationRunsIden::ErrorMessage).is_null())
        .and_where(
            Expr::col(ImportClassificationRunsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_classification_conversation(
    run_id: Uuid,
    conversation_id: Uuid,
) -> DbQueryWithValues {
    Query::insert()
        .into_table(ImportClassificationConversationsIden::Table)
        .columns([
            ImportClassificationConversationsIden::RunId,
            ImportClassificationConversationsIden::ConversationId,
        ])
        .values_panic([run_id.into(), conversation_id.into()])
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_classification_conversations(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .column((
            ImportClassificationConversationsIden::Table,
            ImportClassificationConversationsIden::ConversationId,
        ))
        .from(ImportClassificationConversationsIden::Table)
        .inner_join(
            ImportClassificationRunsIden::Table,
            Expr::col((
                ImportClassificationConversationsIden::Table,
                ImportClassificationConversationsIden::RunId,
            ))
            .equals((
                ImportClassificationRunsIden::Table,
                ImportClassificationRunsIden::Id,
            )),
        )
        .and_where(
            Expr::col((
                ImportClassificationRunsIden::Table,
                ImportClassificationRunsIden::SessionId,
            ))
            .in_subquery(owned_session(session_id, user_id)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_parsed_legs(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            (
                ImportParsedLegsIden::Table,
                ImportParsedLegsIden::ParsedLineId,
            ),
            (ImportParsedLegsIden::Table, ImportParsedLegsIden::AssetRef),
            (ImportParsedLegsIden::Table, ImportParsedLegsIden::Quantity),
            (ImportParsedLegsIden::Table, ImportParsedLegsIden::FeeType),
        ])
        .from(ImportParsedLegsIden::Table)
        .inner_join(
            ImportParsedLinesIden::Table,
            Expr::col((
                ImportParsedLegsIden::Table,
                ImportParsedLegsIden::ParsedLineId,
            ))
            .equals((ImportParsedLinesIden::Table, ImportParsedLinesIden::Id)),
        )
        .and_where(owned_session_lines(session_id, user_id))
        .order_by(
            (ImportParsedLegsIden::Table, ImportParsedLegsIden::Id),
            Order::Asc,
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_staged_decisions(session_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .columns([
            ImportStagedDecisionsIden::Id,
            ImportStagedDecisionsIden::AskKey,
            ImportStagedDecisionsIden::Verb,
            ImportStagedDecisionsIden::TransactionId,
        ])
        .from(ImportStagedDecisionsIden::Table)
        .and_where(
            Expr::col(ImportStagedDecisionsIden::SessionId)
                .in_subquery(owned_session(session_id, user_id)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn pin_staged_decisions(
    session_id: Uuid,
    user_id: Uuid,
    ids: Vec<Uuid>,
    verb: ImportVerb,
    transaction_id: Option<Uuid>,
) -> DbQueryWithValues {
    let rows = Query::select()
        .distinct()
        .expr(Func::cust(Unnest).arg(Expr::val(ids)))
        .column(ImportSessionsIden::Id)
        .expr(Expr::val(verb))
        .expr(Expr::val(transaction_id))
        .from(ImportSessionsIden::Table)
        .and_where(Expr::col(ImportSessionsIden::Id).eq(session_id))
        .and_where(Expr::col(ImportSessionsIden::UserId).eq(user_id))
        .and_where(Expr::col(ImportSessionsIden::Lifecycle).eq(ImportSessionLifecycle::Active))
        .to_owned();
    Query::insert()
        .into_table(ImportStagedDecisionsIden::Table)
        .columns([
            ImportStagedDecisionsIden::Id,
            ImportStagedDecisionsIden::SessionId,
            ImportStagedDecisionsIden::Verb,
            ImportStagedDecisionsIden::TransactionId,
        ])
        .select_from(rows)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::column(ImportStagedDecisionsIden::Id)
                .update_columns([
                    ImportStagedDecisionsIden::Verb,
                    ImportStagedDecisionsIden::TransactionId,
                ])
                .action_and_where(
                    Expr::col((
                        ImportStagedDecisionsIden::Table,
                        ImportStagedDecisionsIden::AskKey,
                    ))
                    .is_null()
                    .and(
                        Expr::col((
                            ImportStagedDecisionsIden::Table,
                            ImportStagedDecisionsIden::SessionId,
                        ))
                        .equals((CommonsIden::Excluded, ImportStagedDecisionsIden::SessionId)),
                    ),
                )
                .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_ask_decisions(
    session_id: Uuid,
    user_id: Uuid,
    ask_key: String,
    verb: ImportVerb,
    decisions: Vec<(Uuid, Uuid)>,
) -> DbQueryWithValues {
    let (ids, transaction_ids): (Vec<Uuid>, Vec<Uuid>) = decisions.into_iter().unzip();
    let rows = active_session(session_id, user_id)
        .clear_selects()
        .expr(Func::cust(Unnest).arg(Expr::val(ids)))
        .column(ImportSessionsIden::Id)
        .expr(Expr::val(ask_key))
        .expr(Expr::val(verb))
        .expr(Func::cust(Unnest).arg(Expr::val(transaction_ids)))
        .to_owned();
    Query::insert()
        .into_table(ImportStagedDecisionsIden::Table)
        .columns([
            ImportStagedDecisionsIden::Id,
            ImportStagedDecisionsIden::SessionId,
            ImportStagedDecisionsIden::AskKey,
            ImportStagedDecisionsIden::Verb,
            ImportStagedDecisionsIden::TransactionId,
        ])
        .select_from(rows)
        .expect("the select yields one value per inserted column")
        .on_conflict(
            OnConflict::columns([
                ImportStagedDecisionsIden::SessionId,
                ImportStagedDecisionsIden::AskKey,
                ImportStagedDecisionsIden::TransactionId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn release_staged_decisions(
    session_id: Uuid,
    user_id: Uuid,
    ids: Vec<Uuid>,
    ask_keys: Vec<String>,
) -> DbQueryWithValues {
    let pinned_rows = Cond::all()
        .add(Expr::col(ImportStagedDecisionsIden::Id).eq(PgFunc::any(Expr::val(ids))))
        .add(Expr::col(ImportStagedDecisionsIden::AskKey).is_null());
    let answers = Expr::col(ImportStagedDecisionsIden::AskKey).eq(PgFunc::any(Expr::val(ask_keys)));
    Query::delete()
        .from_table(ImportStagedDecisionsIden::Table)
        .cond_where(
            Cond::all()
                .add(Cond::any().add(pinned_rows).add(answers))
                .add(
                    Expr::col(ImportStagedDecisionsIden::SessionId)
                        .in_subquery(active_session(session_id, user_id)),
                ),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn complete_session(session_id: Uuid, user_id: Uuid, file_ids: Vec<Uuid>) -> DbQueryWithValues {
    let files = Query::select()
        .column(ImportFilesIden::FileId)
        .from(ImportFilesIden::Table)
        .and_where(Expr::col(ImportFilesIden::SessionId).eq(session_id))
        .order_by(ImportFilesIden::FileId, Order::Asc)
        .to_owned();
    Query::update()
        .table(ImportSessionsIden::Table)
        .value(
            ImportSessionsIden::Lifecycle,
            ImportSessionLifecycle::Completed,
        )
        .and_where(Expr::col(ImportSessionsIden::Id).eq(session_id))
        .and_where(Expr::col(ImportSessionsIden::UserId).eq(user_id))
        .and_where(Expr::col(ImportSessionsIden::Lifecycle).eq(ImportSessionLifecycle::Active))
        .and_where(
            Func::cust(ArrayFunc)
                .arg(SimpleExpr::SubQuery(
                    None,
                    Box::new(files.into_sub_query_statement()),
                ))
                .eq(Expr::val(file_ids)),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn detach_file(session_id: Uuid, file_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::delete()
        .from_table(ImportFilesIden::Table)
        .cond_where(owned_file(session_id, file_id, user_id))
        .and_where(
            Expr::col(ImportFilesIden::SessionId).in_subquery(active_session(session_id, user_id)),
        )
        .returning(Query::returning().columns(file_columns().map(|(_, column)| column)))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}
