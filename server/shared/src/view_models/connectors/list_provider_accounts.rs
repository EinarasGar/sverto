#[cfg(feature = "backend")]
use business::dtos::connectors::{
    ProviderAccountBalanceDto, ProviderAccountDto, ProviderAccountHistoryDto,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProviderAccountViewModel {
    pub provider_account_id: String,
    pub display_name: String,
    pub currency: Option<String>,
    pub account_type: Option<String>,
    pub account_number_last4: Option<String>,
    pub balance: Option<ProviderAccountBalanceViewModel>,
    pub history: ProviderAccountHistory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAccountHistory {
    NotFetched,
    Fetching,
    Fetched,
    Failed,
}

#[cfg(feature = "backend")]
impl From<ProviderAccountHistoryDto> for ProviderAccountHistory {
    fn from(dto: ProviderAccountHistoryDto) -> Self {
        match dto {
            ProviderAccountHistoryDto::NotFetched => Self::NotFetched,
            ProviderAccountHistoryDto::Fetching => Self::Fetching,
            ProviderAccountHistoryDto::Fetched => Self::Fetched,
            ProviderAccountHistoryDto::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ProviderAccountBalanceViewModel {
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    #[schema(value_type = f64)]
    pub amount: Decimal,
    pub currency: String,
}

#[cfg(feature = "backend")]
impl From<ProviderAccountDto> for ProviderAccountViewModel {
    fn from(dto: ProviderAccountDto) -> Self {
        Self {
            provider_account_id: dto.provider_account_id,
            display_name: dto.display_name,
            currency: dto.currency,
            account_type: dto.account_type,
            account_number_last4: dto.account_number_last4,
            balance: dto.balance.map(Into::into),
            history: dto.history.into(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ProviderAccountBalanceDto> for ProviderAccountBalanceViewModel {
    fn from(dto: ProviderAccountBalanceDto) -> Self {
        Self {
            amount: dto.amount,
            currency: dto.currency,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ListProviderAccountsResponseViewModel {
    pub accounts: Vec<ProviderAccountViewModel>,
}
