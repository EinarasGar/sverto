use serde::{Deserialize, Serialize};

use crate::models::balance::{ProviderBookedBalance, ProviderCashBalance};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub provider_account_id: String,
    pub display_name: String,
    pub currency: Option<String>,
    pub account_type: Option<String>,
    pub account_number_last4: Option<String>,
    pub balance: Option<ProviderCashBalance>,
    #[serde(default)]
    pub booked_balance: Option<ProviderBookedBalance>,
    #[serde(default)]
    pub iban: Option<String>,
}
