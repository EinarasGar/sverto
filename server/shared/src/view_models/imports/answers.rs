#[cfg(feature = "backend")]
use business::dtos::imports::import_ask_dto::{
    AccountMappingAnswerDto, AssetMappingAnswerDto, ImportAnswerDto, NoCheckpointAnswerDto,
    NotOnStatementAnswerDto,
};
#[cfg(feature = "backend")]
use business::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use macros::type_tag;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One answer per ask kind; the flattened `choice` enum carries any data the
/// chosen decision needs, so no field is ever optional. The kind must match
/// the `ask_key` prefix of the ask being answered.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case", untagged)]
#[schema(discriminator = "kind")]
pub enum ImportAnswerViewModel {
    AccountMapping(AccountMappingAnswer),
    AssetMapping(AssetMappingAnswer),
    NoCheckpoint(NoCheckpointAnswer),
    NotOnStatement(NotOnStatementAnswer),
}

#[type_tag(value = "account_mapping", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AccountMappingAnswer {
    #[serde(flatten)]
    pub choice: AccountMappingAnswerChoice,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum AccountMappingAnswerChoice {
    Map { account_id: Uuid },
    Exclude,
}

#[type_tag(value = "asset_mapping", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssetMappingAnswer {
    #[serde(flatten)]
    pub choice: AssetMappingAnswerChoice,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum AssetMappingAnswerChoice {
    Map { asset_id: i32 },
    Exclude,
}

#[type_tag(value = "no_checkpoint", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NoCheckpointAnswer {
    #[serde(flatten)]
    pub choice: NoCheckpointAnswerChoice,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum NoCheckpointAnswerChoice {
    EnterBalance { quantity: String },
}

/// For a ledger transaction a statement doesn't list (the balance checks' `unlisted`, by its
/// `ask_key`). `delete` removes it when the import is applied; `keep` takes that back.
#[type_tag(value = "not_on_statement", tag = "kind")]
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotOnStatementAnswer {
    #[serde(flatten)]
    pub choice: NotOnStatementAnswerChoice,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum NotOnStatementAnswerChoice {
    Delete,
    Keep,
}

#[cfg(feature = "backend")]
impl TryFrom<ImportAnswerViewModel> for ImportAnswerDto {
    type Error = BusinessValidationErrorDto;

    fn try_from(value: ImportAnswerViewModel) -> Result<Self, Self::Error> {
        match value {
            ImportAnswerViewModel::AccountMapping(answer) => {
                Ok(Self::AccountMapping(match answer.choice {
                    AccountMappingAnswerChoice::Map { account_id } => {
                        AccountMappingAnswerDto::Map { account_id }
                    }
                    AccountMappingAnswerChoice::Exclude => AccountMappingAnswerDto::Exclude,
                }))
            }
            ImportAnswerViewModel::AssetMapping(answer) => {
                Ok(Self::AssetMapping(match answer.choice {
                    AssetMappingAnswerChoice::Map { asset_id } => {
                        AssetMappingAnswerDto::Map { asset_id }
                    }
                    AssetMappingAnswerChoice::Exclude => AssetMappingAnswerDto::Exclude,
                }))
            }
            ImportAnswerViewModel::NoCheckpoint(answer) => match answer.choice {
                NoCheckpointAnswerChoice::EnterBalance { quantity } => quantity
                    .trim()
                    .parse()
                    .map(|quantity| {
                        Self::NoCheckpoint(NoCheckpointAnswerDto::EnterBalance { quantity })
                    })
                    .map_err(|_| BusinessValidationErrorDto {
                        errors: vec![BusinessFieldErrorDto {
                            field: "quantity".to_string(),
                            message: "Enter the balance as a number, such as 1234.56.".to_string(),
                        }],
                    }),
            },
            ImportAnswerViewModel::NotOnStatement(answer) => {
                Ok(Self::NotOnStatement(match answer.choice {
                    NotOnStatementAnswerChoice::Delete => NotOnStatementAnswerDto::Delete,
                    NotOnStatementAnswerChoice::Keep => NotOnStatementAnswerDto::Keep,
                }))
            }
        }
    }
}
