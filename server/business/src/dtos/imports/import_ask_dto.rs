use rust_decimal::Decimal;
use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAskChoiceDto {
    Map,
    Exclude,
    EnterBalance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountCandidateDto {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCandidateDto {
    pub id: i32,
    pub ticker: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountMappingAskDto {
    pub file_id: Uuid,
    pub raw_account: String,
    pub candidates: Vec<AccountCandidateDto>,
    pub choices: Vec<ImportAskChoiceDto>,
    pub answer_covers_all_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetMappingAskDto {
    pub raw: String,
    pub candidates: Vec<AssetCandidateDto>,
    pub choices: Vec<ImportAskChoiceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoCheckpointAskDto {
    pub account_id: Uuid,
    pub asset_id: i32,
    pub as_of_date: Date,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportAskDto {
    AccountMapping(AccountMappingAskDto),
    AssetMapping(AssetMappingAskDto),
    NoCheckpoint(NoCheckpointAskDto),
}

pub struct ImportOpenAskDto {
    pub ask_key: String,
    pub ask: ImportAskDto,
}

#[derive(Debug, Clone)]
pub enum AccountMappingAnswerDto {
    Map { account_id: Uuid },
    Exclude,
}

#[derive(Debug, Clone)]
pub enum AssetMappingAnswerDto {
    Map { asset_id: i32 },
    Exclude,
}

#[derive(Debug, Clone)]
pub enum NoCheckpointAnswerDto {
    EnterBalance { quantity: Decimal },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotOnStatementAnswerDto {
    Delete,
    Keep,
}

#[derive(Debug, Clone)]
pub enum ImportAnswerDto {
    AccountMapping(AccountMappingAnswerDto),
    AssetMapping(AssetMappingAnswerDto),
    NoCheckpoint(NoCheckpointAnswerDto),
    NotOnStatement(NotOnStatementAnswerDto),
}
