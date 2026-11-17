#[cfg(feature = "backend")]
use business::dtos::imports::import_balance_dto::{
    ImportBalanceChangeDto, ImportBalanceCheckDto, ImportBalanceChecksDto,
    ImportEnterableBalanceDto, ImportUnlistedMovementDto,
};
use serde::{Deserialize, Serialize};
use time::{serde::rfc3339, OffsetDateTime};
use uuid::Uuid;

/// A period in which the difference from the statement moved.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportBalanceChangeViewModel {
    #[serde(with = "rfc3339")]
    pub after: OffsetDateTime,
    #[serde(with = "rfc3339")]
    pub through: OffsetDateTime,
    pub change: String,
    /// The difference from the statement at the end of the period.
    pub off_by: String,
    /// A ledger transaction in `unlisted` explains it. When none does, the items hold an
    /// `adjustment` row that would close it, unless `after` and `through` are the same moment:
    /// then two statements disagree about one balance, and no row can close that.
    pub explained: bool,
    /// A possible duplicate dated in the period is still open. Answering it may settle the
    /// difference, so no adjustment is offered until then.
    pub awaits_answer: bool,
}

/// A ledger transaction no bank feed or statement confirms, dated inside the statement's period
/// and matched by none of its rows.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportUnlistedMovementViewModel {
    pub transaction_id: Uuid,
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub quantity: String,
    pub description: Option<String>,
    /// Dated in the statement's last days: the bank may book it on the next statement, so it is
    /// not part of the difference yet.
    pub may_book_later: bool,
    /// Its amount, or the amounts of all of them between the same two balances, is how much the
    /// difference moved there.
    pub explains_difference: bool,
    /// Answer `not_on_statement` with this key to delete its transaction when the import is
    /// applied. The parts of one split payment share a key. Absent while something vouches for the
    /// transaction on another account, or while it may still book on the next statement.
    pub ask_key: Option<String>,
    /// Will be deleted on apply, and no longer counts here.
    pub deleting: bool,
    /// Other accounts its transaction moves money on; deleting it removes those too.
    pub other_account_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportBalanceCheckViewModel {
    pub account_id: Uuid,
    pub asset_id: i32,
    #[serde(with = "rfc3339")]
    pub first_date: OffsetDateTime,
    #[serde(with = "rfc3339")]
    pub last_date: OffsetDateTime,
    pub balances_checked: u32,
    /// The difference at the first balance: what the ledger holds by then that the statement's
    /// rows and its starting balance don't account for.
    pub starting_off_by: String,
    /// The difference at the last balance, with this proposal applied.
    pub final_off_by: String,
    pub changes: Vec<ImportBalanceChangeViewModel>,
    /// In the ledger but not on the statement: the usual reason a balance is off.
    pub unlisted: Vec<ImportUnlistedMovementViewModel>,
}

/// A balance the user entered for a position no file states one for. It is checked like a printed
/// closing balance; answering its `no_checkpoint` ask again replaces it.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportEnteredBalanceViewModel {
    pub ask_key: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    /// The day the balance was entered for.
    #[serde(with = "rfc3339")]
    pub date: OffsetDateTime,
    pub quantity: String,
    /// The day a new answer is for: the position's last row now.
    #[serde(with = "rfc3339")]
    pub as_of_date: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImportBalanceChecksViewModel {
    pub checks: Vec<ImportBalanceCheckViewModel>,
    /// Accounts and assets that were compared with nothing: rows without a stated balance, or a
    /// stated figure that can't be compared, such as a market value.
    pub unchecked: u32,
    pub entered: Vec<ImportEnteredBalanceViewModel>,
}

#[cfg(feature = "backend")]
impl From<ImportBalanceChangeDto> for ImportBalanceChangeViewModel {
    fn from(value: ImportBalanceChangeDto) -> Self {
        Self {
            after: value.after.midnight().assume_utc(),
            through: value.through.midnight().assume_utc(),
            change: value.change.to_string(),
            off_by: value.off_by.to_string(),
            explained: value.explained,
            awaits_answer: value.awaits_answer,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportUnlistedMovementDto> for ImportUnlistedMovementViewModel {
    fn from(value: ImportUnlistedMovementDto) -> Self {
        Self {
            transaction_id: value.transaction_id,
            date: value.date.midnight().assume_utc(),
            quantity: value.quantity.to_string(),
            description: value.description,
            may_book_later: value.may_book_later,
            explains_difference: value.explains_difference,
            ask_key: value.ask_key,
            deleting: value.deleting,
            other_account_ids: value.other_account_ids,
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportBalanceCheckDto> for ImportBalanceCheckViewModel {
    fn from(value: ImportBalanceCheckDto) -> Self {
        Self {
            account_id: value.account_id,
            asset_id: value.asset_id,
            first_date: value.first_date.midnight().assume_utc(),
            last_date: value.last_date.midnight().assume_utc(),
            balances_checked: value.balances_checked as u32,
            starting_off_by: value.starting_off_by.to_string(),
            final_off_by: value.final_off_by.to_string(),
            changes: value.changes.into_iter().map(Into::into).collect(),
            unlisted: value.unlisted.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ImportBalanceChecksDto> for ImportBalanceChecksViewModel {
    fn from(value: ImportBalanceChecksDto) -> Self {
        Self {
            checks: value.checks.into_iter().map(Into::into).collect(),
            unchecked: value.unchecked as u32,
            entered: value
                .enterable
                .into_iter()
                .filter_map(|enterable: ImportEnterableBalanceDto| {
                    let entered = enterable.entered?;
                    Some(ImportEnteredBalanceViewModel {
                        ask_key: enterable.ask_key,
                        account_id: enterable.account_id,
                        asset_id: enterable.asset_id,
                        date: entered.date.midnight().assume_utc(),
                        quantity: entered.quantity.to_string(),
                        as_of_date: enterable.as_of_date.midnight().assume_utc(),
                    })
                })
                .collect(),
        }
    }
}
