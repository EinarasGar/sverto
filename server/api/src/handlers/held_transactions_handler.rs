use axum::{extract::Path, http::StatusCode, Json};
use business::dtos::transaction_dto::TransactionDto;
use itertools::Itertools;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthenticatedUserId,
    converters::{
        transaction_dtos_to_account_ids_hashset, transaction_dtos_to_asset_ids_hashset,
        transaction_dtos_to_category_ids_hashset,
    },
    errors::ApiError,
    extractors::ValidatedJson,
    states::{
        AccountsServiceState, AssetsServiceState, BankIngestionServiceState, CategoryServiceState,
    },
    view_models::{
        errors::{GetResponses, UpdateResponses},
        held_transactions::{
            decide_held_transaction::DecideHeldTransactionRequestViewModel,
            get_held_transactions::GetHeldTransactionsResponseViewModel,
        },
        transactions::base_models::metadata_lookup::MetadataLookupTables,
    },
};

#[derive(Deserialize)]
pub(crate) struct HeldTransactionIdPath {
    held_transaction_id: Uuid,
}

/// List Held Transactions
///
/// Lists the bank rows a sync kept out of the ledger because they may already be recorded, newest
/// first, each with the ledger transactions it may be (best match first). Held rows count in no
/// balance until they are decided.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/held-transactions",
    tag = "Held Transactions",
    responses(
        (status = 200, description = "Every held bank row of the user.", body = GetHeldTransactionsResponseViewModel),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id))]
pub async fn list_held_transactions(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    BankIngestionServiceState(service): BankIngestionServiceState,
    AssetsServiceState(asset_service): AssetsServiceState,
    AccountsServiceState(accounts_service): AccountsServiceState,
    CategoryServiceState(category_service): CategoryServiceState,
) -> Result<Json<GetHeldTransactionsResponseViewModel>, ApiError> {
    let held = service.held_transactions(user_id).await?;

    let candidates: Vec<&TransactionDto> = held.iter().flat_map(|held| &held.candidates).collect();
    let mut account_ids = transaction_dtos_to_account_ids_hashset(&candidates);
    account_ids.extend(held.iter().map(|held| held.held.account_id));
    let mut asset_ids = transaction_dtos_to_asset_ids_hashset(&candidates);
    asset_ids.extend(held.iter().map(|held| held.held.asset_id));
    let (assets, accounts, categories) = tokio::try_join!(
        asset_service.get_assets(asset_ids),
        accounts_service.get_accounts(account_ids),
        category_service.get_categories(transaction_dtos_to_category_ids_hashset(&candidates)),
    )?;

    Ok(Json(GetHeldTransactionsResponseViewModel {
        held_transactions: held.into_iter().map(Into::into).collect(),
        lookup_tables: MetadataLookupTables {
            assets: assets.into_iter().map_into().collect(),
            accounts: accounts.into_iter().map_into().collect(),
            categories: categories.into_iter().map_into().collect(),
        },
    }))
}

/// Decide Held Transaction
///
/// Settles a held bank row. `same` records it on one of its candidates: a candidate that is a
/// single entry in the same account and asset takes the bank's amount and date and keeps its
/// description and category; any other candidate is left as it is. `different` adds the bank row
/// to the ledger as a transaction of its own. Either way the row is no longer held. A candidate
/// that already has another row of the same bank account can't take this one.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/held-transactions/{held_transaction_id}/decision",
    tag = "Held Transactions",
    request_body = DecideHeldTransactionRequestViewModel,
    responses(
        (status = 204, description = "Decision recorded."),
        (status = 409, description = "The held row was decided or recorded already, or the chosen candidate has since taken another row of the same bank account."),
        UpdateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("held_transaction_id" = Uuid, Path, description = "Id of the held bank row."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, held_transaction_id = %held_transaction_id))]
pub async fn decide_held_transaction(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(HeldTransactionIdPath {
        held_transaction_id,
    }): Path<HeldTransactionIdPath>,
    BankIngestionServiceState(service): BankIngestionServiceState,
    ValidatedJson(body): ValidatedJson<DecideHeldTransactionRequestViewModel>,
) -> Result<StatusCode, ApiError> {
    service
        .decide(user_id, held_transaction_id, body.into())
        .await
        .map_err(ApiError::from_anyhow)?;
    Ok(StatusCode::NO_CONTENT)
}
