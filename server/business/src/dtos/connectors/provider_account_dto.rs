use connectors::models::balance::{ProviderBookedBalance, ProviderCashBalance};
use connectors::models::ProviderAccount;
use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct ProviderAccountDto {
    pub provider_account_id: String,
    pub display_name: String,
    pub currency: Option<String>,
    pub account_type: Option<String>,
    pub account_number_last4: Option<String>,
    pub balance: Option<ProviderAccountBalanceDto>,
    pub booked_balance: Option<ProviderAccountBookedBalanceDto>,
    pub iban: Option<String>,
    pub history: ProviderAccountHistoryDto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAccountHistoryDto {
    NotFetched,
    Fetching,
    Fetched,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ProviderAccountBalanceDto {
    pub amount: Decimal,
    pub currency: String,
}

#[derive(Clone, Debug)]
pub struct ProviderAccountBookedBalanceDto {
    pub amount: Decimal,
    pub currency: String,
    pub captured_at: OffsetDateTime,
}

impl From<(ProviderAccount, ProviderAccountHistoryDto)> for ProviderAccountDto {
    fn from((account, history): (ProviderAccount, ProviderAccountHistoryDto)) -> Self {
        Self {
            provider_account_id: account.provider_account_id,
            display_name: account.display_name,
            currency: account.currency,
            account_type: account.account_type,
            account_number_last4: account.account_number_last4,
            balance: account.balance.map(Into::into),
            booked_balance: account.booked_balance.map(Into::into),
            iban: account.iban,
            history,
        }
    }
}

impl From<ProviderCashBalance> for ProviderAccountBalanceDto {
    fn from(balance: ProviderCashBalance) -> Self {
        Self {
            amount: balance.amount,
            currency: balance.currency,
        }
    }
}

impl From<ProviderBookedBalance> for ProviderAccountBookedBalanceDto {
    fn from(balance: ProviderBookedBalance) -> Self {
        Self {
            amount: balance.amount,
            currency: balance.currency,
            captured_at: balance.captured_at,
        }
    }
}
