use crate::models::classification::{ClassificationCategory, ClassifiedKind, MoneyDirection};

mod grouping;
mod llm;
mod system_one;

pub use llm::llm_classify;
pub use system_one::system_one_classify;

const KINDS: [(ClassifiedKind, &str); 7] = [
    (
        ClassifiedKind::Expense,
        "A purchase or payment for goods or services to a merchant, shop or biller: card payments, direct debits, subscriptions, bills. Money leaves the account.",
    ),
    (
        ClassifiedKind::Refund,
        "Money returned by a merchant for an earlier purchase: a refund, reversal, chargeback or cashback.",
    ),
    (
        ClassifiedKind::TransferOut,
        "Money sent to a person or to another account, including the account holder's own other accounts, savings pots, investment or crypto apps, or topping up another wallet. Not a purchase from a merchant.",
    ),
    (
        ClassifiedKind::TransferIn,
        "Money received from a person or from another account: salary or wages from an employer, payments from friends or family, money moved in from the holder's own accounts, top-ups.",
    ),
    (
        ClassifiedKind::BankFee,
        "A fee charged by the bank or card provider itself for the account, a payment, a transfer, a currency exchange or a card.",
    ),
    (
        ClassifiedKind::CurrencyExchange,
        "One side of converting money between currencies inside the account.",
    ),
    (
        ClassifiedKind::Interest,
        "Interest or a dividend paid into the account.",
    ),
];

fn kind_name(kind: ClassifiedKind) -> &'static str {
    match kind {
        ClassifiedKind::Expense => "expense",
        ClassifiedKind::Refund => "refund",
        ClassifiedKind::TransferOut => "transfer_out",
        ClassifiedKind::TransferIn => "transfer_in",
        ClassifiedKind::BankFee => "bank_fee",
        ClassifiedKind::CurrencyExchange => "currency_exchange",
        ClassifiedKind::Interest => "interest",
    }
}

fn fits(kind: ClassifiedKind, direction: MoneyDirection) -> bool {
    match kind {
        ClassifiedKind::Expense | ClassifiedKind::TransferOut | ClassifiedKind::BankFee => {
            direction == MoneyDirection::Out
        }
        ClassifiedKind::Refund | ClassifiedKind::TransferIn | ClassifiedKind::Interest => {
            direction == MoneyDirection::In
        }
        ClassifiedKind::CurrencyExchange => true,
    }
}

fn category_label(category: &ClassificationCategory) -> String {
    format!("{} › {}", category.type_name, category.name)
}
