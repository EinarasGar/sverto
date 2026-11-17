#[cfg(feature = "backend")]
use business::dtos::imports::import_ask_dto::{
    AccountCandidateDto, AssetCandidateDto, ImportAskChoiceDto, ImportAskDto, ImportOpenAskDto,
};
use macros::type_tag;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssetCandidate {
    pub id: i32,
    pub ticker: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AccountCandidate {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportAskChoice {
    Map,
    Exclude,
    EnterBalance,
}

/// An ambiguity the engine could not resolve on its own.
/// Tagged by `kind`; each kind carries its own evidence and choice menu.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case", untagged)]
#[schema(discriminator = "kind")]
pub enum IdentifiableImportAskViewModel {
    AccountMapping(AccountMappingAsk),
    AssetMapping(AssetMappingAsk),
    NoCheckpoint(NoCheckpointAsk),
}

#[type_tag(value = "account_mapping", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AccountMappingAsk {
    pub ask_key: String,
    pub file_id: Uuid,
    pub raw_account: String,
    pub candidates: Vec<AccountCandidate>,
    pub choices: Vec<ImportAskChoice>,
    pub answer_covers_all_files: bool,
}

#[type_tag(value = "asset_mapping", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssetMappingAsk {
    pub ask_key: String,
    pub raw: String,
    pub candidates: Vec<AssetCandidate>,
    pub choices: Vec<ImportAskChoice>,
}

#[type_tag(value = "no_checkpoint", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NoCheckpointAsk {
    pub ask_key: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub as_of_date: OffsetDateTime,
    pub choices: Vec<ImportAskChoice>,
}

#[cfg(feature = "backend")]
impl From<ImportAskChoiceDto> for ImportAskChoice {
    fn from(value: ImportAskChoiceDto) -> Self {
        match value {
            ImportAskChoiceDto::Map => Self::Map,
            ImportAskChoiceDto::Exclude => Self::Exclude,
            ImportAskChoiceDto::EnterBalance => Self::EnterBalance,
        }
    }
}

#[cfg(feature = "backend")]
impl From<AccountCandidateDto> for AccountCandidate {
    fn from(value: AccountCandidateDto) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

#[cfg(feature = "backend")]
impl From<AssetCandidateDto> for AssetCandidate {
    fn from(value: AssetCandidateDto) -> Self {
        Self {
            id: value.id,
            ticker: value.ticker,
            name: value.name,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportOpenAskDto> for IdentifiableImportAskViewModel {
    fn from(value: ImportOpenAskDto) -> Self {
        match value.ask {
            ImportAskDto::AccountMapping(ask) => Self::AccountMapping(AccountMappingAsk {
                kind: Default::default(),
                ask_key: value.ask_key,
                file_id: ask.file_id,
                raw_account: ask.raw_account,
                candidates: ask.candidates.into_iter().map(Into::into).collect(),
                choices: ask.choices.into_iter().map(Into::into).collect(),
                answer_covers_all_files: ask.answer_covers_all_files,
            }),
            ImportAskDto::AssetMapping(ask) => Self::AssetMapping(AssetMappingAsk {
                kind: Default::default(),
                ask_key: value.ask_key,
                raw: ask.raw,
                candidates: ask.candidates.into_iter().map(Into::into).collect(),
                choices: ask.choices.into_iter().map(Into::into).collect(),
            }),
            ImportAskDto::NoCheckpoint(ask) => Self::NoCheckpoint(NoCheckpointAsk {
                kind: Default::default(),
                ask_key: value.ask_key,
                account_id: ask.account_id,
                asset_id: ask.asset_id,
                as_of_date: ask.as_of_date.midnight().assume_utc(),
                choices: vec![ImportAskChoice::EnterBalance],
            }),
        }
    }
}
