use std::collections::HashMap;

use dal::enums::{
    account_types::DatabaseAccountTypes, fee_categories::DatabaseFeeCategories,
    transaction_type_categories::DatabaseTransactionTypeCategories,
};

use crate::dynamic_enums::{
    account_types::AccountTypes, fee_categories::FeeCategories,
    transaction_type_categories::TransactionTypeCategories, DynamicEnum,
};

/// Category id the helper assigns to Cash Balance Transfer. The real id is
/// sequence-allocated by the migration, so tests must go through this constant
/// rather than assuming a literal.
pub(crate) const CASH_BALANCE_TRANSFER_CATEGORY: i32 = 15;

/// database. The statics are process-global and shared by parallel tests:
/// every caller installs identical values and nothing tears them down, which
/// keeps concurrent test execution safe.
pub(crate) fn load_dynamic_enums() {
    let mut categories = HashMap::new();
    categories.insert(DatabaseTransactionTypeCategories::AssetPurchase, 3);
    categories.insert(DatabaseTransactionTypeCategories::AssetSale, 4);
    categories.insert(DatabaseTransactionTypeCategories::CashTransferIn, 5);
    categories.insert(DatabaseTransactionTypeCategories::CashDividend, 6);
    categories.insert(DatabaseTransactionTypeCategories::AssetDividend, 7);
    categories.insert(DatabaseTransactionTypeCategories::CashTransferOut, 9);
    categories.insert(DatabaseTransactionTypeCategories::AssetTransferOut, 10);
    categories.insert(DatabaseTransactionTypeCategories::AssetTransferIn, 11);
    categories.insert(DatabaseTransactionTypeCategories::AssetTrade, 12);
    categories.insert(DatabaseTransactionTypeCategories::AssetBalanceTransfer, 13);
    categories.insert(DatabaseTransactionTypeCategories::AccountFees, 14);
    categories.insert(
        DatabaseTransactionTypeCategories::CashBalanceTransfer,
        CASH_BALANCE_TRANSFER_CATEGORY,
    );
    TransactionTypeCategories::set_static_map(Some(categories));

    let mut fees = HashMap::new();
    fees.insert(DatabaseFeeCategories::Transaction, 2);
    fees.insert(DatabaseFeeCategories::Exchange, 1);
    fees.insert(DatabaseFeeCategories::WithholdingTax, 8);
    FeeCategories::set_static_map(Some(fees));

    let mut account_types = HashMap::new();
    account_types.insert(DatabaseAccountTypes::Credit, 4);
    account_types.insert(DatabaseAccountTypes::Mortgage, 7);
    account_types.insert(DatabaseAccountTypes::Loan, 8);
    account_types.insert(DatabaseAccountTypes::Investment, 3);
    account_types.insert(DatabaseAccountTypes::PersonalPension, 5);
    account_types.insert(DatabaseAccountTypes::WorkplacePension, 6);
    account_types.insert(DatabaseAccountTypes::CryptoWallet, 10);
    AccountTypes::set_static_map(Some(account_types));
}
