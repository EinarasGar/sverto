use connectors::models::transaction::{ProviderTransaction, ProviderTransactionKind};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::entry_dto::EntryDto;
use crate::dtos::fee_entry_dto::FeeEntryDto;
use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
use crate::dtos::transaction_dto::{
    AccountFeesMetadataDto, AssetPurchaseMetadataDto, AssetSaleMetadataDto,
    CashDividendMetadataDto, CashTransferInMetadataDto, CashTransferOutMetadataDto,
    RegularTransactionMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
};
use crate::entities::transactions::metadata::ConnectorLinkMeta;
use crate::entities::transactions::transaction::Transaction;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

const IMPORT_CATEGORY_ID: i32 = 76;

// TODO: move the Trading212 ticker handling (aliases, `_EQ`/`_US`/`l` suffixes) to its mapper.
const TICKER_ALIASES: &[(&str, &str)] = &[("FB_US_EQ", "META.NASDAQ"), ("JAYl_EQ", "S5WA.F")];

pub(crate) enum TransactionImportOutcome {
    Ready(Transaction),
    Unresolvable(&'static str),
}

#[derive(Clone)]
pub(crate) struct ProviderTransactionImport {
    transaction: ProviderTransaction,
    account_id: Uuid,
    visibility: TransactionVisibilityDto,
    binding_id: Uuid,
}

impl ProviderTransactionImport {
    pub fn new(
        transaction: ProviderTransaction,
        account_id: Uuid,
        visibility: TransactionVisibilityDto,
        binding_id: Uuid,
    ) -> Self {
        Self {
            transaction,
            account_id,
            visibility,
            binding_id,
        }
    }

    pub fn external_id(&self) -> &str {
        &self.transaction.external_id
    }

    pub fn external_hash(&self) -> String {
        self.transaction.external_hash()
    }

    pub fn currency(&self) -> &str {
        &self.transaction.currency
    }

    pub fn isin(&self) -> Option<&str> {
        self.transaction.isin.as_deref()
    }

    pub fn root_symbol(&self) -> Option<String> {
        if !matches!(
            self.transaction.kind,
            ProviderTransactionKind::Dividend { .. }
        ) {
            return None;
        }
        let ticker = self.instrument_ticker_candidates().into_iter().next()?;
        let root = ticker.split('.').next().unwrap_or(&ticker);
        Some(root.to_string())
    }

    pub fn is_trade(&self) -> bool {
        matches!(self.transaction.kind, ProviderTransactionKind::Trade { .. })
    }

    pub fn amount(&self) -> Decimal {
        self.transaction.amount
    }

    pub fn date(&self) -> OffsetDateTime {
        self.transaction.date
    }

    pub fn instrument_ticker_candidates(&self) -> Vec<String> {
        let Some(provider_ticker) = self.transaction.asset_identifier.as_deref() else {
            return vec![];
        };

        if let Some((_, alias)) = TICKER_ALIASES
            .iter()
            .find(|(from, _)| *from == provider_ticker)
        {
            return vec![(*alias).to_string()];
        }

        let stripped = provider_ticker
            .strip_suffix("_EQ")
            .unwrap_or(provider_ticker);

        if let Some(base) = stripped.strip_suffix("_US") {
            return vec![format!("{base}.NASDAQ"), format!("{base}.NYSE")];
        }
        if let Some(base) = stripped.strip_suffix('l') {
            if !base.is_empty()
                && base
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.')
            {
                return vec![format!("{base}.LSE")];
            }
        }
        // Asset data sometimes lists a US stock under the other US exchange (e.g. ETSY as `.NYSE`).
        if let Some(base) = stripped.strip_suffix(".NASDAQ") {
            return vec![stripped.to_string(), format!("{base}.NYSE")];
        }
        if let Some(base) = stripped.strip_suffix(".NYSE") {
            return vec![stripped.to_string(), format!("{base}.NASDAQ")];
        }
        vec![stripped.to_string()]
    }

    pub fn try_into_transaction(
        &self,
        user_id: Uuid,
        cash_asset_id: Option<i32>,
        instrument_asset_id: Option<i32>,
    ) -> anyhow::Result<TransactionImportOutcome> {
        match self.build_dto(cash_asset_id, instrument_asset_id) {
            Ok(dto) => {
                let mut entity = create_transaction_from_dto(dto, user_id)?;
                entity.set_connector_link(Some(ConnectorLinkMeta {
                    binding_id: self.binding_id,
                    external_id: self.transaction.external_id.clone(),
                    external_hash: self.external_hash(),
                }));
                Ok(TransactionImportOutcome::Ready(entity))
            }
            Err(reason) => Ok(TransactionImportOutcome::Unresolvable(reason)),
        }
    }

    fn build_dto(
        &self,
        cash_asset_id: Option<i32>,
        instrument_asset_id: Option<i32>,
    ) -> Result<TransactionDto, &'static str> {
        let tx = &self.transaction;
        let Some(cash_asset_id) = cash_asset_id else {
            return Err("currency has no matching asset");
        };

        let cash_entry = EntryDto::new(cash_asset_id, self.account_id, tx.amount);
        let fee_entry = |quantity: Decimal, entry_type: FeeEntryTypesDto| {
            (!quantity.is_zero()).then(|| FeeEntryDto {
                entry: EntryDto::new(cash_asset_id, self.account_id, quantity),
                entry_type,
            })
        };

        let (transaction_type, fee_entry) = match tx.kind {
            ProviderTransactionKind::Trade { fee } => {
                let Some(quantity) = tx.quantity.filter(|_| tx.asset_identifier.is_some()) else {
                    return Err("trade without quantity or instrument identifier");
                };
                let Some(instrument_asset_id) = instrument_asset_id else {
                    return Err("instrument has no matching asset");
                };
                if quantity.is_zero() {
                    return Err("zero-quantity trade");
                }
                let transaction_type = if quantity.is_sign_positive() {
                    TransactionTypeDto::AssetPurchase(AssetPurchaseMetadataDto {
                        purchase: EntryDto::new(instrument_asset_id, self.account_id, quantity),
                        sale: cash_entry,
                    })
                } else {
                    TransactionTypeDto::AssetSale(AssetSaleMetadataDto {
                        sale: EntryDto::new(instrument_asset_id, self.account_id, quantity),
                        proceeds: cash_entry,
                    })
                };
                (
                    transaction_type,
                    fee_entry(fee, FeeEntryTypesDto::Transaction),
                )
            }
            // TODO: import dividend reversals; a Cash Dividend can't hold a negative amount.
            ProviderTransactionKind::Dividend { .. } if tx.amount.is_sign_negative() => {
                return Err("negative dividend (reversal) is not supported");
            }
            ProviderTransactionKind::Dividend { withholding_tax } => {
                let Some(instrument_asset_id) = instrument_asset_id else {
                    return Err("dividend origin has no matching asset");
                };
                (
                    TransactionTypeDto::CashDividend(CashDividendMetadataDto {
                        entry: cash_entry,
                        origin_asset_id: instrument_asset_id,
                    }),
                    fee_entry(withholding_tax, FeeEntryTypesDto::WithholdingTax),
                )
            }
            ProviderTransactionKind::CashTransfer if tx.amount.is_sign_negative() => (
                TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                    entry: cash_entry,
                }),
                None,
            ),
            ProviderTransactionKind::CashTransfer => (
                TransactionTypeDto::CashTransferIn(CashTransferInMetadataDto { entry: cash_entry }),
                None,
            ),
            // TODO: import fee refunds; an Account Fees transaction can't hold a positive amount.
            ProviderTransactionKind::AccountFee if tx.amount > Decimal::ZERO => {
                return Err("positive account fee (refund) is not supported");
            }
            ProviderTransactionKind::AccountFee => (
                TransactionTypeDto::AccountFees(AccountFeesMetadataDto { entry: cash_entry }),
                None,
            ),
            ProviderTransactionKind::Regular => (
                TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                    description: Some(tx.description.clone()),
                    entry: cash_entry,
                    category_id: IMPORT_CATEGORY_ID,
                }),
                None,
            ),
        };

        Ok(TransactionDto {
            transaction_id: None,
            date: tx.date,
            visibility: self.visibility,
            fee_entries: fee_entry.into_iter().collect(),
            transaction_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    use super::*;

    const CASH: i32 = 1;
    const INSTRUMENT: i32 = 2;

    fn provider_tx(
        amount: Decimal,
        asset_identifier: Option<&str>,
        quantity: Option<Decimal>,
        kind: ProviderTransactionKind,
    ) -> ProviderTransactionImport {
        ProviderTransactionImport::new(
            ProviderTransaction {
                external_id: "1".to_string(),
                amount,
                currency: "EUR".to_string(),
                date: datetime!(2026-09-15 0:00 UTC),
                description: String::new(),
                asset_identifier: asset_identifier.map(str::to_string),
                isin: None,
                quantity,
                kind,
            },
            Uuid::nil(),
            TransactionVisibilityDto::Default,
            Uuid::nil(),
        )
    }

    fn dividend(asset_identifier: &str) -> ProviderTransactionImport {
        provider_tx(
            Decimal::ONE,
            Some(asset_identifier),
            None,
            ProviderTransactionKind::Dividend {
                withholding_tax: Decimal::ZERO,
            },
        )
    }

    fn cash(amount: Decimal, kind: ProviderTransactionKind) -> TransactionDto {
        provider_tx(amount, None, None, kind)
            .build_dto(Some(CASH), None)
            .unwrap()
    }

    fn fees(dto: &TransactionDto) -> Vec<(Decimal, FeeEntryTypesDto)> {
        dto.fee_entries
            .iter()
            .map(|fee| (fee.entry.quantity, fee.entry_type.clone()))
            .collect()
    }

    #[test]
    fn root_symbol_strips_exchange_from_normalised_ticker() {
        let cases = [
            ("AAPL.NASDAQ", "AAPL"),
            ("AAPL_US_EQ", "AAPL"),
            ("SHELL", "SHELL"),
            ("PBR-A", "PBR-A"),
        ];
        for (identifier, expected) in cases {
            assert_eq!(
                dividend(identifier).root_symbol().as_deref(),
                Some(expected),
                "{identifier}"
            );
        }
    }

    #[test]
    fn trades_have_no_root_symbol() {
        let trade = provider_tx(
            dec!(-10),
            Some("SHELL"),
            Some(Decimal::ONE),
            ProviderTransactionKind::Trade { fee: Decimal::ZERO },
        );
        assert_eq!(trade.root_symbol(), None);
    }

    #[test]
    fn trade_commission_becomes_a_transaction_fee_entry() {
        let dto = provider_tx(
            dec!(-100),
            Some("AAPL.NASDAQ"),
            Some(dec!(2)),
            ProviderTransactionKind::Trade { fee: dec!(-1.25) },
        )
        .build_dto(Some(CASH), Some(INSTRUMENT))
        .unwrap();

        let TransactionTypeDto::AssetPurchase(purchase) = &dto.transaction_type else {
            panic!("expected an asset purchase");
        };
        assert_eq!(purchase.sale.quantity, dec!(-100));
        assert_eq!(
            fees(&dto),
            vec![(dec!(-1.25), FeeEntryTypesDto::Transaction)]
        );
    }

    #[test]
    fn withholding_tax_becomes_a_fee_entry_on_the_dividend() {
        let dto = provider_tx(
            dec!(8.30),
            Some("MSFT.NASDAQ"),
            None,
            ProviderTransactionKind::Dividend {
                withholding_tax: dec!(-1.25),
            },
        )
        .build_dto(Some(CASH), Some(INSTRUMENT))
        .unwrap();

        let TransactionTypeDto::CashDividend(dividend) = &dto.transaction_type else {
            panic!("expected a cash dividend");
        };
        assert_eq!(dividend.entry.quantity, dec!(8.30));
        assert_eq!(
            fees(&dto),
            vec![(dec!(-1.25), FeeEntryTypesDto::WithholdingTax)]
        );
    }

    #[test]
    fn cash_transfer_direction_follows_the_amount_sign() {
        assert!(matches!(
            cash(dec!(500), ProviderTransactionKind::CashTransfer).transaction_type,
            TransactionTypeDto::CashTransferIn(_)
        ));
        assert!(matches!(
            cash(dec!(-500), ProviderTransactionKind::CashTransfer).transaction_type,
            TransactionTypeDto::CashTransferOut(_)
        ));
    }

    #[test]
    fn account_fee_is_its_own_transaction_without_fee_entries() {
        let dto = cash(dec!(-3), ProviderTransactionKind::AccountFee);
        assert!(matches!(
            dto.transaction_type,
            TransactionTypeDto::AccountFees(_)
        ));
        assert!(dto.fee_entries.is_empty());
    }
}
