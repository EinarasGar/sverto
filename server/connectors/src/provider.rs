use std::str::FromStr;

use async_trait::async_trait;
use serde_json::Value;

use crate::enablebanking::provider::EnableBankingProvider;
use crate::ibkr::provider::IbkrProvider;
use crate::models::{account::ProviderAccount, aspsp::Aspsp, transaction::MappedTransaction};
use crate::port::{Connector, ConnectorStore};
use crate::trading212::provider::Trading212Provider;
use crate::truelayer::provider::TrueLayerProvider;
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Trading212,
    TrueLayer,
    EnableBanking,
    Ibkr,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Trading212 => "trading212",
            ProviderKind::TrueLayer => "truelayer",
            ProviderKind::EnableBanking => "enablebanking",
            ProviderKind::Ibkr => "ibkr",
        }
    }

    /// Whether this provider exposes bank (ASPSP) discovery used to pick a bank before OAuth.
    pub fn supports_aspsps(self) -> bool {
        matches!(self, ProviderKind::EnableBanking)
    }

    fn map_items<'a>(
        self,
        items: impl Iterator<Item = (&'a str, &'a Value)>,
    ) -> Vec<MappedTransaction> {
        if self == ProviderKind::Ibkr {
            // Rows are mapped together: withholding tax rows are folded into their dividends.
            return crate::ibkr::mapper::map_rows(
                items
                    .filter(|(stream, _)| *stream == "statement")
                    .map(|(_, item)| item),
            );
        }
        items
            .filter_map(|(stream, item)| match (self, stream) {
                (ProviderKind::Trading212, "transactions") => {
                    Some(crate::trading212::mapper::map_transaction(item))
                }
                (ProviderKind::Trading212, "orders") => {
                    Some(crate::trading212::mapper::map_order(item))
                }
                (ProviderKind::Trading212, "dividends") => {
                    Some(crate::trading212::mapper::map_dividend(item))
                }
                (ProviderKind::TrueLayer, "transactions") => {
                    Some(crate::truelayer::mapper::map_transaction(item))
                }
                (ProviderKind::EnableBanking, "transactions") => {
                    Some(crate::enablebanking::mapper::map_transaction(item))
                }
                _ => None,
            })
            .collect()
    }
}

impl FromStr for ProviderKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "trading212" => Ok(ProviderKind::Trading212),
            "truelayer" => Ok(ProviderKind::TrueLayer),
            "enablebanking" => Ok(ProviderKind::EnableBanking),
            "ibkr" => Ok(ProviderKind::Ibkr),
            other => anyhow::bail!("unknown provider kind: {other}"),
        }
    }
}

pub enum CredentialSource {
    Stored,
    Transient(String),
}

#[derive(Debug, Clone, Default)]
pub struct BeginOauthOptions {
    pub bank_name: Option<String>,
    pub bank_country: Option<String>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    async fn build_connector(
        &self,
        provider_account_id: &str,
        credential: CredentialSource,
        store: &dyn ConnectorStore,
    ) -> Result<Box<dyn Connector>>;

    fn resolve_provider_account_id(
        &self,
        client_value: Option<String>,
        store: &dyn ConnectorStore,
    ) -> Result<String>;

    async fn list_accounts(&self, store: &dyn ConnectorStore) -> Result<Vec<ProviderAccount>>;

    fn needs_attended_backfill(&self) -> bool {
        false
    }

    async fn begin_oauth(
        &self,
        _store: &dyn ConnectorStore,
        _state: &str,
        _redirect_uri: Option<&str>,
        _options: BeginOauthOptions,
    ) -> Result<String> {
        anyhow::bail!("provider does not support OAuth: {}", self.kind().as_str())
    }

    async fn list_aspsps(&self, _store: &dyn ConnectorStore, _country: &str) -> Result<Vec<Aspsp>> {
        anyhow::bail!(
            "provider does not support ASPSP discovery: {}",
            self.kind().as_str()
        )
    }

    async fn complete_oauth(
        &self,
        _store: &dyn ConnectorStore,
        _code: &str,
        _redirect_uri: Option<&str>,
    ) -> Result<Option<time::OffsetDateTime>> {
        anyhow::bail!("provider does not support OAuth: {}", self.kind().as_str())
    }
}

pub fn map_pages(
    kind: ProviderKind,
    pages: &[crate::models::sync::RawPage],
) -> Vec<crate::models::transaction::ProviderTransaction> {
    let items = pages.iter().flat_map(|page| {
        page.payload
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| (page.stream.as_str(), item))
    });
    let mut transactions = Vec::new();
    let mut skipped = Vec::new();
    for mapped in kind.map_items(items) {
        match mapped {
            MappedTransaction::Provider(p) => transactions.push(p),
            MappedTransaction::Skipped(s) => skipped.push(s),
        }
    }
    crate::models::transaction::log_skipped(kind.as_str(), &skipped);
    transactions
}

impl ProviderKind {
    pub fn provider(self) -> &'static dyn Provider {
        static TRADING212: Trading212Provider = Trading212Provider;
        static TRUELAYER: TrueLayerProvider = TrueLayerProvider;
        static ENABLEBANKING: EnableBankingProvider = EnableBankingProvider;
        static IBKR: IbkrProvider = IbkrProvider;
        match self {
            ProviderKind::Trading212 => &TRADING212,
            ProviderKind::TrueLayer => &TRUELAYER,
            ProviderKind::EnableBanking => &ENABLEBANKING,
            ProviderKind::Ibkr => &IBKR,
        }
    }
}
