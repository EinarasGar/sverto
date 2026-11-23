use async_trait::async_trait;

use crate::models::account::ProviderAccount;
use crate::port::{Connector, ConnectorStore};
use crate::provider::{CredentialSource, Provider, ProviderKind};
use crate::trading212::client::{Trading212Client, Trading212Env};
use crate::Result;

pub struct Trading212Provider;

#[async_trait]
impl Provider for Trading212Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Trading212
    }

    async fn build_connector(
        &self,
        _provider_account_id: &str,
        credential: CredentialSource,
        store: &dyn ConnectorStore,
    ) -> Result<Box<dyn Connector>> {
        let api_key_id = store
            .provider_key_id()
            .ok_or_else(|| anyhow::anyhow!("trading212 connection has no provider_key_id"))?;
        let client = match credential {
            CredentialSource::Transient(secret) => {
                Trading212Client::new(api_key_id, secret, Trading212Env::Live)
            }
            CredentialSource::Stored => {
                let bytes = store
                    .get_credential()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Credential not found"))?;
                Trading212Client::new(api_key_id, String::from_utf8(bytes)?, Trading212Env::Live)
                    .waiting_for_rate_limits()
            }
        };
        Ok(Box::new(client))
    }

    fn resolve_provider_account_id(
        &self,
        _client_value: Option<String>,
        store: &dyn ConnectorStore,
    ) -> Result<String> {
        Ok(store
            .provider_key_id()
            .unwrap_or_else(|| "default".to_string()))
    }

    async fn list_accounts(&self, store: &dyn ConnectorStore) -> Result<Vec<ProviderAccount>> {
        Ok(vec![ProviderAccount {
            provider_account_id: store
                .provider_key_id()
                .unwrap_or_else(|| "default".to_string()),
            display_name: "Trading212".to_string(),
            currency: None,
            account_type: None,
            account_number_last4: None,
            balance: None,
            booked_balance: None,
            iban: None,
        }])
    }
}
