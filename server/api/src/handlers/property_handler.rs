use axum::{extract::Path, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthenticatedUserId,
    errors::ApiError,
    extractors::ValidatedJson,
    states::PropertyServiceState,
    view_models::errors::{CreateResponses, DeleteResponses, GetResponses, UpdateResponses},
    view_models::properties::{
        base_models::property::IdentifiablePropertyViewModel,
        create_property::CreatePropertyRequestViewModel,
        get_properties::GetPropertiesResponseViewModel, update_property::UpdatePropertyViewModel,
    },
    view_models::transactions::validation::Validatable,
};

#[derive(Deserialize)]
pub(crate) struct PropertyIdPath {
    property_id: Uuid,
}

#[derive(Deserialize)]
pub(crate) struct PropertyLoanPath {
    property_id: Uuid,
    account_id: Uuid,
}

/// Get Properties
///
/// Gets the user's properties (real estate and vehicles) with their linked loan accounts.
#[utoipa::path(
    get,
    path = "/api/users/{user_id}/properties",
    tag = "Properties",
    responses(
        (status = 200, description = "Properties retrieved successfully.", body = GetPropertiesResponseViewModel),
        GetResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique Identifier of the user."),
    ),
    security(
        ("auth_token" = [])
    )
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id))]
pub async fn get_properties(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    PropertyServiceState(property_service): PropertyServiceState,
) -> Result<Json<GetPropertiesResponseViewModel>, ApiError> {
    let properties = property_service.get_properties(user_id).await?;
    Ok(GetPropertiesResponseViewModel {
        properties: properties.into_iter().map(Into::into).collect(),
    }
    .into())
}

/// Create Property
///
/// Creates a property: its hidden asset priced at the given value, an account holding it,
/// the transfer that brings it in, and links to existing loan accounts.
#[utoipa::path(
    post,
    path = "/api/users/{user_id}/properties",
    tag = "Properties",
    responses(
        (status = 201, description = "Property created successfully.", body = IdentifiablePropertyViewModel),
        CreateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique Identifier of the user."),
    ),
    request_body (
        content = CreatePropertyRequestViewModel,
    ),
    security(
        ("auth_token" = [])
    )
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id))]
pub async fn create_property(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    PropertyServiceState(property_service): PropertyServiceState,
    ValidatedJson(body): ValidatedJson<CreatePropertyRequestViewModel>,
) -> Result<Json<IdentifiablePropertyViewModel>, ApiError> {
    body.validate()?;

    let property = property_service
        .create_property(user_id, body.into())
        .await
        .map_err(ApiError::from_anyhow)?;

    Ok(IdentifiablePropertyViewModel::from(property).into())
}

/// Update Property
///
/// Renames a property (its account and asset together) and changes the user's ownership share.
#[utoipa::path(
    put,
    path = "/api/users/{user_id}/properties/{property_id}",
    tag = "Properties",
    responses(
        (status = 200, description = "Property updated successfully.", body = UpdatePropertyViewModel),
        UpdateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique Identifier of the user."),
        ("property_id" = Uuid, Path, description = "Id of the property to update."),
    ),
    request_body (
        content = UpdatePropertyViewModel,
    ),
    security(
        ("auth_token" = [])
    )
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, property_id = %property_id))]
pub async fn update_property(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(PropertyIdPath { property_id }): Path<PropertyIdPath>,
    PropertyServiceState(property_service): PropertyServiceState,
    ValidatedJson(body): ValidatedJson<UpdatePropertyViewModel>,
) -> Result<Json<UpdatePropertyViewModel>, ApiError> {
    property_service
        .update_property(user_id, property_id, body.clone().into())
        .await
        .map_err(ApiError::from_anyhow)?;

    Ok(body.into())
}

/// Link Property Loan
///
/// Links one of the user's loan accounts to a property. A loan finances at most one property.
#[utoipa::path(
    put,
    path = "/api/users/{user_id}/properties/{property_id}/loans/{account_id}",
    tag = "Properties",
    responses(
        (status = 200, description = "Loan linked successfully."),
        UpdateResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique Identifier of the user."),
        ("property_id" = Uuid, Path, description = "Id of the property."),
        ("account_id" = Uuid, Path, description = "Id of the loan account to link."),
    ),
    security(
        ("auth_token" = [])
    )
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, property_id = %property_id, account_id = %account_id))]
pub async fn link_property_loan(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(PropertyLoanPath {
        property_id,
        account_id,
    }): Path<PropertyLoanPath>,
    PropertyServiceState(property_service): PropertyServiceState,
) -> Result<(), ApiError> {
    property_service
        .link_loan(user_id, property_id, account_id)
        .await
        .map_err(ApiError::from_anyhow)
}

/// Unlink Property Loan
///
/// Unlinks a loan account from a property. The account itself is unchanged.
#[utoipa::path(
    delete,
    path = "/api/users/{user_id}/properties/{property_id}/loans/{account_id}",
    tag = "Properties",
    responses(
        (status = 200, description = "Loan unlinked successfully."),
        DeleteResponses
    ),
    params(
        ("user_id" = Uuid, Path, description = "Unique Identifier of the user."),
        ("property_id" = Uuid, Path, description = "Id of the property."),
        ("account_id" = Uuid, Path, description = "Id of the loan account to unlink."),
    ),
    security(
        ("auth_token" = [])
    )
)]
#[tracing::instrument(level = "info", skip_all, fields(user_id = %user_id, property_id = %property_id, account_id = %account_id))]
pub async fn unlink_property_loan(
    AuthenticatedUserId(user_id): AuthenticatedUserId,
    Path(PropertyLoanPath {
        property_id,
        account_id,
    }): Path<PropertyLoanPath>,
    PropertyServiceState(property_service): PropertyServiceState,
) -> Result<(), ApiError> {
    property_service
        .unlink_loan(user_id, property_id, account_id)
        .await
        .map_err(ApiError::from_anyhow)
}
