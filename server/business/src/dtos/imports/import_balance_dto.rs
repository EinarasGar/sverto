use dal::models::import_models::{ImportAssertionKind, ImportAssertionModel};
use rust_decimal::Decimal;
use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAssertionKindDto {
    Opening,
    Closing,
    Snapshot,
    ValueTotal,
}

impl From<ImportAssertionKind> for ImportAssertionKindDto {
    fn from(value: ImportAssertionKind) -> Self {
        match value {
            ImportAssertionKind::Opening => Self::Opening,
            ImportAssertionKind::Closing => Self::Closing,
            ImportAssertionKind::Snapshot => Self::Snapshot,
            ImportAssertionKind::ValueTotal => Self::ValueTotal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportAssertionDto {
    pub source_id: Uuid,
    pub account_ref: String,
    pub asset_ref: Option<String>,
    pub kind: ImportAssertionKindDto,
    pub date: Date,
    pub quantity: Decimal,
}

impl TryFrom<ImportAssertionModel> for ImportAssertionDto {
    type Error = anyhow::Error;

    fn try_from(model: ImportAssertionModel) -> Result<Self, Self::Error> {
        let (Some(source_id), Some(account_ref)) = (model.source_id, model.account_ref) else {
            anyhow::bail!("a printed balance has a source and an account name");
        };
        Ok(Self {
            source_id,
            account_ref,
            asset_ref: model.asset_ref,
            kind: ImportAssertionKind::try_from(model.kind.as_str())?.into(),
            date: model.as_of_date,
            quantity: model.quantity,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportEnteredBalanceDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub date: Date,
    pub quantity: Decimal,
}

impl TryFrom<ImportAssertionModel> for ImportEnteredBalanceDto {
    type Error = anyhow::Error;

    fn try_from(model: ImportAssertionModel) -> Result<Self, Self::Error> {
        let (Some(account_id), Some(asset_id)) = (model.account_id, model.asset_id) else {
            anyhow::bail!("an entered balance names its account and asset");
        };
        Ok(Self {
            account_id,
            asset_id,
            date: model.as_of_date,
            quantity: model.quantity,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportEnterableBalanceDto {
    pub ask_key: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub as_of_date: Date,
    pub entered: Option<ImportEnteredBalanceDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportBalanceChangeDto {
    pub after: Date,
    pub through: Date,
    pub change: Decimal,
    pub off_by: Decimal,
    pub explained: bool,
    pub awaits_answer: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportUnlistedMovementDto {
    pub transaction_id: Uuid,
    pub date: Date,
    pub quantity: Decimal,
    pub description: Option<String>,
    pub may_book_later: bool,
    pub explains_difference: bool,
    pub ask_key: Option<String>,
    pub deleting: bool,
    pub other_account_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportBalanceCheckDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub first_date: Date,
    pub last_date: Date,
    pub balances_checked: usize,
    pub starting_off_by: Decimal,
    pub final_off_by: Decimal,
    pub changes: Vec<ImportBalanceChangeDto>,
    pub unlisted: Vec<ImportUnlistedMovementDto>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportStartingBalanceDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub source_id: Uuid,
    pub date: Date,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportAdjustmentDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub date: Date,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportBalanceChecksDto {
    pub checks: Vec<ImportBalanceCheckDto>,
    pub unchecked: usize,
    pub enterable: Vec<ImportEnterableBalanceDto>,
}
