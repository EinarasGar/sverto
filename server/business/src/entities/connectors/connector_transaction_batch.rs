use std::collections::HashMap;

use connectors::models::transaction::ProviderTransaction;
use dal::models::connector_models::ConnectorTransactionRow;

pub(crate) struct ConnectorTransactionBatch {
    transactions: Vec<ProviderTransaction>,
    duplicates: usize,
}

pub(crate) struct ClassifiedBatch<'a> {
    pub new: Vec<&'a ProviderTransaction>,
    pub unchanged: Vec<&'a ProviderTransaction>,
    pub amended: Vec<&'a ProviderTransaction>,
    pub conflicts: Vec<&'a ProviderTransaction>,
}

impl<'a> ClassifiedBatch<'a> {
    pub fn stored(&self) -> Vec<&'a ProviderTransaction> {
        self.unchanged
            .iter()
            .chain(&self.amended)
            .chain(&self.conflicts)
            .copied()
            .collect()
    }
}

impl ConnectorTransactionBatch {
    pub fn from_mapped(mapped: Vec<ProviderTransaction>) -> Self {
        let mut by_external_id: HashMap<String, ProviderTransaction> = HashMap::new();
        let mut duplicates = 0usize;
        for tx in mapped {
            if by_external_id.insert(tx.external_id.clone(), tx).is_some() {
                duplicates += 1;
            }
        }

        let mut transactions: Vec<ProviderTransaction> = by_external_id.into_values().collect();
        transactions.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| a.external_id.cmp(&b.external_id))
        });

        Self {
            transactions,
            duplicates,
        }
    }

    pub fn external_ids(&self) -> Vec<String> {
        self.transactions
            .iter()
            .map(|tx| tx.external_id.clone())
            .collect()
    }

    pub fn duplicates(&self) -> usize {
        self.duplicates
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    pub fn classify(
        &self,
        ledger: &HashMap<String, ConnectorTransactionRow>,
    ) -> ClassifiedBatch<'_> {
        let mut classified = ClassifiedBatch {
            new: Vec::new(),
            unchanged: Vec::new(),
            amended: Vec::new(),
            conflicts: Vec::new(),
        };
        for tx in &self.transactions {
            match ledger.get(&tx.external_id) {
                None => classified.new.push(tx),
                Some(row) if row.external_hash == tx.external_hash() => {
                    classified.unchanged.push(tx)
                }
                Some(row) if row.edited_by_user => classified.conflicts.push(tx),
                Some(_) => classified.amended.push(tx),
            }
        }
        classified
    }
}

#[cfg(test)]
mod tests {
    use connectors::models::transaction::ProviderTransactionKind;
    use rust_decimal::Decimal;
    use time::macros::datetime;

    use super::*;

    fn provider_tx(external_id: &str, quantity: Option<Decimal>) -> ProviderTransaction {
        ProviderTransaction {
            external_id: external_id.to_string(),
            amount: Decimal::new(-100, 0),
            currency: "USD".to_string(),
            date: datetime!(2024-08-22 0:00 UTC),
            description: String::new(),
            asset_identifier: Some("AAPL.NASDAQ".to_string()),
            isin: None,
            quantity,
            kind: ProviderTransactionKind::Trade { fee: Decimal::ZERO },
        }
    }

    fn ledger_row(
        tx: &ProviderTransaction,
        hash: &str,
        edited_by_user: bool,
    ) -> ConnectorTransactionRow {
        ConnectorTransactionRow {
            transaction_id: None,
            external_id: tx.external_id.clone(),
            external_hash: hash.to_string(),
            edited_by_user,
        }
    }

    #[test]
    fn stored_covers_unchanged_amended_and_conflicting_rows_but_not_new_ones() {
        let unchanged = provider_tx("unchanged", Some(Decimal::ONE));
        let amended = provider_tx("amended", Some(Decimal::ONE));
        let conflict = provider_tx("conflict", Some(Decimal::ONE));
        let new = provider_tx("new", Some(Decimal::ONE));
        let ledger: HashMap<String, ConnectorTransactionRow> = [
            ledger_row(&unchanged, &unchanged.external_hash(), false),
            ledger_row(&amended, "old", false),
            ledger_row(&conflict, "old", true),
        ]
        .into_iter()
        .map(|row| (row.external_id.clone(), row))
        .collect();
        let batch = ConnectorTransactionBatch::from_mapped(vec![
            unchanged.clone(),
            amended.clone(),
            conflict.clone(),
            new.clone(),
        ]);

        let classified = batch.classify(&ledger);

        let mut stored: Vec<&str> = classified
            .stored()
            .into_iter()
            .map(|tx| tx.external_id.as_str())
            .collect();
        stored.sort_unstable();
        assert_eq!(stored, vec!["amended", "conflict", "unchanged"]);
        let new_ids: Vec<&str> = classified
            .new
            .iter()
            .map(|tx| tx.external_id.as_str())
            .collect();
        assert_eq!(new_ids, vec!["new"]);
    }

    #[test]
    fn a_fully_stored_batch_has_no_new_rows() {
        let tx = provider_tx("sell", Some(Decimal::NEGATIVE_ONE));
        let ledger: HashMap<String, ConnectorTransactionRow> = [(
            tx.external_id.clone(),
            ledger_row(&tx, &tx.external_hash(), false),
        )]
        .into_iter()
        .collect();
        let batch = ConnectorTransactionBatch::from_mapped(vec![tx]);

        let classified = batch.classify(&ledger);

        assert!(classified.new.is_empty());
        assert_eq!(classified.unchanged.len(), 1);
        assert_eq!(classified.stored().len(), 1);
    }
}
