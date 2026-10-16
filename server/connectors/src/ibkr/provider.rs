use async_trait::async_trait;

use crate::ibkr::client::IbkrFlexClient;
use crate::models::account::ProviderAccount;
use crate::port::{Connector, ConnectorStore};
use crate::provider::{CredentialSource, Provider, ProviderKind};
use crate::Result;

/// `provider_key_id` is the Flex Query ID; the stored credential is the Flex token.
pub struct IbkrProvider;

#[async_trait]
impl Provider for IbkrProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ibkr
    }

    async fn build_connector(
        &self,
        _provider_account_id: &str,
        credential: CredentialSource,
        store: &dyn ConnectorStore,
    ) -> Result<Box<dyn Connector>> {
        let query_id = store
            .provider_key_id()
            .ok_or_else(|| anyhow::anyhow!("ibkr connection has no flex query id"))?;
        let token = match credential {
            CredentialSource::Transient(token) => token,
            CredentialSource::Stored => {
                let bytes = store
                    .get_credential()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Credential not found"))?;
                String::from_utf8(bytes)?
            }
        };
        Ok(Box::new(IbkrFlexClient::new(query_id, token)))
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
            display_name: "Interactive Brokers".to_string(),
            currency: None,
            account_type: None,
        }])
    }
}
