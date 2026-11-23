use serde::{Deserialize, Serialize};

use crate::models::account::ProviderAccount;

#[derive(Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: String,
    pub accounts: Vec<ProviderAccount>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_stored_before_booked_balances_still_reads() {
        let session: StoredSession = serde_json::from_value(serde_json::json!({
            "session_id": "s-1",
            "accounts": [{
                "provider_account_id": "acc-1",
                "display_name": "Main",
                "currency": "EUR",
                "account_type": "CACC",
                "account_number_last4": "1234",
                "balance": { "currency": "EUR", "amount": "159.21" }
            }]
        }))
        .unwrap();
        assert_eq!(session.accounts[0].booked_balance, None);
        assert_eq!(session.accounts[0].iban, None);
        assert!(session.accounts[0].balance.is_some());
    }
}
