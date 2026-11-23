use std::collections::HashSet;

use crate::errors::FieldError;
use crate::view_models::connectors::create_import::CreateConnectorImportRequestViewModel;
use crate::view_models::transactions::validation::Validatable;

impl Validatable for CreateConnectorImportRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        if self.accounts.is_empty() {
            return Err(vec![FieldError {
                field: "accounts".to_string(),
                message: "Pick at least one account to import.".to_string(),
            }]);
        }
        let mut seen = HashSet::new();
        let errors: Vec<FieldError> = self
            .accounts
            .iter()
            .enumerate()
            .filter(|(_, account)| !seen.insert(account.provider_account_id.as_str()))
            .map(|(index, _)| FieldError {
                field: format!("accounts[{index}].provider_account_id"),
                message: "Each bank account can be imported once.".to_string(),
            })
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
