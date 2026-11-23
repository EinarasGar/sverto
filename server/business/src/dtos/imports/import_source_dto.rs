use anyhow::Context;
use dal::models::import_models::{
    ImportDateOrder, ImportFileStatus, ImportSourceKind, ImportSourceModel,
};
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use super::import_file_dto::{ImportDateOrderDto, ImportFileStatusDto};
use crate::dtos::connectors::{
    ConnectorBindingDto, ProviderAccountBookedBalanceDto, ProviderAccountDto,
};

pub struct ImportSourceDto {
    pub id: Uuid,
    pub kind: ImportSourceKindDto,
    pub created_at: OffsetDateTime,
    pub recorded_transactions: i64,
}

pub enum ImportSourceKindDto {
    File(ImportSourceFileDto),
    BankAccount(ImportSourceBankAccountDto),
}

pub struct ImportSourceBankAccountDto {
    pub binding_id: Uuid,
    pub provider_kind: String,
    pub display_name: Option<String>,
    pub currency: Option<String>,
    pub account_number_last4: Option<String>,
}

pub struct ImportExternalLineDto {
    pub source_id: Uuid,
    pub external_id: String,
    pub transaction_id: Option<Uuid>,
}

pub struct ImportBankSourceDto {
    pub id: Uuid,
    pub binding_id: Option<Uuid>,
    pub cutoff_page_id: Option<Uuid>,
}

pub struct AddImportBankSourceDto {
    pub binding_id: Uuid,
    pub cutoff_page_id: Option<Uuid>,
    pub account_ref: String,
    pub account_id: Uuid,
    pub lines: Vec<ImportBankLineDto>,
    pub closing_balance: Option<ImportBankBalanceDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportBankLineDto {
    pub external_id: String,
    pub date: Date,
    pub description: Option<String>,
    pub amount: Decimal,
    pub currency: String,
}

pub struct ImportBankBalanceDto {
    pub amount: Decimal,
    pub currency: String,
    pub as_of_date: Date,
}

impl From<ProviderAccountBookedBalanceDto> for ImportBankBalanceDto {
    fn from(balance: ProviderAccountBookedBalanceDto) -> Self {
        Self {
            amount: balance.amount,
            currency: balance.currency,
            as_of_date: balance.captured_at.date(),
        }
    }
}

pub struct ImportSourceFileDto {
    pub file_id: Uuid,
    pub status: ImportFileStatusDto,
    pub error_message: Option<String>,
    pub paused_until: Option<OffsetDateTime>,
    pub date_order: Option<ImportDateOrderDto>,
}

impl ImportSourceDto {
    pub fn new(
        model: ImportSourceModel,
        recorded_transactions: i64,
        binding: Option<&ConnectorBindingDto>,
        provider_account: Option<&ProviderAccountDto>,
    ) -> anyhow::Result<Self> {
        let kind = match model.source_kind()? {
            ImportSourceKind::File => ImportSourceKindDto::File(ImportSourceFileDto {
                file_id: model.file_id.context("a file source has its file")?,
                status: ImportFileStatus::try_from(
                    model
                        .status
                        .as_deref()
                        .context("a file source has its file")?,
                )?
                .into(),
                error_message: model.error_message,
                paused_until: model.paused_until,
                date_order: model
                    .date_order
                    .as_deref()
                    .map(ImportDateOrder::try_from)
                    .transpose()?
                    .map(Into::into),
            }),
            ImportSourceKind::BankAccount => {
                ImportSourceKindDto::BankAccount(ImportSourceBankAccountDto {
                    binding_id: model
                        .binding_id
                        .context("a bank account source has its binding")?,
                    provider_kind: binding
                        .context("a bank account source has its binding")?
                        .provider_kind
                        .clone(),
                    display_name: provider_account.map(|account| account.display_name.clone()),
                    currency: provider_account.and_then(|account| account.currency.clone()),
                    account_number_last4: provider_account
                        .and_then(|account| account.account_number_last4.clone()),
                })
            }
        };
        Ok(Self {
            id: model.id,
            kind,
            created_at: model.created_at,
            recorded_transactions,
        })
    }
}
