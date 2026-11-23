use sea_query::extension::postgres::PgFunc;
use sea_query::{Alias, Expr, ExprTrait, Order, PostgresQueryBuilder, Query};
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;

use super::DbQueryWithValues;
use crate::idens::account_idens::AccountIden;
use crate::idens::account_identifier_idens::AccountIdentifierIden;
use crate::models::account_models::AccountIdentifierInsert;

#[macros::named_query]
pub fn insert_account_identifiers(
    user_id: Uuid,
    rows: Vec<AccountIdentifierInsert>,
) -> DbQueryWithValues {
    let identifier = Alias::new("identifier");
    let user_accounts = Query::select()
        .column(AccountIden::Id)
        .from(AccountIden::Table)
        .and_where(Expr::col(AccountIden::UserId).eq(user_id))
        .to_owned();
    let recorded = Query::select()
        .expr(Expr::val(1))
        .from(AccountIdentifierIden::Table)
        .and_where(
            Expr::col((AccountIdentifierIden::Table, AccountIdentifierIden::Kind))
                .equals((identifier.clone(), Alias::new("column2"))),
        )
        .and_where(
            Expr::col((AccountIdentifierIden::Table, AccountIdentifierIden::Value))
                .equals((identifier.clone(), Alias::new("column3"))),
        )
        .and_where(
            Expr::col((
                AccountIdentifierIden::Table,
                AccountIdentifierIden::AccountId,
            ))
            .in_subquery(user_accounts),
        )
        .to_owned();
    let new_rows = Query::select()
        .column((AccountIden::Table, AccountIden::Id))
        .column((identifier.clone(), Alias::new("column2")))
        .column((identifier.clone(), Alias::new("column3")))
        .from_values(
            rows.into_iter()
                .map(|row| (row.account_id, row.kind, row.value)),
            identifier.clone(),
        )
        .inner_join(
            AccountIden::Table,
            Expr::col((AccountIden::Table, AccountIden::Id))
                .equals((identifier, Alias::new("column1"))),
        )
        .and_where(Expr::col((AccountIden::Table, AccountIden::UserId)).eq(user_id))
        .and_where(Expr::exists(recorded).not())
        .to_owned();
    Query::insert()
        .into_table(AccountIdentifierIden::Table)
        .columns([
            AccountIdentifierIden::AccountId,
            AccountIdentifierIden::Kind,
            AccountIdentifierIden::Value,
        ])
        .select_from(new_rows)
        .expect("the select yields one value per inserted column")
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_identifiers_for_accounts(account_ids: Vec<Uuid>) -> DbQueryWithValues {
    Query::select()
        .column(AccountIdentifierIden::AccountId)
        .column(AccountIdentifierIden::Kind)
        .column(AccountIdentifierIden::Value)
        .from(AccountIdentifierIden::Table)
        .and_where(Expr::col(AccountIdentifierIden::AccountId).is_in(account_ids))
        .order_by(AccountIdentifierIden::Value, Order::Asc)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_account_identifiers(account_id: Uuid) -> DbQueryWithValues {
    Query::delete()
        .from_table(AccountIdentifierIden::Table)
        .and_where(Expr::col(AccountIdentifierIden::AccountId).eq(account_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn find_identifiers_by_value(
    user_id: Uuid,
    exclude_account_id: Option<Uuid>,
    values: Vec<String>,
) -> DbQueryWithValues {
    let mut builder = Query::select()
        .column((
            AccountIdentifierIden::Table,
            AccountIdentifierIden::AccountId,
        ))
        .column((AccountIdentifierIden::Table, AccountIdentifierIden::Kind))
        .column((AccountIdentifierIden::Table, AccountIdentifierIden::Value))
        .from(AccountIdentifierIden::Table)
        .inner_join(
            AccountIden::Table,
            Expr::col((AccountIden::Table, AccountIden::Id)).equals((
                AccountIdentifierIden::Table,
                AccountIdentifierIden::AccountId,
            )),
        )
        .and_where(Expr::col((AccountIden::Table, AccountIden::UserId)).eq(user_id))
        .and_where(
            Expr::col((AccountIdentifierIden::Table, AccountIdentifierIden::Value))
                .eq(PgFunc::any(Expr::val(values))),
        )
        .to_owned();
    if let Some(id) = exclude_account_id {
        builder.and_where(
            Expr::col((
                AccountIdentifierIden::Table,
                AccountIdentifierIden::AccountId,
            ))
            .ne(id),
        );
    }
    builder.build_sqlx(PostgresQueryBuilder).into()
}
