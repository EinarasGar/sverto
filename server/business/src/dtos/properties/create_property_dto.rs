use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use super::property_dto::PropertyKindDto;

pub struct CreatePropertyDto {
    pub kind: PropertyKindDto,
    pub name: String,
    pub currency_asset_id: i32,
    pub ownership_share: Decimal,
    pub value: Decimal,
    pub purchase: Option<PropertyPurchaseDto>,
    pub loan_account_ids: Vec<Uuid>,
}

pub struct PropertyPurchaseDto {
    pub date: OffsetDateTime,
    pub price: Decimal,
}
