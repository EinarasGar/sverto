use dal::enums::asset_types::DatabaseAssetTypes;
use dal::models::property_models::{PropertyKind, PropertyModel};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKindDto {
    RealEstate,
    Vehicle,
}

impl PropertyKindDto {
    pub fn account_type_id(&self) -> i32 {
        match self {
            Self::RealEstate => 9,
            Self::Vehicle => 12,
        }
    }

    pub fn asset_type(&self) -> DatabaseAssetTypes {
        match self {
            Self::RealEstate => DatabaseAssetTypes::RealEstate,
            Self::Vehicle => DatabaseAssetTypes::Vehicle,
        }
    }

    pub fn ticker_prefix(&self) -> &'static str {
        match self {
            Self::RealEstate => "HOME",
            Self::Vehicle => "CAR",
        }
    }
}

impl From<PropertyKindDto> for PropertyKind {
    fn from(kind: PropertyKindDto) -> Self {
        match kind {
            PropertyKindDto::RealEstate => PropertyKind::RealEstate,
            PropertyKindDto::Vehicle => PropertyKind::Vehicle,
        }
    }
}

impl From<PropertyKind> for PropertyKindDto {
    fn from(kind: PropertyKind) -> Self {
        match kind {
            PropertyKind::RealEstate => PropertyKindDto::RealEstate,
            PropertyKind::Vehicle => PropertyKindDto::Vehicle,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PropertyDto {
    pub id: Uuid,
    pub kind: PropertyKindDto,
    pub name: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub currency_asset_id: i32,
    pub ownership_share: Decimal,
    pub loan_account_ids: Vec<Uuid>,
}

impl TryFrom<PropertyModel> for PropertyDto {
    type Error = anyhow::Error;

    fn try_from(model: PropertyModel) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            kind: PropertyKind::try_from(model.kind.as_str())?.into(),
            name: model.account_name,
            account_id: model.account_id,
            asset_id: model.asset_id,
            currency_asset_id: model.currency_asset_id,
            ownership_share: model.ownership_share,
            loan_account_ids: model.loan_account_ids,
        })
    }
}
