use time::Date;

use super::connector_transaction_batch::ConnectorTransactionBatch;
use crate::dtos::connectors::{ConnectorImportFromDto, HistoryStartDto};
use crate::dtos::imports::import_source_dto::ImportBankLineDto;

pub(crate) fn history_start(
    import_from: ConnectorImportFromDto,
    latest_entries: impl IntoIterator<Item = Date>,
) -> Option<HistoryStartDto> {
    match import_from {
        ConnectorImportFromDto::All => None,
        ConnectorImportFromDto::AfterLastEntry => latest_entries
            .into_iter()
            .max()
            .map(HistoryStartDto::AfterLastEntry),
        ConnectorImportFromDto::Date(date) => Some(HistoryStartDto::From(date)),
    }
}

pub(crate) fn lines(
    batch: &ConnectorTransactionBatch,
    start: Option<Date>,
) -> Vec<ImportBankLineDto> {
    batch
        .transactions()
        .iter()
        .filter(|transaction| transaction.asset_identifier.is_none())
        .filter(|transaction| !transaction.amount.is_zero())
        .filter(|transaction| start.is_none_or(|start| transaction.date.date() >= start))
        .map(|transaction| ImportBankLineDto {
            external_id: transaction.external_id.clone(),
            date: transaction.date.date(),
            description: Some(transaction.description.clone())
                .filter(|description| !description.is_empty()),
            amount: transaction.amount,
            currency: transaction.currency.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use connectors::models::transaction::ProviderTransaction;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};
    use time::OffsetDateTime;

    use super::*;

    fn transaction(
        external_id: &str,
        date: OffsetDateTime,
        amount: Decimal,
    ) -> ProviderTransaction {
        ProviderTransaction {
            external_id: external_id.to_string(),
            amount,
            currency: "EUR".to_string(),
            date,
            description: format!("Payment {external_id}"),
            asset_identifier: None,
            quantity: None,
            counterparty_iban: None,
        }
    }

    fn batch() -> ConnectorTransactionBatch {
        let mut trade = transaction("trade", datetime!(2026-03-05 0:00 UTC), dec!(-100));
        trade.asset_identifier = Some("AAPL_US_EQ".to_string());
        trade.quantity = Some(dec!(1));
        ConnectorTransactionBatch::from_mapped(vec![
            transaction("early", datetime!(2026-03-01 0:00 UTC), dec!(-12.5)),
            transaction("same-day", datetime!(2026-03-04 0:00 UTC), dec!(40)),
            transaction("late", datetime!(2026-03-09 0:00 UTC), dec!(-3)),
            trade,
        ])
    }

    fn kept(batch: &ConnectorTransactionBatch, start: Option<Date>) -> Vec<String> {
        lines(batch, start)
            .into_iter()
            .map(|line| line.external_id)
            .collect()
    }

    #[test]
    fn rows_from_the_start_day_on_are_kept() {
        assert_eq!(
            kept(&batch(), Some(date!(2026 - 03 - 04))),
            vec!["same-day", "late"]
        );
    }
}
