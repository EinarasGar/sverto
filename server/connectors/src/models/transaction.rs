use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTransaction {
    pub external_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub date: time::OffsetDateTime,
    pub description: String,
    pub asset_identifier: Option<String>,
    /// Resolution fallback when `asset_identifier` matches no ticker; not part of `external_hash`.
    pub isin: Option<String>,
    pub quantity: Option<Decimal>,
    pub kind: ProviderTransactionKind,
}

/// Fees are signed cash changes in `currency` on top of `amount`: negative for a charge, zero for none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderTransactionKind {
    Regular,
    Trade {
        fee: Decimal,
    },
    Dividend {
        withholding_tax: Decimal,
    },
    /// Deposits, withdrawals and interest; the sign of `amount` gives the direction.
    CashTransfer,
    AccountFee,
}

impl ProviderTransaction {
    pub fn external_hash(&self) -> String {
        let mut parts = vec![
            self.external_id.clone(),
            self.amount.to_string(),
            self.currency.clone(),
            self.date
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            self.description.clone(),
            self.asset_identifier.clone().unwrap_or_default(),
            self.quantity.map(|q| q.to_string()).unwrap_or_default(),
        ];
        // Regular rows keep their original hash so stored bank rows don't all read as amended.
        if self.kind != ProviderTransactionKind::Regular {
            parts.push(format!("{:?}", self.kind));
        }
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part.as_bytes());
            hasher.update(b":");
        }
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedTransaction {
    pub external_id: String,
    pub reason: String,
}

pub enum MappedTransaction {
    Provider(ProviderTransaction),
    Skipped(SkippedTransaction),
}

pub(crate) fn log_skipped(provider: &str, skipped: &[SkippedTransaction]) {
    if skipped.is_empty() {
        return;
    }
    let mut reasons: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for skip in skipped {
        *reasons.entry(skip.reason.as_str()).or_default() += 1;
        tracing::debug!(
            provider,
            external_id = %skip.external_id,
            reason = %skip.reason,
            "provider transaction skipped during mapping"
        );
    }
    tracing::debug!(
        provider,
        skipped = skipped.len(),
        reasons = ?reasons,
        "mapping skipped provider transactions"
    );
}
