use rust_decimal::Decimal;
use sqlx::types::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyKind {
    RealEstate,
    Vehicle,
}

impl PropertyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RealEstate => "real_estate",
            Self::Vehicle => "vehicle",
        }
    }
}

impl TryFrom<&str> for PropertyKind {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "real_estate" => Ok(Self::RealEstate),
            "vehicle" => Ok(Self::Vehicle),
            other => Err(anyhow::anyhow!("Unknown property kind: {other}")),
        }
    }
}

impl From<PropertyKind> for sea_query::Value {
    fn from(value: PropertyKind) -> Self {
        sea_query::Value::String(Some(value.as_str().to_string()))
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct PropertyModel {
    pub id: Uuid,
    pub kind: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub account_name: String,
    pub ownership_share: Decimal,
    pub currency_asset_id: i32,
    pub loan_account_ids: Vec<Uuid>,
}

pub struct InsertPropertyModel {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub kind: PropertyKind,
}
