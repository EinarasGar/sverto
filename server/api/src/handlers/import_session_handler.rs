use business::dtos::imports::import_ask_dto::ImportAnswerDto;
use business::dtos::imports::import_staging_dto::ImportVerbPinDto;
use business::dtos::transaction_dto::TransactionDto;
use itertools::Itertools;
use serde::Deserialize;
use uuid::Uuid;

use axum::{extract::Path, http::StatusCode, Json};

use crate::{
    auth::AuthenticatedUserId,
    converters::{
        transaction_dtos_to_account_ids_hashset, transaction_dtos_to_asset_ids_hashset,
        transaction_dtos_to_category_ids_hashset,
    },
    errors::ApiError,
    extractors::ValidatedJson,
    states::{
        AccountsServiceState, AssetsServiceState, CategoryServiceState, ImportApplyServiceState,
        ImportClassifyServiceState, ImportFileServiceState, ImportLinkServiceState,
        ImportSessionServiceState, ImportStagingServiceState,
    },
    view_models::{
        errors::{CreateResponses, DeleteResponses, GetResponses, UpdateResponses},
        imports::{
            answers::ImportAnswerViewModel,
            pending::IdentifiableImportPendingTransactionViewModel,
            requests::{
                ApplyImportSessionRequestViewModel, AttachImportFilesRequestViewModel,
                ReadImportFileDatesRequestViewModel, SetImportItemsVerbRequestViewModel,
            },
            session::{
                IdentifiableImportFileViewModel, IdentifiableImportSessionResponseViewModel,
                ImportSessionDetailResponseViewModel,
            },
            staged::ImportItemsResponseViewModel,
        },
        transactions::{
            base_models::metadata_lookup::MetadataLookupTables, validation::Validatable,
        },
    },
};

#[derive(Deserialize)]
pub(crate) struct ImportSessionIdPath {
    import_session_id: Uuid,
}

#[derive(Deserialize)]
pub(crate) struct ImportSessionFilePath {
    import_session_id: Uuid,
    file_id: Uuid,
}

#[derive(Deserialize)]
pub(crate) struct ImportSessionAskPath {
    import_session_id: Uuid,
    ask_key: String,
}

/// Create Import Session
///
/// Creates a new, empty import session. Accounts are bound later, as the
/// attached material is interpreted.
/// Refused with 403 while the user has AI features turned off: the import reads
/// and categorizes with AI.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions",
    tag = "Imports",
    responses(
        (status = 201, description = "Import session created.", body = IdentifiableImportSessionResponseViewModel),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id))]
pub async fn create_import_session(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportSessionServiceState(service): ImportSessionServiceState,
) -> Result<(StatusCode, Json<IdentifiableImportSessionResponseViewModel>), ApiError> {
    let session = service.create_session(user_id).await?;
    Ok((StatusCode::CREATED, Json(session.into())))
}

/// List Import Sessions
///
/// Lists all import sessions of the user.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/import-sessions",
    tag = "Imports",
    responses(
        (status = 200, description = "List of import sessions.", body = Vec<IdentifiableImportSessionResponseViewModel>),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id))]
pub async fn list_import_sessions(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportSessionServiceState(service): ImportSessionServiceState,
) -> Result<Json<Vec<IdentifiableImportSessionResponseViewModel>>, ApiError> {
    let sessions = service.get_sessions(user_id).await?;
    Ok(Json(sessions.into_iter().map(Into::into).collect()))
}

/// Get Import Session
///
/// Gets a single import session with its sources, open asks and verb counts.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}",
    tag = "Imports",
    responses(
        (status = 200, description = "Import session details.", body = ImportSessionDetailResponseViewModel),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn get_import_session(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportSessionServiceState(service): ImportSessionServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
) -> Result<Json<ImportSessionDetailResponseViewModel>, ApiError> {
    let session = service.get_session(import_session_id, user_id).await?;
    Ok(Json(session.into()))
}

/// Delete Import Session
///
/// Deletes an import session together with its attached import state.
#[utoipa::path(
    delete,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}",
    tag = "Imports",
    responses(
        (status = 204, description = "Import session deleted."),
        (status = 409, description = "A file was added while the import was being deleted; try again.", body = crate::errors::ApiErrorResponse),
        DeleteResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn delete_import_session(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportSessionServiceState(service): ImportSessionServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
) -> Result<StatusCode, ApiError> {
    service.delete_session(import_session_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Attach Files
///
/// Attaches previously uploaded files to the import session and starts
/// parsing them. Returns the attached files with their initial parse state.
/// Refused with 403 while the user has AI features turned off: the import reads
/// and categorizes with AI.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/files",
    tag = "Imports",
    request_body = AttachImportFilesRequestViewModel,
    responses(
        (status = 200, description = "Files attached.", body = Vec<IdentifiableImportFileViewModel>),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn attach_files(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportFileServiceState(service): ImportFileServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
    ValidatedJson(body): ValidatedJson<AttachImportFilesRequestViewModel>,
) -> Result<Json<Vec<IdentifiableImportFileViewModel>>, ApiError> {
    body.validate()?;
    let files = service
        .attach_files(import_session_id, user_id, body.file_ids)
        .await?;
    Ok(Json(files.into_iter().map(Into::into).collect()))
}

/// List Import Session Files
///
/// Lists the files attached to the import session with their parse state.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/files",
    tag = "Imports",
    responses(
        (status = 200, description = "Attached files with their parse state.", body = Vec<IdentifiableImportFileViewModel>),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn list_files(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportFileServiceState(service): ImportFileServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
) -> Result<Json<Vec<IdentifiableImportFileViewModel>>, ApiError> {
    let files = service.get_files(import_session_id, user_id).await?;
    Ok(Json(files.into_iter().map(Into::into).collect()))
}

/// Retry Import File
///
/// Re-enqueues a failed file for parsing.
/// Refused with 403 while the user has AI features turned off: the import reads
/// and categorizes with AI.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/files/{file_id}/retry",
    tag = "Imports",
    responses(
        (status = 204, description = "File re-enqueued for parsing."),
        (status = 404, description = "File is not attached to the session.", body = crate::errors::ApiErrorResponse),
        (status = 409, description = "File is not in a failed state, or the import is completed.", body = crate::errors::ApiErrorResponse),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
        ("file_id" = Uuid, Path, description = "The uploaded file to re-parse."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %path.import_session_id, file_id = %path.file_id))]
pub async fn retry_file(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportFileServiceState(service): ImportFileServiceState,
    Path(path): Path<ImportSessionFilePath>,
) -> Result<StatusCode, ApiError> {
    service
        .retry_file(path.import_session_id, path.file_id, user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Read Import File Dates
///
/// Reads a file again with its dates in the given order. Only for a file whose
/// dates all read both day-first and month-first with nothing in it settling
/// which (its `date_order` is set): the file is parsed again, so its rows get
/// new ids and pins on them are dropped.
/// Refused with 403 while the user has AI features turned off: the import reads
/// and categorizes with AI.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/files/{file_id}/date-order",
    tag = "Imports",
    request_body = ReadImportFileDatesRequestViewModel,
    responses(
        (status = 204, description = "File re-enqueued for parsing in that order."),
        (status = 404, description = "File is not attached to the session.", body = crate::errors::ApiErrorResponse),
        (status = 409, description = "The file's dates read only one way, it is already read in that order, or the import is completed.", body = crate::errors::ApiErrorResponse),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
        ("file_id" = Uuid, Path, description = "The uploaded file to read again."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %path.import_session_id, file_id = %path.file_id))]
pub async fn read_file_dates(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportFileServiceState(service): ImportFileServiceState,
    Path(path): Path<ImportSessionFilePath>,
    ValidatedJson(body): ValidatedJson<ReadImportFileDatesRequestViewModel>,
) -> Result<StatusCode, ApiError> {
    service
        .read_dates(
            path.import_session_id,
            path.file_id,
            user_id,
            body.date_order.into(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Retry Import Classification
///
/// Classifies the session's lines again after the last classification failed. Refused with 403
/// while the user has AI features turned off and the session has files: the import reads and
/// categorizes them with AI. A session of bank accounts alone is classified without AI, its lines
/// left uncategorized.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/classification/retry",
    tag = "Imports",
    responses(
        (status = 204, description = "Classification queued again."),
        (status = 404, description = "Import session not found.", body = crate::errors::ApiErrorResponse),
        (status = 409, description = "The last classification did not fail, or the session has no parsed files.", body = crate::errors::ApiErrorResponse),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn retry_classification(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportClassifyServiceState(service): ImportClassifyServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
) -> Result<StatusCode, ApiError> {
    service.retry(import_session_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Detach File
///
/// Removes a file from the import session along with everything extracted
/// from it. Safe during parsing: a parse that finishes later finds the file
/// detached and drops its result.
#[utoipa::path(
    delete,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/files/{file_id}",
    tag = "Imports",
    responses(
        (status = 204, description = "File detached and its extracted data removed."),
        (status = 404, description = "File is not attached to the session.", body = crate::errors::ApiErrorResponse),
        (status = 409, description = "The import is completed and can no longer be changed.", body = crate::errors::ApiErrorResponse),
        DeleteResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
        ("file_id" = Uuid, Path, description = "The attached file to remove."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %path.import_session_id, file_id = %path.file_id))]
pub async fn detach_file(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportFileServiceState(service): ImportFileServiceState,
    Path(path): Path<ImportSessionFilePath>,
) -> Result<StatusCode, ApiError> {
    service
        .detach_file(path.import_session_id, path.file_id, user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Answer Ask
///
/// Records the user's answer to an open ask. Asks are worked out from the
/// session on every read, so the next read reflects the answer. The answer is
/// tagged by the ask's `kind`, which must match the ask. An ask can be
/// answered once, except two that belong to the balance checks:
/// `no_checkpoint`, where entering a balance again replaces the one entered
/// before (its key is in the checks' `entered`), and `not_on_statement`, where
/// `delete` marks a ledger transaction the statement doesn't list for deletion
/// on apply and `keep` takes that back (its key is on the checks' `unlisted`).
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/asks/{ask_key}/answer",
    tag = "Imports",
    request_body = ImportAnswerViewModel,
    responses(
        (status = 204, description = "Answer recorded."),
        (status = 404, description = "Ask is not open or does not exist.", body = crate::errors::ApiErrorResponse),
        (status = 409, description = "Session cannot be mutated.", body = crate::errors::ApiErrorResponse),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
        ("ask_key" = String, Path, description = "Deterministic key of the ask being answered."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %path.import_session_id, ask_key = %path.ask_key))]
pub async fn answer_ask(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportStagingServiceState(service): ImportStagingServiceState,
    Path(path): Path<ImportSessionAskPath>,
    ValidatedJson(body): ValidatedJson<ImportAnswerViewModel>,
) -> Result<StatusCode, ApiError> {
    let answer = ImportAnswerDto::try_from(body).map_err(anyhow::Error::from)?;
    service
        .answer_ask(path.import_session_id, &path.ask_key, user_id, answer)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List Staged Items
///
/// Lists the session's import proposal: the ledger transactions derived from
/// its parsed lines, merged with the stored decisions (verbs, pins, targets),
/// and the line groups that could not become a transaction, in date and
/// document order. `ready` is false until the session's lines are classified,
/// including after a failed classification; the list is then empty. Rows
/// blocked on unresolved references are in the pending list instead.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/items",
    tag = "Imports",
    responses(
        (status = 200, description = "Every staged transaction of the session.", body = ImportItemsResponseViewModel),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn list_items(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
    ImportLinkServiceState(link_service): ImportLinkServiceState,
    ImportStagingServiceState(staging_service): ImportStagingServiceState,
    AssetsServiceState(asset_service): AssetsServiceState,
    AccountsServiceState(accounts_service): AccountsServiceState,
    CategoryServiceState(category_service): CategoryServiceState,
) -> Result<Json<ImportItemsResponseViewModel>, ApiError> {
    let links = link_service.links(import_session_id, user_id).await?;
    let Some(proposal) = staging_service
        .proposal(import_session_id, user_id, &links)
        .await?
    else {
        return Ok(Json(ImportItemsResponseViewModel {
            ready: false,
            items: Vec::new(),
            balance_checks: None,
            unstageable: Vec::new(),
            lookup_tables: MetadataLookupTables::default(),
            fingerprint: None,
            read_as_owed: Vec::new(),
        }));
    };

    let transactions: Vec<&TransactionDto> =
        proposal.rows.iter().map(|row| &row.transaction).collect();
    let mut account_ids = transaction_dtos_to_account_ids_hashset(&transactions);
    account_ids.extend(
        proposal
            .rows
            .iter()
            .flat_map(|row| row.near_miss.iter())
            .filter_map(|near| near.candidate.account_id),
    );
    let checks = proposal
        .balance_checks
        .iter()
        .flat_map(|checks| &checks.checks);
    account_ids.extend(checks.clone().map(|check| check.account_id));
    account_ids.extend(
        checks
            .clone()
            .flat_map(|check| &check.unlisted)
            .flat_map(|unlisted| unlisted.other_account_ids.iter().copied()),
    );
    account_ids.extend(proposal.read_as_owed.iter().copied());
    let mut asset_ids = transaction_dtos_to_asset_ids_hashset(&transactions);
    asset_ids.extend(checks.map(|check| check.asset_id));
    let (assets, accounts, categories) = tokio::try_join!(
        asset_service.get_assets(asset_ids),
        accounts_service.get_accounts(account_ids),
        category_service.get_categories(transaction_dtos_to_category_ids_hashset(&transactions)),
    )?;

    Ok(Json(ImportItemsResponseViewModel {
        ready: true,
        fingerprint: Some(proposal.fingerprint),
        items: proposal.rows.into_iter().map(Into::into).collect(),
        balance_checks: proposal.balance_checks.map(Into::into),
        unstageable: proposal.unstageable.into_iter().map(Into::into).collect(),
        read_as_owed: proposal.read_as_owed,
        lookup_tables: MetadataLookupTables {
            assets: assets.into_iter().map_into().collect(),
            accounts: accounts.into_iter().map_into().collect(),
            categories: categories.into_iter().map_into().collect(),
        },
    }))
}

/// List Pending Items
///
/// Lists parsed rows that cannot be staged yet because an asset, account,
/// or interpretation they depend on is unresolved. Each row names the asks
/// blocking it; answering those moves the rows to the staged list.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/pending",
    tag = "Imports",
    responses(
        (status = 200, description = "Every parsed row awaiting resolution, in date and document order.", body = Vec<IdentifiableImportPendingTransactionViewModel>),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn list_pending(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    ImportLinkServiceState(service): ImportLinkServiceState,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
) -> Result<Json<Vec<IdentifiableImportPendingTransactionViewModel>>, ApiError> {
    let lines = service.pending(import_session_id, user_id).await?;
    Ok(Json(lines.into_iter().map(Into::into).collect()))
}

/// Set Items Verb
///
/// Pins the given staged rows to `skip` or `add`, or releases them back to
/// the engine's derivation with `auto`. Pinned rows are excluded from
/// re-derivation. `apart` takes `joined_transfer` rows and pins each of the
/// two rows it was joined from to `add`. `skip` with the id of a candidate
/// that has `parts` says those ledger transactions together are the row's
/// payment: apply groups them, and `auto` takes the answer back.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/items/verb",
    tag = "Imports",
    request_body = SetImportItemsVerbRequestViewModel,
    responses(
        (status = 204, description = "Verbs pinned."),
        (status = 409, description = "The import isn't ready for review yet, or can no longer be changed.", body = crate::errors::ApiErrorResponse),
        UpdateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn set_items_verb(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
    ImportStagingServiceState(staging_service): ImportStagingServiceState,
    ValidatedJson(request): ValidatedJson<SetImportItemsVerbRequestViewModel>,
) -> Result<StatusCode, ApiError> {
    request.validate()?;
    let pin = ImportVerbPinDto::from(&request);
    staging_service
        .pin(import_session_id, user_id, request.item_ids, pin)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Apply Import Session
///
/// Writes the reviewed proposal to the ledger in one transaction: new transactions, corrections,
/// deletions, and a source record for every statement movement. The request carries the
/// `fingerprint` from the items the user reviewed; a proposal that changed since is refused.
/// Afterwards the session is completed and read-only.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/import-sessions/{import_session_id}/apply",
    tag = "Imports",
    request_body = ApplyImportSessionRequestViewModel,
    responses(
        (status = 204, description = "The import was written to the ledger."),
        (status = 409, description = "The import isn't ready, has open questions or possible duplicates, changed since it was reviewed, or was already applied.", body = crate::errors::ApiErrorResponse),
        UpdateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique identifier of the user."),
        ("import_session_id" = Uuid, Path, description = "Unique identifier of the import session."),
    ),
    security(("auth_token" = []))
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, import_session_id = %import_session_id))]
pub async fn apply_import_session(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(ImportSessionIdPath { import_session_id }): Path<ImportSessionIdPath>,
    ImportApplyServiceState(service): ImportApplyServiceState,
    ValidatedJson(request): ValidatedJson<ApplyImportSessionRequestViewModel>,
) -> Result<StatusCode, ApiError> {
    service
        .apply(import_session_id, user_id, request.fingerprint)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
