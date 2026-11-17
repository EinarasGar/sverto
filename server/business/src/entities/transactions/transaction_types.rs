use account_fees::AccountFeesTransaction;
use anyhow::Result;
use asset_balance_transfer::AssetBalanceTransferTransaction;
use asset_dividend::AssetDividendTransaction;
use asset_purhcase::AssetPurchaseTransaction;
use asset_sale::AssetSaleTransaction;
use asset_trade::AssetTradeTransaction;
use asset_transfer_in::AssetTransferInTransaction;
use asset_transfer_out::AssetTransferOutTransaction;
use cash_balance_transfer::CashBalanceTransferTransaction;
use cash_dividend::CashDividendTransaction;
use cash_transfer_in::CashTransferInTransaction;
use cash_transfer_out::CashTransferOutTransaction;
use dal::{
    enums::transaction_types::DatabaseTransactionTypes,
    models::transaction_models::TransactionWithEntriesModel,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::dtos::transaction_dto::TransactionDto;

use self::regular_transaction::RegularTransaction;

use super::transaction::{Transaction, TransactionProcessor};

pub mod account_fees;
pub mod asset_balance_transfer;
pub mod asset_dividend;
pub mod asset_purhcase;
pub mod asset_sale;
pub mod asset_trade;
pub mod asset_transfer_in;
pub mod asset_transfer_out;
pub mod cash_balance_transfer;
pub mod cash_dividend;
pub mod cash_transfer_in;
pub mod cash_transfer_out;
pub mod regular_transaction;

pub enum TransactionTypes {
    RegularTransaction,
    CashTransferOut,
    CashTransferIn,
    CashDividend,
    AssetTransferOut,
    AssetTransferIn,
    AssetTrade,
    AssetSale,
    AssetPurchase,
    AssetDividend,
    AssetBalanceTransfer,
    AccountFees,
    CashBalanceTransfer,
}

impl From<DatabaseTransactionTypes> for TransactionTypes {
    fn from(value: DatabaseTransactionTypes) -> Self {
        match value {
            DatabaseTransactionTypes::RegularTransaction => TransactionTypes::RegularTransaction,
            DatabaseTransactionTypes::CashTransferOut => TransactionTypes::CashTransferOut,
            DatabaseTransactionTypes::CashTransferIn => TransactionTypes::CashTransferIn,
            DatabaseTransactionTypes::CashDividend => TransactionTypes::CashDividend,
            DatabaseTransactionTypes::AssetTransferOut => TransactionTypes::AssetTransferOut,
            DatabaseTransactionTypes::AssetTransferIn => TransactionTypes::AssetTransferIn,
            DatabaseTransactionTypes::AssetTrade => TransactionTypes::AssetTrade,
            DatabaseTransactionTypes::AssetSale => TransactionTypes::AssetSale,
            DatabaseTransactionTypes::AssetPurchase => TransactionTypes::AssetPurchase,
            DatabaseTransactionTypes::AssetDividend => TransactionTypes::AssetDividend,
            DatabaseTransactionTypes::AssetBalanceTransfer => {
                TransactionTypes::AssetBalanceTransfer
            }
            DatabaseTransactionTypes::AccountFees => TransactionTypes::AccountFees,
            DatabaseTransactionTypes::CashBalanceTransfer => TransactionTypes::CashBalanceTransfer,
        }
    }
}

fn get_dto_constructor(
    choice: TransactionTypes,
) -> &'static dyn Fn(TransactionDto, Uuid) -> Result<Transaction> {
    match choice {
        TransactionTypes::RegularTransaction => &RegularTransaction::try_from_dto,
        TransactionTypes::AssetPurchase => &AssetPurchaseTransaction::try_from_dto,
        TransactionTypes::CashTransferOut => &CashTransferOutTransaction::try_from_dto,
        TransactionTypes::CashTransferIn => &CashTransferInTransaction::try_from_dto,
        TransactionTypes::CashDividend => &CashDividendTransaction::try_from_dto,
        TransactionTypes::AssetTransferOut => &AssetTransferOutTransaction::try_from_dto,
        TransactionTypes::AssetTransferIn => &AssetTransferInTransaction::try_from_dto,
        TransactionTypes::AssetTrade => &AssetTradeTransaction::try_from_dto,
        TransactionTypes::AssetSale => &AssetSaleTransaction::try_from_dto,
        TransactionTypes::AssetDividend => &AssetDividendTransaction::try_from_dto,
        TransactionTypes::AssetBalanceTransfer => &AssetBalanceTransferTransaction::try_from_dto,
        TransactionTypes::AccountFees => &AccountFeesTransaction::try_from_dto,
        TransactionTypes::CashBalanceTransfer => &CashBalanceTransferTransaction::try_from_dto,
    }
}

fn get_model_constructor(
    choice: TransactionTypes,
) -> &'static dyn Fn(Vec<TransactionWithEntriesModel>) -> Transaction {
    match choice {
        TransactionTypes::RegularTransaction => {
            &RegularTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetPurchase => {
            &AssetPurchaseTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::CashTransferOut => {
            &CashTransferOutTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::CashTransferIn => {
            &CashTransferInTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::CashDividend => {
            &CashDividendTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetTransferOut => {
            &AssetTransferOutTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetTransferIn => {
            &AssetTransferInTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetTrade => {
            &AssetTradeTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetSale => &AssetSaleTransaction::from_transaction_with_entries_models,
        TransactionTypes::AssetDividend => {
            &AssetDividendTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AssetBalanceTransfer => {
            &AssetBalanceTransferTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::AccountFees => {
            &AccountFeesTransaction::from_transaction_with_entries_models
        }
        TransactionTypes::CashBalanceTransfer => {
            &CashBalanceTransferTransaction::from_transaction_with_entries_models
        }
    }
}

pub fn create_transaction_from_dto(value: TransactionDto, user_id: Uuid) -> Result<Transaction> {
    let constructor = get_dto_constructor(value.clone().transaction_type.into());
    constructor(value, user_id)
}

pub fn create_transaction_from_transaction_with_entries_model(
    value: Vec<TransactionWithEntriesModel>,
) -> anyhow::Result<Transaction> {
    // check if all models have same type_id
    if value.is_empty() {
        return Err(anyhow::anyhow!("No transaction with entries model found"));
    }
    let is_same = value.iter().all(|x| x.type_id == value[0].type_id);
    if !is_same {
        return Err(anyhow::anyhow!(
            "All transaction with entries model should have same type_id"
        ));
    }

    let type_id = value[0].type_id;

    let constructor = get_model_constructor(type_id.into());
    let transaction = constructor(value);
    Ok(transaction)
}

pub fn create_transactions_from_transaction_with_entries_models(
    models: Vec<TransactionWithEntriesModel>,
) -> anyhow::Result<Vec<Transaction>> {
    // Group entries by transaction_id while preserving the order of first appearance.
    // This is critical for cursor-based pagination: the DB returns rows ordered by
    // (date DESC, id DESC), and we must maintain that ordering after grouping.
    let mut order: Vec<Uuid> = Vec::new();
    let mut map: HashMap<Uuid, Vec<TransactionWithEntriesModel>> = HashMap::new();
    for model in models {
        let tid = model.transaction_id;
        if !map.contains_key(&tid) {
            order.push(tid);
        }
        map.entry(tid).or_default().push(model);
    }

    let grouped_results_full: Vec<Vec<TransactionWithEntriesModel>> = order
        .into_iter()
        .filter_map(|tid| map.remove(&tid))
        .collect();

    let transactions: Vec<Transaction> = grouped_results_full
        .into_iter()
        .map(create_transaction_from_transaction_with_entries_model)
        .collect::<Result<_, _>>()?;

    Ok(transactions)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use time::macros::datetime;
    use uuid::Uuid;

    use super::create_transaction_from_dto;
    use crate::dtos::entry_dto::EntryDto;
    use crate::dtos::transaction_dto::*;
    use crate::dtos::validation_error_dto::BusinessValidationErrorDto;
    use crate::test_support::load_dynamic_enums;

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const OTHER_ACCOUNT: Uuid = Uuid::from_u128(2);
    const GBP: i32 = 1;
    const VUSA: i32 = 2;

    fn entry(asset_id: i32, quantity: &str) -> EntryDto {
        EntryDto::new(asset_id, ACCOUNT, quantity.parse::<Decimal>().unwrap())
    }

    fn dto(transaction_type: TransactionTypeDto) -> TransactionDto {
        TransactionDto {
            transaction_id: None,
            date: datetime!(2026-01-02 0:00 UTC),
            visibility: TransactionVisibilityDto::Default,
            fee_entries: Vec::new(),
            transaction_type,
        }
    }

    fn rejected_fields(transaction_type: TransactionTypeDto) -> Vec<String> {
        load_dynamic_enums();
        let error = create_transaction_from_dto(dto(transaction_type), Uuid::nil())
            .err()
            .expect("the transaction is rejected");
        error
            .downcast_ref::<BusinessValidationErrorDto>()
            .expect("a validation error")
            .errors
            .iter()
            .map(|error| error.field.clone())
            .collect()
    }

    fn assert_accepted(transaction_type: TransactionTypeDto) {
        load_dynamic_enums();
        if let Err(error) = create_transaction_from_dto(dto(transaction_type), Uuid::nil()) {
            panic!(
                "the transaction is accepted: {error:?} {:?}",
                error.downcast_ref::<BusinessValidationErrorDto>()
            );
        }
    }

    #[test]
    fn single_entry_types_check_the_sign_the_api_requires() {
        assert_accepted(TransactionTypeDto::CashTransferIn(
            CashTransferInMetadataDto {
                entry: entry(GBP, "10"),
            },
        ));
        assert_accepted(TransactionTypeDto::Regular(RegularTransactionMetadataDto {
            description: None,
            entry: entry(GBP, "25"),
            category_id: 76,
        }));

        let rejected = [
            TransactionTypeDto::CashTransferIn(CashTransferInMetadataDto {
                entry: entry(GBP, "-10"),
            }),
            TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                entry: entry(GBP, "10"),
            }),
            TransactionTypeDto::CashDividend(CashDividendMetadataDto {
                entry: entry(GBP, "0"),
                origin_asset_id: VUSA,
            }),
            TransactionTypeDto::AssetDividend(AssetDividendMetadataDto {
                entry: entry(VUSA, "-1"),
            }),
            TransactionTypeDto::AssetTransferIn(AssetTransferInMetadataDto {
                entry: entry(VUSA, "-1"),
            }),
            TransactionTypeDto::AssetTransferOut(AssetTransferOutMetadataDto {
                entry: entry(VUSA, "1"),
            }),
            TransactionTypeDto::AccountFees(AccountFeesMetadataDto {
                entry: entry(GBP, "2"),
            }),
            TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: None,
                entry: entry(GBP, "0"),
                category_id: 76,
            }),
        ];
        for transaction_type in rejected {
            assert_eq!(rejected_fields(transaction_type), vec!["entry.amount"]);
        }
    }

    #[test]
    fn two_entry_types_check_signs_and_accounts() {
        assert_accepted(TransactionTypeDto::AssetPurchase(
            AssetPurchaseMetadataDto {
                purchase: entry(VUSA, "3"),
                sale: entry(GBP, "-300"),
            },
        ));
        assert_eq!(
            rejected_fields(TransactionTypeDto::AssetPurchase(
                AssetPurchaseMetadataDto {
                    purchase: entry(VUSA, "-3"),
                    sale: EntryDto::new(GBP, OTHER_ACCOUNT, "-300".parse().unwrap()),
                }
            )),
            vec![
                "purchase_change.amount",
                "purchase_change.account_id",
                "cash_outgoings_change.account_id"
            ]
        );
        assert_eq!(
            rejected_fields(TransactionTypeDto::AssetSale(AssetSaleMetadataDto {
                sale: entry(VUSA, "3"),
                proceeds: entry(GBP, "300"),
            })),
            vec!["sale_entry.amount"]
        );
        assert_eq!(
            rejected_fields(TransactionTypeDto::AssetTrade(AssetTradeMetadataDto {
                outgoing_entry: entry(GBP, "100"),
                incoming_entry: entry(VUSA, "1"),
            })),
            vec!["outgoing_entry.amount"]
        );
        assert_eq!(
            rejected_fields(TransactionTypeDto::AssetBalanceTransfer(
                AssetBalanceTransferMetadataDto {
                    outgoing_change: entry(VUSA, "-1"),
                    incoming_change: entry(VUSA, "-1"),
                }
            )),
            vec!["incoming_change.amount"]
        );
        assert_eq!(
            rejected_fields(TransactionTypeDto::CashBalanceTransfer(
                CashBalanceTransferMetadataDto {
                    outgoing_change: entry(GBP, "-100"),
                    incoming_change: entry(VUSA, "90"),
                }
            )),
            vec![
                "outgoing_change.asset_id",
                "incoming_change.asset_id",
                "outgoing_change.amount",
                "incoming_change.amount",
                "outgoing_change.account_id",
                "incoming_change.account_id"
            ]
        );
        assert_accepted(TransactionTypeDto::CashBalanceTransfer(
            CashBalanceTransferMetadataDto {
                outgoing_change: entry(GBP, "-100"),
                incoming_change: EntryDto::new(GBP, OTHER_ACCOUNT, "100".parse().unwrap()),
            },
        ));
    }
}
