use sea_query::extension::postgres::PgFunc;
use sea_query::*;
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;

use crate::idens::account_idens::AccountIden;
use crate::idens::asset_idens::AssetsIden;
use crate::idens::property_idens::{PropertyIden, PropertyLoanIden};
use crate::idens::ArrayFunc;
use crate::models::property_models::InsertPropertyModel;
use crate::query_params::get_properties_params::GetPropertiesParams;

use super::DbQueryWithValues;

#[macros::named_query]
pub fn get_properties(params: GetPropertiesParams) -> DbQueryWithValues {
    let loan_account = Alias::new("loan_account");
    let loan_account_ids = Query::select()
        .column((PropertyLoanIden::Table, PropertyLoanIden::AccountId))
        .from(PropertyLoanIden::Table)
        .join_as(
            JoinType::InnerJoin,
            AccountIden::Table,
            loan_account.clone(),
            Expr::col((loan_account.clone(), AccountIden::Id))
                .equals((PropertyLoanIden::Table, PropertyLoanIden::AccountId)),
        )
        .and_where(
            Expr::col((PropertyLoanIden::Table, PropertyLoanIden::PropertyId))
                .equals((PropertyIden::Table, PropertyIden::Id)),
        )
        .and_where(Expr::col((loan_account.clone(), AccountIden::Active)).eq(true))
        .order_by((loan_account, AccountIden::AccountName), Order::Asc)
        .to_owned();

    let mut query = Query::select();
    query
        .column((PropertyIden::Table, PropertyIden::Id))
        .column((PropertyIden::Table, PropertyIden::Kind))
        .column((PropertyIden::Table, PropertyIden::AccountId))
        .column((PropertyIden::Table, PropertyIden::AssetId))
        .column((AccountIden::Table, AccountIden::AccountName))
        .column((AccountIden::Table, AccountIden::OwnershipShare))
        .expr_as(
            Expr::col((AssetsIden::Table, AssetsIden::BasePairId)),
            Alias::new("currency_asset_id"),
        )
        .expr_as(
            Func::cust(ArrayFunc).arg(SimpleExpr::SubQuery(
                None,
                Box::new(loan_account_ids.into_sub_query_statement()),
            )),
            Alias::new("loan_account_ids"),
        )
        .from(PropertyIden::Table)
        .inner_join(
            AccountIden::Table,
            Expr::col((AccountIden::Table, AccountIden::Id))
                .equals((PropertyIden::Table, PropertyIden::AccountId)),
        )
        .inner_join(
            AssetsIden::Table,
            Expr::col((AssetsIden::Table, AssetsIden::Id))
                .equals((PropertyIden::Table, PropertyIden::AssetId)),
        )
        .and_where(Expr::col((AccountIden::Table, AccountIden::UserId)).eq(params.user_id))
        .and_where(Expr::col((AccountIden::Table, AccountIden::Active)).eq(true))
        .order_by((AccountIden::Table, AccountIden::AccountName), Order::Asc);

    if let Some(property_id) = params.property_id {
        query.and_where(Expr::col((PropertyIden::Table, PropertyIden::Id)).eq(property_id));
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn insert_property(model: InsertPropertyModel) -> DbQueryWithValues {
    Query::insert()
        .into_table(PropertyIden::Table)
        .columns([
            PropertyIden::AccountId,
            PropertyIden::AssetId,
            PropertyIden::Kind,
        ])
        .values_panic([
            model.account_id.into(),
            model.asset_id.into(),
            model.kind.into(),
        ])
        .returning_col(PropertyIden::Id)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn insert_property_loans(
    user_id: Uuid,
    property_id: Uuid,
    account_ids: Vec<Uuid>,
) -> DbQueryWithValues {
    let account_is_a_property = Query::select()
        .expr(Expr::val(1))
        .from(PropertyIden::Table)
        .and_where(
            Expr::col((PropertyIden::Table, PropertyIden::AccountId))
                .equals((AccountIden::Table, AccountIden::Id)),
        )
        .to_owned();
    let loans = Query::select()
        .column((AccountIden::Table, AccountIden::Id))
        .expr(Expr::val(property_id))
        .from(AccountIden::Table)
        .and_where(
            Expr::col((AccountIden::Table, AccountIden::Id))
                .eq(PgFunc::any(Expr::val(account_ids))),
        )
        .and_where(Expr::col((AccountIden::Table, AccountIden::UserId)).eq(user_id))
        .and_where(Expr::col((AccountIden::Table, AccountIden::Active)).eq(true))
        .and_where(Expr::exists(account_is_a_property).not())
        .and_where(Expr::exists(owned_property(user_id, property_id)))
        .to_owned();
    Query::insert()
        .into_table(PropertyLoanIden::Table)
        .columns([PropertyLoanIden::AccountId, PropertyLoanIden::PropertyId])
        .select_from(loans)
        .expect("the select yields one value per inserted column")
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_property_loan(
    user_id: Uuid,
    property_id: Uuid,
    account_id: Uuid,
) -> DbQueryWithValues {
    Query::delete()
        .from_table(PropertyLoanIden::Table)
        .and_where(Expr::col((PropertyLoanIden::Table, PropertyLoanIden::AccountId)).eq(account_id))
        .and_where(
            Expr::col((PropertyLoanIden::Table, PropertyLoanIden::PropertyId)).eq(property_id),
        )
        .and_where(Expr::exists(owned_property(user_id, property_id)))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

/// For asset lists and searches only; lookups by id must keep returning property assets.
pub fn is_property_asset() -> SimpleExpr {
    Expr::exists(
        Query::select()
            .expr(Expr::val(1))
            .from(PropertyIden::Table)
            .and_where(
                Expr::col((PropertyIden::Table, PropertyIden::AssetId))
                    .equals((AssetsIden::Table, AssetsIden::Id)),
            )
            .to_owned(),
    )
}

fn owned_property(user_id: Uuid, property_id: Uuid) -> SelectStatement {
    let property_account = Alias::new("property_account");
    Query::select()
        .expr(Expr::val(1))
        .from(PropertyIden::Table)
        .join_as(
            JoinType::InnerJoin,
            AccountIden::Table,
            property_account.clone(),
            Expr::col((property_account.clone(), AccountIden::Id))
                .equals((PropertyIden::Table, PropertyIden::AccountId)),
        )
        .and_where(Expr::col((PropertyIden::Table, PropertyIden::Id)).eq(property_id))
        .and_where(Expr::col((property_account.clone(), AccountIden::UserId)).eq(user_id))
        .and_where(Expr::col((property_account, AccountIden::Active)).eq(true))
        .to_owned()
}
