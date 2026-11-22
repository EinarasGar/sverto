use rust_decimal::Decimal;

pub struct UpdatePropertyDto {
    pub name: String,
    pub ownership_share: Decimal,
}
