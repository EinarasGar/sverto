use std::collections::{HashMap, HashSet};

use connectors::models::transaction::ProviderTransaction;
use dal::models::connector_models::{AddConnectorTransactionModel, ConnectorTransactionRow};
use uuid::Uuid;

use crate::dtos::connectors::HistoryStartDto;

pub(crate) struct ConnectorTransactionBatch {
    transactions: Vec<ProviderTransaction>,
    duplicates: usize,
}

pub(crate) struct ClassifiedBatch<'a> {
    pub new: Vec<&'a ProviderTransaction>,
    pub unchanged: usize,
    pub amended: Vec<&'a ProviderTransaction>,
    pub conflicts: Vec<&'a ProviderTransaction>,
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

    pub fn transactions(&self) -> &[ProviderTransaction] {
        &self.transactions
    }

    pub fn connector_transactions(
        &self,
        binding_id: Uuid,
        transaction_ids: &HashMap<&str, Option<Uuid>>,
    ) -> Vec<AddConnectorTransactionModel> {
        self.transactions
            .iter()
            .map(|transaction| AddConnectorTransactionModel {
                binding_id,
                transaction_id: transaction_ids
                    .get(transaction.external_id.as_str())
                    .copied()
                    .flatten(),
                external_id: transaction.external_id.clone(),
                external_hash: transaction.external_hash(),
            })
            .collect()
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
        held: &HashSet<String>,
        history_start: Option<HistoryStartDto>,
    ) -> ClassifiedBatch<'_> {
        let before_start = |tx: &ProviderTransaction| match history_start {
            None => false,
            Some(HistoryStartDto::From(day)) => tx.date.date() < day,
            Some(HistoryStartDto::AfterLastEntry(day)) if tx.asset_identifier.is_some() => {
                tx.date.date() <= day
            }
            Some(HistoryStartDto::AfterLastEntry(day)) => tx.date.date() < day,
        };
        let mut classified = ClassifiedBatch {
            new: Vec::new(),
            unchanged: 0,
            amended: Vec::new(),
            conflicts: Vec::new(),
        };
        for tx in &self.transactions {
            match ledger.get(&tx.external_id) {
                None if held.contains(&tx.external_id) => classified.unchanged += 1,
                None if before_start(tx) => classified.unchanged += 1,
                None => classified.new.push(tx),
                Some(row) if row.external_hash == tx.external_hash() => classified.unchanged += 1,
                Some(row) if row.edited_by_user => classified.conflicts.push(tx),
                Some(_) => classified.amended.push(tx),
            }
        }
        classified
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};

    use super::*;

    fn transaction(external_id: &str) -> ProviderTransaction {
        ProviderTransaction {
            external_id: external_id.to_string(),
            amount: dec!(-4),
            currency: "EUR".to_string(),
            date: datetime!(2026-03-01 0:00 UTC),
            description: format!("Payment {external_id}"),
            asset_identifier: None,
            quantity: None,
            counterparty_iban: None,
        }
    }

    #[test]
    fn every_archived_row_is_recorded_and_rows_the_import_left_out_get_no_transaction() {
        let binding_id = Uuid::from_u128(1);
        let imported = Uuid::from_u128(2);
        let batch = ConnectorTransactionBatch::from_mapped(vec![
            transaction("before-start"),
            transaction("imported"),
            transaction("skipped"),
        ]);

        let recorded = batch.connector_transactions(
            binding_id,
            &HashMap::from([("imported", Some(imported)), ("skipped", None)]),
        );

        let recorded: Vec<_> = recorded
            .iter()
            .map(|row| (row.external_id.as_str(), row.transaction_id))
            .collect();
        assert_eq!(
            recorded,
            vec![
                ("before-start", None),
                ("imported", Some(imported)),
                ("skipped", None),
            ]
        );
    }

    #[test]
    fn recorded_deleted_and_held_rows_are_not_new() {
        let batch = ConnectorTransactionBatch::from_mapped(vec![
            transaction("recorded"),
            transaction("deleted"),
            transaction("held"),
            transaction("fresh"),
        ]);
        let known = |external_id: &str, transaction_id: Option<Uuid>| {
            (
                external_id.to_string(),
                ConnectorTransactionRow {
                    binding_id: Uuid::from_u128(1),
                    transaction_id,
                    external_id: external_id.to_string(),
                    external_hash: transaction(external_id).external_hash(),
                    edited_by_user: false,
                },
            )
        };
        let ledger = HashMap::from([
            known("recorded", Some(Uuid::from_u128(1))),
            known("deleted", None),
        ]);

        let classified = batch.classify(&ledger, &HashSet::from(["held".to_string()]), None);

        let new: Vec<&str> = classified
            .new
            .iter()
            .map(|tx| tx.external_id.as_str())
            .collect();
        assert_eq!(new, vec!["fresh"]);
        assert_eq!(classified.unchanged, 3);
    }

    #[test]
    fn after_the_last_entry_its_day_brings_cash_rows_but_no_trades() {
        let mut boundary_trade = transaction("boundary-trade");
        boundary_trade.date = datetime!(2026-03-01 15:00 UTC);
        boundary_trade.asset_identifier = Some("AAPL_US_EQ".to_string());
        boundary_trade.quantity = Some(dec!(1));
        let mut next_day_trade = boundary_trade.clone();
        next_day_trade.external_id = "next-day-trade".to_string();
        next_day_trade.date = datetime!(2026-03-02 09:00 UTC);
        let mut boundary_cash = transaction("boundary-cash");
        boundary_cash.date = datetime!(2026-03-01 18:00 UTC);
        let mut day_before_cash = transaction("day-before-cash");
        day_before_cash.date = datetime!(2026-02-28 12:00 UTC);
        let batch = ConnectorTransactionBatch::from_mapped(vec![
            boundary_trade,
            next_day_trade,
            boundary_cash,
            day_before_cash,
        ]);

        let classified = batch.classify(
            &HashMap::new(),
            &HashSet::new(),
            Some(HistoryStartDto::AfterLastEntry(date!(2026 - 03 - 01))),
        );

        let new: Vec<&str> = classified
            .new
            .iter()
            .map(|tx| tx.external_id.as_str())
            .collect();
        assert_eq!(new, vec!["boundary-cash", "next-day-trade"]);
        assert_eq!(classified.unchanged, 2);
    }
}
