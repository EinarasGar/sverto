#[cfg(feature = "backend")]
use business::dtos::properties::{
    create_property_dto::{CreatePropertyDto, PropertyPurchaseDto},
    property_dto::PropertyKindDto,
};
use serde::{Deserialize, Serialize};
use time::{serde::timestamp, OffsetDateTime};

use super::base_models::property::PropertyKind;
use crate::view_models::accounts::base_models::account_id::RequiredAccountId;
use crate::view_models::accounts::base_models::account_name::AccountName;
use crate::view_models::accounts::base_models::ownership_share::OwnershipShare;
use crate::view_models::assets::base_models::asset_id::RequiredAssetId;
use crate::view_models::assets::base_models::positive_rate::PositiveRate;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreatePropertyRequestViewModel {
    pub kind: PropertyKind,
    #[schema(example = "Flat 12")]
    pub name: AccountName,
    #[schema(example = 45)]
    pub currency_asset_id: RequiredAssetId,
    pub ownership_share: OwnershipShare,
    #[schema(example = 328000)]
    pub value: PositiveRate,
    pub purchase: Option<PropertyPurchaseViewModel>,
    #[serde(default)]
    pub loan_account_ids: Vec<RequiredAccountId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PropertyPurchaseViewModel {
    #[serde(with = "timestamp")]
    #[schema(value_type = i32)]
    pub date: OffsetDateTime,
    #[schema(example = 296000)]
    pub price: PositiveRate,
}

#[cfg(feature = "backend")]
impl From<PropertyKind> for PropertyKindDto {
    fn from(kind: PropertyKind) -> Self {
        match kind {
            PropertyKind::RealEstate => PropertyKindDto::RealEstate,
            PropertyKind::Vehicle => PropertyKindDto::Vehicle,
        }
    }
}

#[cfg(feature = "backend")]
impl From<CreatePropertyRequestViewModel> for CreatePropertyDto {
    fn from(request: CreatePropertyRequestViewModel) -> Self {
        Self {
            kind: request.kind.into(),
            name: request.name.into_inner(),
            currency_asset_id: request.currency_asset_id.0,
            ownership_share: request.ownership_share.as_decimal(),
            value: request.value.as_decimal(),
            purchase: request.purchase.map(|purchase| PropertyPurchaseDto {
                date: purchase.date,
                price: purchase.price.as_decimal(),
            }),
            loan_account_ids: request
                .loan_account_ids
                .into_iter()
                .map(|id| id.0)
                .collect(),
        }
    }
}
