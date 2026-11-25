use std::sync::Mutex;

use super::accounts::{refresh_accounts, AccountsModule};
use super::assets::{add_user_asset_rate, get_asset_pair_converted_rates, get_asset_pair_rates};
use super::infra::SharedInfra;
use super::transactions::{
    evict_transaction_caches, get_editable_transaction, update_individual_transaction,
    TransactionsModule,
};
use crate::api::account_transactions::{
    extract_account_transactions, scoped_transactions_page_path,
};
use crate::api::accounts::{extract_accounts, LOAN, MORTGAGE};
use crate::api::holdings::extract_account_balances;
use crate::api::properties::{
    attach_properties, build_create_property_body, extract_created_property_id, extract_properties,
    fill_property_figures, value_trend,
};
use crate::error::{server_error, ApiError};
use crate::models::{
    AccountListItem, CreatePropertyInput, CreateTransactionInput, PropertyDetail,
    PropertyValueChart, TransactionListItem, TransactionListScope, UpdatePropertyInput,
};

pub async fn create_property(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    input: CreatePropertyInput,
    auth_token: Option<&str>,
) -> Result<String, ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let body = build_create_property_body(input)?;
    let resp = infra
        .post(
            &format!("/api/users/{user_id}/properties"),
            &body,
            auth_token,
        )
        .await;

    // The server stores the valuation after committing the property, so even a failed
    // create may have written one; refresh either way.
    evict_transaction_caches(infra, &user_id);
    infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/portfolio"));
    refresh_accounts(infra, module, auth_token).await;

    let resp = resp?;
    if resp.status >= 400 {
        return Err(server_error(resp.status, &resp.body));
    }
    extract_created_property_id(&resp.body).map_err(|reason| ApiError::Parse { reason })
}

pub async fn update_property(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    property_id: &str,
    input: UpdatePropertyInput,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let body = serde_json::json!({
        "name": input.name,
        "ownership_share": input.ownership_share,
    })
    .to_string();
    let resp = infra
        .put(
            &format!("/api/users/{user_id}/properties/{property_id}"),
            &body,
            auth_token,
        )
        .await?;
    if resp.status >= 400 {
        return Err(server_error(resp.status, &resp.body));
    }
    infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/portfolio"));
    refresh_accounts(infra, module, auth_token).await;
    Ok(())
}

pub async fn link_property_loan(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    property_id: &str,
    account_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let resp = infra
        .put(
            &format!("/api/users/{user_id}/properties/{property_id}/loans/{account_id}"),
            "",
            auth_token,
        )
        .await?;
    if resp.status >= 400 {
        return Err(server_error(resp.status, &resp.body));
    }
    refresh_accounts(infra, module, auth_token).await;
    Ok(())
}

pub async fn unlink_property_loan(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    property_id: &str,
    account_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let resp = infra
        .delete(
            &format!("/api/users/{user_id}/properties/{property_id}/loans/{account_id}"),
            auth_token,
        )
        .await?;
    if resp.status >= 400 {
        return Err(server_error(resp.status, &resp.body));
    }
    refresh_accounts(infra, module, auth_token).await;
    Ok(())
}

pub async fn add_property_valuation(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    asset_id: i32,
    currency_asset_id: i32,
    date: i64,
    value: f64,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let result =
        add_user_asset_rate(infra, asset_id, currency_asset_id, date, value, auth_token).await;
    refresh_property(infra, module, asset_id, auth_token).await;
    result
}

pub async fn edit_property_valuation(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    asset_id: i32,
    currency_asset_id: i32,
    old_date: i64,
    new_date: i64,
    value: f64,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    // A valuation's time is unique per asset, so an edit within the same second must free the
    // slot first; otherwise the new valuation is added first so a failed add keeps the old one.
    let result = if (new_date - old_date).abs() <= 1 {
        match remove_valuation(infra, asset_id, currency_asset_id, old_date, auth_token).await {
            Ok(()) => {
                add_user_asset_rate(
                    infra,
                    asset_id,
                    currency_asset_id,
                    new_date,
                    value,
                    auth_token,
                )
                .await
            }
            Err(e) => Err(e),
        }
    } else {
        match add_user_asset_rate(
            infra,
            asset_id,
            currency_asset_id,
            new_date,
            value,
            auth_token,
        )
        .await
        {
            Ok(()) => {
                remove_valuation(infra, asset_id, currency_asset_id, old_date, auth_token).await
            }
            Err(e) => Err(e),
        }
    };
    refresh_property(infra, module, asset_id, auth_token).await;
    result
}

pub async fn get_property_purchase_date(
    infra: &SharedInfra,
    account_id: &str,
    asset_id: i32,
    auth_token: Option<&str>,
) -> Result<i64, ApiError> {
    Ok(
        find_purchase_transaction(infra, account_id, asset_id, auth_token)
            .await?
            .date,
    )
}

pub async fn set_property_purchase_date(
    infra: &SharedInfra,
    accounts: &Mutex<AccountsModule>,
    transactions: &Mutex<TransactionsModule>,
    account_id: &str,
    asset_id: i32,
    currency_asset_id: i32,
    date: i64,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let purchase_id = find_purchase_transaction(infra, account_id, asset_id, auth_token)
        .await?
        .id;
    let editable = get_editable_transaction(infra, &purchase_id, auth_token).await?;
    let old_date = editable.date;
    update_individual_transaction(
        infra,
        transactions,
        &purchase_id,
        CreateTransactionInput {
            transaction_id: Some(purchase_id.clone()),
            type_key: editable.type_key,
            date,
            primary_entry_id: editable.primary_entry_id,
            primary_account_id: editable.primary_account_id,
            primary_asset_id: editable.primary_asset_id,
            primary_amount: editable.primary_amount,
            secondary_entry_id: editable.secondary_entry_id,
            secondary_account_id: editable.secondary_account_id,
            secondary_asset_id: editable.secondary_asset_id,
            secondary_amount: editable.secondary_amount,
            origin_asset_id: editable.origin_asset_id,
            category_id: editable.category_id,
            description: Some(editable.description),
        },
        auth_token,
    )
    .await?;

    let purchase_valuation =
        get_asset_pair_rates(infra, asset_id, currency_asset_id, "all", true, auth_token)
            .await?
            .into_iter()
            .find(|valuation| valuation.timestamp == old_date);
    match purchase_valuation {
        Some(valuation) if old_date != date => {
            edit_property_valuation(
                infra,
                accounts,
                asset_id,
                currency_asset_id,
                old_date,
                date,
                valuation.value,
                auth_token,
            )
            .await
        }
        _ => {
            refresh_property(infra, accounts, asset_id, auth_token).await;
            Ok(())
        }
    }
}

pub async fn delete_property_valuation(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    asset_id: i32,
    currency_asset_id: i32,
    date: i64,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let result = remove_valuation(infra, asset_id, currency_asset_id, date, auth_token).await;
    refresh_property(infra, module, asset_id, auth_token).await;
    result
}

async fn remove_valuation(
    infra: &SharedInfra,
    asset_id: i32,
    currency_asset_id: i32,
    date: i64,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    // Stored times keep microseconds while the API works in whole seconds.
    let path = format!(
        "/api/users/{user_id}/assets/{asset_id}/{currency_asset_id}/rates?start_timestamp={date}&end_timestamp={}",
        date + 1
    );
    let resp = infra.delete(&path, auth_token).await?;
    if resp.status >= 400 {
        return Err(server_error(resp.status, &resp.body));
    }
    Ok(())
}

pub async fn get_property_detail(
    infra: &SharedInfra,
    property_id: &str,
    auth_token: Option<&str>,
) -> Result<PropertyDetail, ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let mut accounts = accounts_with_properties(infra, &user_id, auth_token).await?;
    let holdings_resp = infra
        .get(
            &format!("/api/users/{user_id}/portfolio/holdings?apply_ownership_share=false"),
            auth_token,
        )
        .await?;
    let balances = extract_account_balances(&holdings_resp.body)
        .map_err(|reason| ApiError::Parse { reason })?;
    for account in accounts.iter_mut() {
        account.balance = Some(balances.get(&account.id).copied().unwrap_or(0.0));
    }
    fill_property_figures(&mut accounts);

    let account = accounts
        .iter()
        .find(|a| {
            a.property
                .as_ref()
                .is_some_and(|p| p.property_id == property_id)
        })
        .cloned()
        .ok_or_else(|| ApiError::Parse {
            reason: "property not found".into(),
        })?;
    let summary = account
        .property
        .clone()
        .expect("the account was found by its property");
    let loans = accounts
        .into_iter()
        .filter(|a| summary.loan_account_ids.contains(&a.id))
        .collect();

    Ok(PropertyDetail {
        account,
        summary,
        loans,
        base_ticker: infra.default_asset_ticker().unwrap_or_default(),
    })
}

pub async fn get_loan_candidates(
    infra: &SharedInfra,
    auth_token: Option<&str>,
) -> Result<Vec<AccountListItem>, ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let accounts = accounts_with_properties(infra, &user_id, auth_token).await?;
    Ok(accounts
        .into_iter()
        .filter(|a| {
            matches!(a.account_type_id, MORTGAGE | LOAN)
                && a.linked_property_id.is_none()
                && a.property.is_none()
        })
        .collect())
}

async fn accounts_with_properties(
    infra: &SharedInfra,
    user_id: &str,
    auth_token: Option<&str>,
) -> Result<Vec<AccountListItem>, ApiError> {
    let parse = |reason: String| ApiError::Parse { reason };
    let accounts_resp = infra
        .get(&format!("/api/users/{user_id}/accounts"), auth_token)
        .await?;
    let properties_resp = infra
        .get(&format!("/api/users/{user_id}/properties"), auth_token)
        .await?;
    let mut accounts = extract_accounts(&accounts_resp.body).map_err(parse)?;
    attach_properties(
        &mut accounts,
        extract_properties(&properties_resp.body).map_err(parse)?,
    );
    Ok(accounts)
}

pub async fn refresh_property(
    infra: &SharedInfra,
    module: &Mutex<AccountsModule>,
    asset_id: i32,
    auth_token: Option<&str>,
) {
    if let Some(user_id) = infra.user_id() {
        infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/assets/{asset_id}/"));
        infra.evict_memory_cache_prefix(&format!("/api/users/{user_id}/portfolio"));
    }
    refresh_accounts(infra, module, auth_token).await;
}

pub async fn get_property_value_chart(
    infra: &SharedInfra,
    asset_id: i32,
    currency_asset_id: i32,
    current_value: Option<f64>,
    auth_token: Option<&str>,
) -> Result<PropertyValueChart, ApiError> {
    // Converted rates come back bucketed by week, so the exact valuations are only used when no
    // conversion is needed.
    let valuations = match infra.default_asset_id() {
        Some(base) if base != currency_asset_id => {
            get_asset_pair_converted_rates(infra, asset_id, base, "all", true, auth_token).await?
        }
        _ => {
            get_asset_pair_rates(infra, asset_id, currency_asset_id, "all", true, auth_token)
                .await?
        }
    };
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    Ok(PropertyValueChart {
        line: value_trend(&valuations, now, current_value),
        valuations,
    })
}

async fn find_purchase_transaction(
    infra: &SharedInfra,
    account_id: &str,
    asset_id: i32,
    auth_token: Option<&str>,
) -> Result<TransactionListItem, ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let scope = TransactionListScope::Asset {
        asset_id,
        account_id: Some(account_id.to_string()),
    };
    let resp = infra
        .get(
            &scoped_transactions_page_path(&user_id, &scope, 100, 0),
            auth_token,
        )
        .await?;
    extract_account_transactions(&resp.body)
        .map_err(|reason| ApiError::Parse { reason })?
        .items
        .into_iter()
        .filter(|item| item.transaction_type == "asset_transfer_in")
        .min_by_key(|item| item.date)
        .ok_or_else(|| ApiError::Parse {
            reason: "the property's purchase transaction was not found".into(),
        })
}
