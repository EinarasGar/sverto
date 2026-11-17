use strum::EnumCount;

use crate::models::import_models::ImportFeeType;

#[derive(sqlx::Type, Clone, Debug, PartialEq, Eq, Hash, EnumCount)]
#[repr(i32)]
pub enum DatabaseFeeCategories {
    Transaction = 1,
    Exchange = 2,
    WithholdingTax = 3,
}

impl DatabaseFeeCategories {
    pub fn len() -> usize {
        DatabaseFeeCategories::COUNT
    }
}

impl From<ImportFeeType> for DatabaseFeeCategories {
    fn from(value: ImportFeeType) -> Self {
        match value {
            ImportFeeType::Transaction => Self::Transaction,
            ImportFeeType::Exchange => Self::Exchange,
            ImportFeeType::WithholdingTax => Self::WithholdingTax,
        }
    }
}
