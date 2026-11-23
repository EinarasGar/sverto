use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;

pub struct ImportPendingLegDto {
    pub asset_ref: String,
    pub quantity: Decimal,
    pub fee_type: Option<FeeEntryTypesDto>,
}

pub struct ImportPendingLineDto {
    pub id: Uuid,
    pub source_id: Uuid,
    pub account_ref: String,
    pub date: OffsetDateTime,
    pub description: Option<String>,
    pub legs: Vec<ImportPendingLegDto>,
    pub blocked_by: Vec<String>,
}
