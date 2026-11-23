use std::collections::HashMap;

use futures_util::future::try_join_all;
use shared::view_models::connectors::create_import::{
    ConnectorImportAccountViewModel, CreateConnectorImportRequestViewModel,
};
use shared::view_models::imports::asks::IdentifiableImportAskViewModel;
use shared::view_models::imports::requests::{
    ApplyImportSessionRequestViewModel, AttachImportFilesRequestViewModel,
    ReadImportFileDatesRequestViewModel, SetImportItemsVerbRequestViewModel,
};
use shared::view_models::imports::session::{
    IdentifiableImportSourceViewModel, ImportSessionDetailResponseViewModel,
};
use uuid::Uuid;

use super::accounts::account_names;
use super::infra::SharedInfra;
use super::{assets, files, ok_or_err, transactions, user_id};
use crate::api::connectors::{extract_import_session_id, import_from_to_vm};
use crate::api::imports::*;
use crate::error::ApiError;
use crate::models::{
    ApiResponse, ConnectorImportAccount, ConnectorImportFrom, ImportAnswer, ImportDateOrder,
    ImportItemVerb, ImportItems, ImportPendingRow, ImportSessionDetail, ImportSessionSummary,
};

fn sessions_path(uid: &str) -> String {
    format!("/api/users/{uid}/import-sessions")
}

fn evict_connectors(infra: &SharedInfra, uid: &str) {
    infra.evict_memory_cache_prefix(&format!("/api/users/{uid}/connectors"));
}

fn check(infra: &SharedInfra, resp: &ApiResponse) -> Result<(), ApiError> {
    let result = ok_or_err(resp.status, &resp.body);
    if let Err(ApiError::AiFeaturesDisabled) = result {
        infra.set_ai_features_enabled(false);
    }
    result
}

fn parse_id(id: &str) -> Result<Uuid, ApiError> {
    id.parse().map_err(|_| ApiError::Parse {
        reason: format!("invalid id '{id}'"),
    })
}

fn to_json(body: &impl serde::Serialize) -> Result<String, ApiError> {
    serde_json::to_string(body).map_err(|e| ApiError::Parse {
        reason: e.to_string(),
    })
}

async fn post_to_session(
    infra: &SharedInfra,
    session_id: &str,
    suffix: &str,
    body: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let uid = user_id(infra)?;
    let base = sessions_path(&uid);
    let resp = infra
        .post(&format!("{base}/{session_id}{suffix}"), body, auth_token)
        .await?;
    check(infra, &resp)?;
    infra.evict_memory_cache_prefix(&base);
    Ok(())
}

pub async fn list_sessions(
    infra: &SharedInfra,
    auth_token: Option<&str>,
) -> Result<Vec<ImportSessionSummary>, ApiError> {
    let uid = user_id(infra)?;
    let path = sessions_path(&uid);
    infra.evict_memory_cache(&path);
    let resp = infra.get(&path, auth_token).await?;
    let names = account_names(infra, auth_token).await?;
    extract_sessions(&resp.body, &names).map_err(|e| ApiError::Parse { reason: e })
}

pub async fn create_session(
    infra: &SharedInfra,
    auth_token: Option<&str>,
) -> Result<String, ApiError> {
    let uid = user_id(infra)?;
    let path = sessions_path(&uid);
    let resp = infra.post(&path, "", auth_token).await?;
    check(infra, &resp)?;
    infra.evict_memory_cache_prefix(&path);
    extract_created_session_id(&resp.body).map_err(|e| ApiError::Parse { reason: e })
}

pub async fn create_connector_import(
    infra: &SharedInfra,
    connection_id: &str,
    accounts: Vec<ConnectorImportAccount>,
    import_from: ConnectorImportFrom,
    auth_token: Option<&str>,
) -> Result<String, ApiError> {
    let uid = user_id(infra)?;
    let body = to_json(&CreateConnectorImportRequestViewModel {
        accounts: accounts
            .into_iter()
            .map(|account| {
                Ok(ConnectorImportAccountViewModel {
                    provider_account_id: account.provider_account_id,
                    sverto_account_id: parse_id(&account.sverto_account_id)?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
        import_from: import_from_to_vm(import_from).map_err(|e| ApiError::Parse { reason: e })?,
    })?;
    let resp = infra
        .post(
            &format!("/api/users/{uid}/connectors/connections/{connection_id}/imports"),
            &body,
            auth_token,
        )
        .await?;
    check(infra, &resp)?;
    infra.evict_memory_cache_prefix(&sessions_path(&uid));
    evict_connectors(infra, &uid);
    extract_import_session_id(&resp.body).map_err(|e| ApiError::Parse { reason: e })
}

pub async fn get_session(
    infra: &SharedInfra,
    session_id: &str,
    auth_token: Option<&str>,
) -> Result<ImportSessionDetail, ApiError> {
    let uid = user_id(infra)?;
    let path = format!("{}/{session_id}", sessions_path(&uid));
    infra.evict_memory_cache(&path);
    let resp = infra.get(&path, auth_token).await?;
    let vm: ImportSessionDetailResponseViewModel =
        serde_json::from_str(&resp.body).map_err(|e| ApiError::Parse {
            reason: e.to_string(),
        })?;

    let names = account_names(infra, auth_token).await?;
    let file_names: HashMap<Uuid, String> = try_join_all(
        vm.sources
            .iter()
            .filter_map(|source| match source {
                IdentifiableImportSourceViewModel::File(f) => Some(f.file_id),
                IdentifiableImportSourceViewModel::BankAccount(_) => None,
            })
            .map(|file_id| async move {
                files::file_name(infra, &file_id.to_string(), auth_token)
                    .await
                    .map(|name| (file_id, name))
            }),
    )
    .await?
    .into_iter()
    .collect();
    let mut asset_ids: Vec<i32> = vm
        .open_asks
        .iter()
        .filter_map(|ask| match ask {
            IdentifiableImportAskViewModel::NoCheckpoint(a) => Some(a.asset_id),
            _ => None,
        })
        .collect();
    asset_ids.sort_unstable();
    asset_ids.dedup();
    let tickers: HashMap<i32, String> = try_join_all(asset_ids.into_iter().map(|id| async move {
        assets::get_asset_detail(infra, id, false, auth_token)
            .await
            .map(|asset| (id, asset.ticker))
    }))
    .await?
    .into_iter()
    .collect();

    Ok(session_detail(
        session_id,
        vm,
        &names,
        &file_names,
        &tickers,
    ))
}

pub async fn delete_session(
    infra: &SharedInfra,
    session_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let uid = user_id(infra)?;
    let base = sessions_path(&uid);
    let resp = infra
        .delete(&format!("{base}/{session_id}"), auth_token)
        .await?;
    check(infra, &resp)?;
    infra.evict_memory_cache_prefix(&base);
    evict_connectors(infra, &uid);
    Ok(())
}

pub async fn attach_files(
    infra: &SharedInfra,
    session_id: &str,
    file_ids: &[String],
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let body = to_json(&AttachImportFilesRequestViewModel {
        file_ids: file_ids
            .iter()
            .map(|id| parse_id(id))
            .collect::<Result<_, _>>()?,
    })?;
    post_to_session(infra, session_id, "/files", &body, auth_token).await
}

pub async fn detach_file(
    infra: &SharedInfra,
    session_id: &str,
    file_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let uid = user_id(infra)?;
    let base = sessions_path(&uid);
    let resp = infra
        .delete(&format!("{base}/{session_id}/files/{file_id}"), auth_token)
        .await?;
    check(infra, &resp)?;
    infra.evict_memory_cache_prefix(&base);
    Ok(())
}

pub async fn retry_file(
    infra: &SharedInfra,
    session_id: &str,
    file_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    post_to_session(
        infra,
        session_id,
        &format!("/files/{file_id}/retry"),
        "",
        auth_token,
    )
    .await
}

pub async fn set_file_date_order(
    infra: &SharedInfra,
    session_id: &str,
    file_id: &str,
    date_order: ImportDateOrder,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let body = to_json(&ReadImportFileDatesRequestViewModel {
        date_order: date_order_to_vm(date_order),
    })?;
    post_to_session(
        infra,
        session_id,
        &format!("/files/{file_id}/date-order"),
        &body,
        auth_token,
    )
    .await
}

pub async fn retry_classification(
    infra: &SharedInfra,
    session_id: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    post_to_session(infra, session_id, "/classification/retry", "", auth_token).await
}

pub async fn answer_ask(
    infra: &SharedInfra,
    session_id: &str,
    ask_key: &str,
    answer: ImportAnswer,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let body = to_json(&answer_to_vm(answer).map_err(|e| ApiError::Parse { reason: e })?)?;
    post_to_session(
        infra,
        session_id,
        &format!("/asks/{}/answer", urlencoding::encode(ask_key)),
        &body,
        auth_token,
    )
    .await
}

pub async fn list_items(
    infra: &SharedInfra,
    session_id: &str,
    auth_token: Option<&str>,
) -> Result<ImportItems, ApiError> {
    let uid = user_id(infra)?;
    let path = format!("{}/{session_id}/items", sessions_path(&uid));
    infra.evict_memory_cache(&path);
    let resp = infra.get(&path, auth_token).await?;
    extract_items(&resp.body).map_err(|e| ApiError::Parse { reason: e })
}

pub async fn list_pending(
    infra: &SharedInfra,
    session_id: &str,
    auth_token: Option<&str>,
) -> Result<Vec<ImportPendingRow>, ApiError> {
    let uid = user_id(infra)?;
    let path = format!("{}/{session_id}/pending", sessions_path(&uid));
    infra.evict_memory_cache(&path);
    let resp = infra.get(&path, auth_token).await?;
    extract_pending(&resp.body).map_err(|e| ApiError::Parse { reason: e })
}

pub async fn set_items_verb(
    infra: &SharedInfra,
    session_id: &str,
    item_ids: &[String],
    verb: ImportItemVerb,
    candidate_id: Option<String>,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let body = to_json(&SetImportItemsVerbRequestViewModel {
        item_ids: item_ids
            .iter()
            .map(|id| parse_id(id))
            .collect::<Result<_, _>>()?,
        verb: item_verb_to_vm(verb),
        candidate_id: candidate_id.as_deref().map(parse_id).transpose()?,
    })?;
    post_to_session(infra, session_id, "/items/verb", &body, auth_token).await
}

pub async fn apply_session(
    infra: &SharedInfra,
    session_id: &str,
    fingerprint: &str,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let body = to_json(&ApplyImportSessionRequestViewModel {
        fingerprint: parse_id(fingerprint)?,
    })?;
    post_to_session(infra, session_id, "/apply", &body, auth_token).await?;
    let uid = user_id(infra)?;
    transactions::evict_transaction_caches(infra, &uid);
    evict_connectors(infra, &uid);
    Ok(())
}
