#[cfg(feature = "backend")]
use business::dtos::properties::property_dto::{PropertyDto, PropertyKindDto};
use serde::{Deserialize, Serialize};

use super::property_id::RequiredPropertyId;
use crate::view_models::accounts::base_models::account_id::RequiredAccountId;
use crate::view_models::accounts::base_models::account_name::AccountName;
use crate::view_models::accounts::base_models::ownership_share::OwnershipShare;
use crate::view_models::assets::base_models::asset_id::RequiredAssetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PropertyKind {
    RealEstate,
    Vehicle,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiablePropertyViewModel {
    pub property_id: RequiredPropertyId,
    pub kind: PropertyKind,
    #[schema(example = "Flat 12")]
    pub name: AccountName,
    pub account_id: RequiredAccountId,
    pub asset_id: RequiredAssetId,
    pub currency_asset_id: RequiredAssetId,
    pub ownership_share: OwnershipShare,
    pub loan_account_ids: Vec<RequiredAccountId>,
}

#[cfg(feature = "backend")]
impl From<PropertyKindDto> for PropertyKind {
    fn from(kind: PropertyKindDto) -> Self {
        match kind {
            PropertyKindDto::RealEstate => PropertyKind::RealEstate,
            PropertyKindDto::Vehicle => PropertyKind::Vehicle,
        }
    }
}

#[cfg(feature = "backend")]
impl From<PropertyDto> for IdentifiablePropertyViewModel {
    fn from(dto: PropertyDto) -> Self {
        Self {
            property_id: RequiredPropertyId(dto.id),
            kind: dto.kind.into(),
            name: AccountName::from_trusted(dto.name),
            account_id: RequiredAccountId(dto.account_id),
            asset_id: RequiredAssetId(dto.asset_id),
            currency_asset_id: RequiredAssetId(dto.currency_asset_id),
            ownership_share: OwnershipShare::from_trusted(dto.ownership_share),
            loan_account_ids: dto
                .loan_account_ids
                .into_iter()
                .map(RequiredAccountId)
                .collect(),
        }
    }
}
