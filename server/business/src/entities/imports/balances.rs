use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use dal::models::import_models::{ImportAccountMappingModel, ImportAssetMappingModel};
use rust_decimal::Decimal;
use time::{Date, Duration};
use uuid::Uuid;

use crate::dtos::imports::import_balance_dto::{
    ImportAdjustmentDto, ImportAssertionDto, ImportAssertionKindDto, ImportBalanceChangeDto,
    ImportBalanceCheckDto, ImportBalanceChecksDto, ImportEnterableBalanceDto,
    ImportEnteredBalanceDto, ImportStartingBalanceDto, ImportUnlistedMovementDto,
};
use crate::dtos::imports::import_staging_dto::{
    ImportClaimDto, ImportProposalRowDto, ImportRowKindDto, ImportVerbDto,
};
use crate::dtos::ledger_matching_dto::{LedgerActivityDto, LedgerMovementDto, LedgerTotalDto};
use crate::entities::ledger_matching::ledger_roots;

const UNCONFIRMED_BOOKING_DAYS: i64 = 3;

type Position = (Uuid, i32);
type LedgerKey = (Uuid, Uuid, i32);
type Period = (Date, Date);

struct StatedBalance {
    balance_at: Date,
    is_opening: bool,
    quantity: Decimal,
    file_id: Uuid,
}

struct Stated {
    by_position: BTreeMap<Position, Vec<StatedBalance>>,
    unchecked: usize,
    periods: HashMap<Position, Vec<Period>>,
    enterable: Vec<ImportEnterableBalanceDto>,
}

struct LedgerMovement {
    quantity: Decimal,
    date: Date,
    booked_on: Option<Date>,
    bank_fed: bool,
    states_position: bool,
    deleting: bool,
}

struct CountedMovement {
    counted_on: Date,
    date: Date,
    quantity: Decimal,
    listed_by_a_row: bool,
    states_position: bool,
}

pub struct SessionBalances {
    assertions: Vec<ImportAssertionDto>,
    entered: Vec<ImportEnteredBalanceDto>,
    accounts: HashMap<(Uuid, String), Option<Uuid>>,
    assets: HashMap<String, Option<i32>>,
    rows: BTreeMap<Position, BTreeMap<Date, Decimal>>,
    confirmed: Vec<ImportClaimDto>,
    replaced: Vec<ImportClaimDto>,
    asked_about: HashSet<Uuid>,
    asked_on: Vec<(Position, Date)>,
    deleted: HashSet<Uuid>,
    row_dates: HashMap<(Uuid, Position), Period>,
}

impl SessionBalances {
    pub fn new(assertions: Vec<ImportAssertionDto>) -> Self {
        Self {
            assertions,
            entered: Vec::new(),
            accounts: HashMap::new(),
            assets: HashMap::new(),
            rows: BTreeMap::new(),
            confirmed: Vec::new(),
            replaced: Vec::new(),
            asked_about: HashSet::new(),
            asked_on: Vec::new(),
            deleted: HashSet::new(),
            row_dates: HashMap::new(),
        }
    }

    pub fn add_entered_balances(&mut self, entered: Vec<ImportEnteredBalanceDto>) {
        self.entered.extend(entered);
    }

    pub fn add_account_mappings(&mut self, mappings: &[ImportAccountMappingModel]) {
        self.accounts.extend(mappings.iter().map(|mapping| {
            (
                (mapping.file_id, mapping.account_ref.clone()),
                mapping.account_id,
            )
        }));
    }

    pub fn add_asset_mappings(&mut self, mappings: &[ImportAssetMappingModel]) {
        self.assets.extend(
            mappings
                .iter()
                .map(|mapping| (mapping.asset_ref.clone(), mapping.asset_id)),
        );
    }

    pub fn add_rows<'a>(&mut self, rows: impl IntoIterator<Item = &'a ImportProposalRowDto>) {
        for row in rows {
            for movement in &row.movements {
                widen(
                    &mut self.row_dates,
                    movement.file_id,
                    (movement.account_id, movement.asset_id),
                    movement.date,
                );
            }
            for near in &row.near_miss {
                let in_ledger = std::iter::once(&near.candidate)
                    .filter(|candidate| candidate.in_ledger)
                    .chain(&near.parts);
                for candidate in in_ledger {
                    self.asked_about.insert(candidate.id);
                    if let Some(account_id) = candidate.account_id {
                        self.asked_on
                            .push(((account_id, candidate.asset_id), candidate.date));
                    }
                }
            }
            if !row.near_miss.is_empty() {
                self.asked_on.extend(
                    row.movements
                        .iter()
                        .map(|movement| ((movement.account_id, movement.asset_id), movement.date)),
                );
            }
            let mut count = |position: Position, date: Date, quantity: Decimal| {
                *self
                    .rows
                    .entry(position)
                    .or_default()
                    .entry(date)
                    .or_default() += quantity;
            };
            match (row.kind, row.verb) {
                (ImportRowKindDto::StartingBalance(starting), ImportVerbDto::Add) => count(
                    (starting.account_id, starting.asset_id),
                    starting.date,
                    starting.quantity,
                ),
                (ImportRowKindDto::StartingBalance(_), _) => {}
                (ImportRowKindDto::Adjustment(adjustment), ImportVerbDto::Add) => count(
                    (adjustment.account_id, adjustment.asset_id),
                    adjustment.date,
                    adjustment.quantity,
                ),
                (ImportRowKindDto::Adjustment(_), _) => {}
                (
                    ImportRowKindDto::Movement | ImportRowKindDto::JoinedTransfer(_),
                    ImportVerbDto::Skip,
                ) => self.confirmed.extend(&row.claims),
                (
                    ImportRowKindDto::Movement | ImportRowKindDto::JoinedTransfer(_),
                    ImportVerbDto::Delete,
                ) => self.deleted.extend(row.transaction.transaction_id),
                (
                    ImportRowKindDto::Movement | ImportRowKindDto::JoinedTransfer(_),
                    ImportVerbDto::Add | ImportVerbDto::Amend,
                ) => {
                    self.replaced.extend(&row.claims);
                    for movement in &row.movements {
                        count(
                            (movement.account_id, movement.asset_id),
                            movement.date,
                            movement.quantity,
                        );
                    }
                }
            }
        }
    }

    fn position(&self, assertion: &ImportAssertionDto) -> Option<Position> {
        if assertion.kind == ImportAssertionKindDto::ValueTotal {
            return None;
        }
        self.linked_position(assertion)
    }

    fn linked_position(&self, assertion: &ImportAssertionDto) -> Option<Position> {
        let account_id = (*self
            .accounts
            .get(&(assertion.file_id, assertion.account_ref.clone()))?)?;
        let asset_id = (*self.assets.get(assertion.asset_ref.as_ref()?)?)?;
        Some((account_id, asset_id))
    }

    fn is_left_out(&self, assertion: &ImportAssertionDto) -> bool {
        let account_left_out = matches!(
            self.accounts
                .get(&(assertion.file_id, assertion.account_ref.clone())),
            Some(None)
        );
        let asset_left_out = assertion
            .asset_ref
            .as_ref()
            .is_some_and(|asset_ref| matches!(self.assets.get(asset_ref), Some(None)));
        assertion.kind != ImportAssertionKindDto::ValueTotal && (account_left_out || asset_left_out)
    }

    pub fn date_range(&self) -> Option<(Date, Date)> {
        let dates = || {
            self.assertions
                .iter()
                .filter(|assertion| self.position(assertion).is_some())
                .map(balance_at)
                .chain(
                    self.row_dates
                        .values()
                        .flat_map(|(first, last)| [*first, *last]),
                )
        };
        dates().min().zip(dates().max())
    }

    fn stated(&self) -> Stated {
        let mut printed: BTreeMap<(Position, Uuid), BTreeMap<&str, Vec<&ImportAssertionDto>>> =
            BTreeMap::new();
        let mut unchecked: BTreeSet<Position> = BTreeSet::new();
        let mut unchecked_refs: BTreeSet<(&str, Option<&str>)> = BTreeSet::new();
        let mut periods = self.row_dates.clone();
        for assertion in &self.assertions {
            match self.position(assertion) {
                Some(position) => {
                    widen(&mut periods, assertion.file_id, position, assertion.date);
                    printed
                        .entry((position, assertion.file_id))
                        .or_default()
                        .entry(assertion.account_ref.as_str())
                        .or_default()
                        .push(assertion);
                }
                None if self.is_left_out(assertion) => {}
                None => match self.linked_position(assertion) {
                    Some(position) => {
                        unchecked.insert(position);
                    }
                    None => {
                        unchecked_refs.insert((
                            assertion.account_ref.as_str(),
                            assertion.asset_ref.as_deref(),
                        ));
                    }
                },
            }
        }

        let mut by_position: BTreeMap<Position, Vec<StatedBalance>> = BTreeMap::new();
        for ((position, file_id), printed_accounts) in printed {
            let mut stated: Vec<StatedBalance> = Vec::new();
            if printed_accounts.len() == 1 {
                stated.extend(
                    printed_accounts
                        .values()
                        .flatten()
                        .map(|assertion| StatedBalance {
                            balance_at: balance_at(assertion),
                            is_opening: assertion.kind == ImportAssertionKindDto::Opening,
                            quantity: assertion.quantity,
                            file_id,
                        }),
                );
            } else {
                let mut moments: BTreeMap<Date, (Decimal, bool, usize)> = BTreeMap::new();
                for assertions in printed_accounts.values() {
                    let mut own: BTreeMap<Date, Option<&ImportAssertionDto>> = BTreeMap::new();
                    for assertion in assertions {
                        own.entry(balance_at(assertion))
                            .and_modify(|stated| {
                                if stated.is_some_and(|other| other.quantity != assertion.quantity)
                                {
                                    *stated = None;
                                }
                            })
                            .or_insert(Some(assertion));
                    }
                    for (at, assertion) in own {
                        if let Some(assertion) = assertion {
                            let (sum, all_opening, parts) =
                                moments.entry(at).or_insert((Decimal::ZERO, true, 0));
                            *sum += assertion.quantity;
                            *all_opening &= assertion.kind == ImportAssertionKindDto::Opening;
                            *parts += 1;
                        }
                    }
                }
                stated.extend(
                    moments
                        .into_iter()
                        .filter(|(_, (_, _, parts))| *parts == printed_accounts.len())
                        .map(|(balance_at, (quantity, is_opening, _))| StatedBalance {
                            balance_at,
                            is_opening,
                            quantity,
                            file_id,
                        }),
                );
            }
            if stated.is_empty() {
                unchecked.insert(position);
            } else {
                by_position.entry(position).or_default().extend(stated);
            }
        }

        let mut moved: BTreeMap<Position, ((Date, Uuid), Date)> = BTreeMap::new();
        for ((file_id, position), (first, last)) in &self.row_dates {
            let (first_row, last_row) = moved
                .entry(*position)
                .or_insert(((*first, *file_id), *last));
            *first_row = (*first_row).min((*first, *file_id));
            *last_row = (*last_row).max(*last);
        }
        let mut enterable = Vec::new();
        for (position, ((_, first_file), last_row)) in moved {
            if by_position.contains_key(&position) {
                continue;
            }
            let entered = self
                .entered
                .iter()
                .find(|entered| (entered.account_id, entered.asset_id) == position)
                .copied();
            if let Some(entered) = entered {
                by_position.insert(
                    position,
                    vec![StatedBalance {
                        balance_at: entered.date,
                        is_opening: false,
                        quantity: entered.quantity,
                        file_id: first_file,
                    }],
                );
            }
            enterable.push(ImportEnterableBalanceDto {
                ask_key: balance_ask_key(position.0, position.1),
                account_id: position.0,
                asset_id: position.1,
                as_of_date: last_row,
                entered,
            });
        }
        for stated in by_position.values_mut() {
            stated.sort_by_key(|balance| {
                (
                    balance.balance_at,
                    balance.is_opening,
                    balance.quantity,
                    balance.file_id,
                )
            });
            stated.dedup_by_key(|balance| (balance.balance_at, balance.quantity));
        }

        unchecked.extend(self.row_dates.keys().map(|(_, position)| *position));
        unchecked.retain(|position| !by_position.contains_key(position));

        let mut statement_periods: HashMap<Position, Vec<Period>> = HashMap::new();
        for ((_, position), period) in periods {
            statement_periods.entry(position).or_default().push(period);
        }
        Stated {
            by_position,
            unchecked: unchecked.len() + unchecked_refs.len(),
            periods: statement_periods,
            enterable,
        }
    }

    pub fn starting_balances(
        &self,
        totals: &[LedgerTotalDto],
        activity: &LedgerActivityDto,
    ) -> Vec<ImportStartingBalanceDto> {
        let Stated {
            by_position,
            periods,
            ..
        } = self.stated();
        let (ledger, _) = ledger_by_booking_date(
            activity,
            &self.confirmed,
            &self.replaced,
            &self.asked_about,
            &self.deleted,
            &periods,
        );

        let mut starting_balances = Vec::new();
        for (position, stated) in &by_position {
            let first = &stated[0];
            if stated
                .get(1)
                .is_some_and(|next| next.balance_at == first.balance_at)
            {
                continue;
            }
            let first_row = self
                .row_dates
                .iter()
                .filter(|((file_id, moved), _)| *file_id == first.file_id || moved == position)
                .map(|(_, (starts, _))| *starts)
                .min();
            let date = first_row.map_or(first.balance_at, |row| {
                (row - Duration::days(1)).min(first.balance_at)
            });
            let printed_on = if first.is_opening {
                first.balance_at + Duration::days(1)
            } else {
                first.balance_at
            };
            let ledger = ledger.get(position).map(Vec::as_slice).unwrap_or_default();
            let before_window: Decimal = totals
                .iter()
                .filter(|total| (total.account_id, total.asset_id) == *position)
                .map(|total| total.quantity)
                .sum();
            let held: Decimal = ledger
                .iter()
                .filter(|movement| {
                    if movement.listed_by_a_row {
                        movement.counted_on <= first.balance_at
                    } else if movement.states_position {
                        movement.date <= date
                    } else {
                        movement.date <= printed_on
                    }
                })
                .map(|movement| movement.quantity)
                .sum();
            let rows: Decimal = self
                .rows
                .get(position)
                .into_iter()
                .flat_map(|rows| rows.range(..=first.balance_at))
                .map(|(_, quantity)| *quantity)
                .sum();
            let quantity = (first.quantity - rows - before_window - held).normalize();
            if quantity.is_zero() {
                continue;
            }
            starting_balances.push(ImportStartingBalanceDto {
                account_id: position.0,
                asset_id: position.1,
                file_id: first.file_id,
                date,
                quantity,
            });
        }
        starting_balances
    }

    pub fn check(
        &self,
        totals: &[LedgerTotalDto],
        activity: &LedgerActivityDto,
    ) -> ImportBalanceChecksDto {
        let Stated {
            by_position,
            unchecked,
            periods,
            enterable,
        } = self.stated();
        let opening: HashMap<Position, Decimal> = totals
            .iter()
            .map(|total| ((total.account_id, total.asset_id), total.quantity))
            .collect();
        let (ledger, mut unlisted) = ledger_by_booking_date(
            activity,
            &self.confirmed,
            &self.replaced,
            &self.asked_about,
            &self.deleted,
            &periods,
        );

        let mut checks = Vec::new();
        for (position, stated) in by_position {
            let ledger = ledger.get(&position).map(Vec::as_slice).unwrap_or_default();
            let mut activity: BTreeMap<Date, Decimal> =
                self.rows.get(&position).cloned().unwrap_or_default();
            for movement in ledger {
                *activity.entry(movement.counted_on).or_default() += movement.quantity;
            }
            let mut activity = activity.into_iter().peekable();
            let mut running = opening.get(&position).copied().unwrap_or_default();
            let mut unlisted = unlisted.remove(&position).unwrap_or_default();
            unlisted.sort_by_key(|(counted_on, movement)| {
                (*counted_on, movement.date, movement.transaction_id)
            });
            let mut unlisted_before = 0;

            let mut changes = Vec::new();
            let mut previous: Option<(Date, Decimal)> = None;
            let mut starting_off_by = Decimal::ZERO;
            for balance in &stated {
                while let Some((_, quantity)) =
                    activity.next_if(|(date, _)| *date <= balance.balance_at)
                {
                    running += quantity;
                }
                let first_day = balance.balance_at + Duration::days(1);
                let before_first_row: Decimal = if balance.is_opening {
                    ledger
                        .iter()
                        .filter(|movement| {
                            !movement.listed_by_a_row
                                && !movement.states_position
                                && movement.counted_on > balance.balance_at
                                && movement.date <= first_day
                        })
                        .map(|movement| movement.quantity)
                        .sum()
                } else {
                    Decimal::ZERO
                };
                let off_by = running + before_first_row - balance.quantity;
                let moved = off_by - previous.map_or(Decimal::ZERO, |(_, earlier)| earlier);
                let counted_through = if balance.is_opening {
                    first_day
                } else {
                    balance.balance_at
                };
                let unlisted_through = unlisted
                    .partition_point(|(counted_on, _)| *counted_on <= counted_through)
                    .max(unlisted_before);
                let counted_here = &mut unlisted[unlisted_before..unlisted_through];
                let together: Decimal = counted_here
                    .iter()
                    .filter(|(_, movement)| !movement.deleting)
                    .map(|(_, movement)| movement.quantity)
                    .sum();
                let mut explained = false;
                for (_, movement) in counted_here
                    .iter_mut()
                    .filter(|(_, movement)| !movement.deleting)
                {
                    movement.explains_difference =
                        !moved.is_zero() && (movement.quantity == moved || together == moved);
                    explained |= movement.explains_difference;
                }
                unlisted_before = unlisted_through;
                match previous {
                    None => starting_off_by = off_by,
                    Some((after, _)) if !moved.is_zero() => {
                        let awaits_answer = self.asked_on.iter().any(|(at, date)| {
                            *at == position && after < *date && *date <= counted_through
                        });
                        changes.push(ImportBalanceChangeDto {
                            after,
                            through: balance.balance_at,
                            change: moved.normalize(),
                            off_by: off_by.normalize(),
                            explained,
                            awaits_answer,
                        });
                    }
                    Some(_) => {}
                }
                previous = Some((balance.balance_at, off_by));
            }

            let (last_date, final_off_by) =
                previous.expect("a position only exists with a stated balance");
            checks.push(ImportBalanceCheckDto {
                account_id: position.0,
                asset_id: position.1,
                first_date: stated[0].balance_at,
                last_date,
                balances_checked: stated.len(),
                starting_off_by: starting_off_by.normalize(),
                final_off_by: final_off_by.normalize(),
                changes,
                unlisted: unlisted.into_iter().map(|(_, movement)| movement).collect(),
            });
        }
        ImportBalanceChecksDto {
            checks,
            unchecked,
            enterable,
        }
    }
}

pub fn balance_ask_key(account_id: Uuid, asset_id: i32) -> String {
    format!("no_checkpoint:{account_id}:{asset_id}")
}

pub fn not_on_statement_ask_key(root: Uuid) -> String {
    format!("not_on_statement:{root}")
}

pub fn adjustments(checks: &ImportBalanceChecksDto) -> Vec<ImportAdjustmentDto> {
    checks
        .checks
        .iter()
        .flat_map(|check| {
            check
                .changes
                .iter()
                .filter(|change| {
                    !change.explained && !change.awaits_answer && change.after != change.through
                })
                .map(|change| ImportAdjustmentDto {
                    account_id: check.account_id,
                    asset_id: check.asset_id,
                    date: change.through,
                    quantity: -change.change,
                })
        })
        .collect()
}

fn balance_at(assertion: &ImportAssertionDto) -> Date {
    if assertion.kind == ImportAssertionKindDto::Opening {
        assertion.date - Duration::days(1)
    } else {
        assertion.date
    }
}

fn widen(
    periods: &mut HashMap<(Uuid, Position), Period>,
    file_id: Uuid,
    position: Position,
    date: Date,
) {
    let (first, last) = periods.entry((file_id, position)).or_insert((date, date));
    *first = (*first).min(date);
    *last = (*last).max(date);
}

fn ledger_by_booking_date(
    activity: &LedgerActivityDto,
    confirmed: &[ImportClaimDto],
    replaced: &[ImportClaimDto],
    asked_about: &HashSet<Uuid>,
    deleted: &HashSet<Uuid>,
    statement_periods: &HashMap<Position, Vec<Period>>,
) -> (
    HashMap<Position, Vec<CountedMovement>>,
    HashMap<Position, Vec<(Date, ImportUnlistedMovementDto)>>,
) {
    let roots = ledger_roots(&activity.movements, &activity.sources);
    let mut any_root: HashMap<Uuid, Uuid> = HashMap::new();
    for ((transaction_id, _, _), root) in &roots {
        any_root
            .entry(*transaction_id)
            .and_modify(|lowest| *lowest = (*lowest).min(*root))
            .or_insert(*root);
    }
    let key_of_movement = |movement: &LedgerMovementDto| -> LedgerKey {
        (
            roots[&(
                movement.transaction_id,
                movement.account_id,
                movement.asset_id,
            )],
            movement.account_id,
            movement.asset_id,
        )
    };
    let key_of = |claim: &ImportClaimDto| -> Option<LedgerKey> {
        let root = roots
            .get(&(claim.transaction_id, claim.account_id, claim.asset_id))
            .or_else(|| any_root.get(&claim.transaction_id))?;
        Some((*root, claim.account_id, claim.asset_id))
    };
    let replaced: HashSet<LedgerKey> = replaced.iter().filter_map(key_of).collect();
    let mut booked_by_statement: HashMap<LedgerKey, Date> = HashMap::new();
    for claim in confirmed {
        if let Some(key) = key_of(claim) {
            booked_by_statement
                .entry(key)
                .and_modify(|date| *date = (*date).min(claim.booked_on))
                .or_insert(claim.booked_on);
        }
    }
    let asked_about: HashSet<Uuid> = asked_about
        .iter()
        .map(|transaction_id| *any_root.get(transaction_id).unwrap_or(transaction_id))
        .collect();

    let mut movements: HashMap<LedgerKey, LedgerMovement> = HashMap::new();
    for movement in &activity.movements {
        let entry = movements
            .entry(key_of_movement(movement))
            .or_insert(LedgerMovement {
                quantity: Decimal::ZERO,
                date: movement.date,
                booked_on: None,
                bank_fed: false,
                states_position: false,
                deleting: false,
            });
        entry.quantity += movement.quantity;
        entry.date = entry.date.min(movement.date);
        entry.bank_fed |= !movement.connector_binding_ids.is_empty();
        entry.deleting |= deleted.contains(&movement.transaction_id);
    }
    for source in &activity.sources {
        let traits = source.kind.traits();
        let Some(entry) = roots
            .get(&(source.transaction_id, source.account_id, source.asset_id))
            .and_then(|root| movements.get_mut(&(*root, source.account_id, source.asset_id)))
        else {
            continue;
        };
        if traits.proves_movement {
            entry.booked_on = Some(
                entry
                    .booked_on
                    .map_or(source.date, |booked| booked.min(source.date)),
            );
        }
        entry.states_position |= traits.states_position;
    }

    let mut by_position: HashMap<Position, Vec<CountedMovement>> = HashMap::new();
    let mut unlisted_keys: HashMap<LedgerKey, (Date, bool)> = HashMap::new();
    for (key, movement) in &movements {
        if replaced.contains(key) {
            continue;
        }
        let covered = |date: Date| {
            statement_periods
                .get(&(key.1, key.2))
                .into_iter()
                .flatten()
                .any(|(first, last)| (*first..=*last).contains(&date))
        };
        let settled = movement.bank_fed || movement.states_position;
        let may_book_on = movement.date + Duration::days(UNCONFIRMED_BOOKING_DAYS);
        let may_book_later = !settled && !covered(may_book_on);
        let listed_on = booked_by_statement.get(key).copied();
        let booked_on = listed_on.or(movement.booked_on);
        let counted_on = booked_on.unwrap_or(if may_book_later {
            may_book_on
        } else {
            movement.date
        });
        if booked_on.is_none()
            && !movement.bank_fed
            && covered(movement.date)
            && !asked_about.contains(&key.0)
        {
            unlisted_keys.insert(*key, (counted_on, may_book_later));
        }
        if movement.deleting {
            continue;
        }
        by_position
            .entry((key.1, key.2))
            .or_default()
            .push(CountedMovement {
                counted_on,
                date: movement.date,
                quantity: movement.quantity,
                listed_by_a_row: listed_on.is_some(),
                states_position: movement.states_position,
            });
    }

    let mut keys_by_root: HashMap<Uuid, Vec<LedgerKey>> = HashMap::new();
    for key in movements.keys() {
        keys_by_root.entry(key.0).or_default().push(*key);
    }
    let elsewhere = |key: LedgerKey| {
        keys_by_root[&key.0]
            .iter()
            .filter(move |other| **other != key)
    };
    let deletable = |key: LedgerKey| {
        elsewhere(key).all(|other| {
            let movement = &movements[other];
            !replaced.contains(other)
                && !booked_by_statement.contains_key(other)
                && movement.booked_on.is_none()
                && !movement.bank_fed
                && !movement.states_position
        })
    };

    let mut unlisted: HashMap<Position, Vec<(Date, ImportUnlistedMovementDto)>> = HashMap::new();
    for movement in &activity.movements {
        let key = key_of_movement(movement);
        if let Some((counted_on, may_book_later)) = unlisted_keys.get(&key) {
            let mut other_account_ids: Vec<Uuid> = elsewhere(key)
                .map(|other| other.1)
                .filter(|account_id| *account_id != key.1)
                .collect();
            other_account_ids.sort();
            other_account_ids.dedup();
            unlisted.entry((key.1, key.2)).or_default().push((
                *counted_on,
                ImportUnlistedMovementDto {
                    transaction_id: movement.transaction_id,
                    date: movement.date,
                    quantity: movement.quantity.normalize(),
                    description: movement.description.clone(),
                    may_book_later: *may_book_later,
                    explains_difference: false,
                    ask_key: (!may_book_later && deletable(key))
                        .then(|| not_on_statement_ask_key(key.0)),
                    deleting: movements[&key].deleting,
                    other_account_ids,
                },
            ));
        }
    }
    (by_position, unlisted)
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};

    use super::*;
    use crate::dtos::entry_dto::EntryDto;
    use crate::dtos::imports::import_staging_dto::{ImportNearMissCandidateDto, StagedMovementDto};
    use crate::dtos::ledger_matching_dto::{
        LedgerMovementDto, MatchCandidateDto, SourceKindDto, TransactionSourceDto,
    };
    use crate::dtos::transaction_dto::{
        CashTransferOutMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
    };

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const FILE: Uuid = Uuid::from_u128(2);
    const NEXT_FILE: Uuid = Uuid::from_u128(3);
    const GBP: i32 = 45;
    const USD: i32 = 46;

    fn assertion(kind: ImportAssertionKindDto, on: Date, quantity: Decimal) -> ImportAssertionDto {
        ImportAssertionDto {
            file_id: FILE,
            account_ref: "Current".to_string(),
            asset_ref: Some("GBP".to_string()),
            kind,
            date: on,
            quantity,
        }
    }

    fn opening(on: Date, quantity: Decimal) -> ImportAssertionDto {
        assertion(ImportAssertionKindDto::Opening, on, quantity)
    }

    fn closing(on: Date, quantity: Decimal) -> ImportAssertionDto {
        assertion(ImportAssertionKindDto::Closing, on, quantity)
    }

    fn snapshot(on: Date, quantity: Decimal) -> ImportAssertionDto {
        assertion(ImportAssertionKindDto::Snapshot, on, quantity)
    }

    fn in_next_file(mut assertion: ImportAssertionDto) -> ImportAssertionDto {
        assertion.file_id = NEXT_FILE;
        assertion
    }

    fn row(
        verb: ImportVerbDto,
        on: Date,
        quantity: Decimal,
        claims: &[u128],
    ) -> ImportProposalRowDto {
        ImportProposalRowDto {
            id: Uuid::now_v7(),
            kind: ImportRowKindDto::Movement,
            file_ids: vec![FILE],
            ask_key: None,
            verb,
            verb_locked: false,
            transaction: TransactionDto {
                transaction_id: None,
                date: datetime!(2026-10-01 0:00 UTC),
                visibility: TransactionVisibilityDto::Default,
                fee_entries: Vec::new(),
                transaction_type: TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                    entry: EntryDto::new(GBP, ACCOUNT, dec!(-1)),
                }),
            },
            near_miss: Vec::new(),
            movements: vec![StagedMovementDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                quantity,
                file_id: FILE,
                date: on,
                description: None,
                itemised_fee: None,
            }],
            claims: claims
                .iter()
                .map(|id| ImportClaimDto {
                    transaction_id: Uuid::from_u128(*id),
                    account_id: ACCOUNT,
                    asset_id: GBP,
                    booked_on: on,
                })
                .collect(),
            recorded_on: Vec::new(),
            amend_refusal: None,
            parts: None,
        }
    }

    fn add(on: Date, quantity: Decimal) -> ImportProposalRowDto {
        row(ImportVerbDto::Add, on, quantity, &[])
    }

    fn movement(id: u128, on: Date, quantity: Decimal) -> LedgerMovementDto {
        LedgerMovementDto {
            transaction_id: Uuid::from_u128(id),
            group_id: None,
            date: on,
            account_id: ACCOUNT,
            asset_id: GBP,
            quantity,
            description: None,
            connector_binding_ids: Vec::new(),
        }
    }

    fn balances(
        assertions: Vec<ImportAssertionDto>,
        rows: Vec<ImportProposalRowDto>,
    ) -> SessionBalances {
        let mut balances = SessionBalances::new(assertions);
        balances.add_account_mappings(&[FILE, NEXT_FILE].map(|file_id| {
            ImportAccountMappingModel {
                file_id,
                account_ref: "Current".to_string(),
                account_id: Some(ACCOUNT),
            }
        }));
        balances.add_asset_mappings(&[ImportAssetMappingModel {
            asset_ref: "GBP".to_string(),
            asset_id: Some(GBP),
        }]);
        balances.add_rows(&rows);
        balances
    }

    fn check_with(
        assertions: Vec<ImportAssertionDto>,
        rows: Vec<ImportProposalRowDto>,
        ledger: Vec<LedgerMovementDto>,
    ) -> ImportBalanceCheckDto {
        balances(assertions, rows)
            .check(
                &[],
                &LedgerActivityDto {
                    movements: ledger,
                    sources: Vec::new(),
                },
            )
            .checks
            .remove(0)
    }

    #[test]
    fn a_file_starting_partway_through_a_day_follows_the_rows_the_ledger_has_before_it() {
        let mut fed = movement(3, date!(2026 - 10 - 21), dec!(-20));
        fed.connector_binding_ids = vec![Uuid::from_u128(9)];
        let balances = balances(
            vec![
                opening(date!(2026 - 10 - 21), dec!(950)),
                closing(date!(2026 - 10 - 21), dec!(940)),
                closing(date!(2026 - 10 - 23), dec!(935)),
            ],
            vec![
                row(ImportVerbDto::Skip, date!(2026 - 10 - 21), dec!(-10), &[4]),
                add(date!(2026 - 10 - 23), dec!(-5)),
            ],
        );
        let result = balances.check(
            &[],
            &LedgerActivityDto {
                movements: vec![
                    movement(1, date!(2026 - 01 - 01), dec!(1000)),
                    movement(2, date!(2026 - 10 - 21), dec!(-30)),
                    fed,
                    movement(4, date!(2026 - 10 - 21), dec!(-10)),
                ],
                sources: vec![TransactionSourceDto {
                    kind: SourceKindDto::Statement,
                    stream: "statement:earlier".to_string(),
                    ..balance_source(2, date!(2026 - 10 - 21))
                }],
            },
        );
        let check = &result.checks[0];
        assert_eq!(check.starting_off_by, dec!(0));
        assert_eq!(check.final_off_by, dec!(0));
        assert!(check.changes.is_empty());
    }

    #[test]
    fn a_typed_row_on_a_statements_first_day_that_it_does_not_list_is_named_at_its_opening() {
        let check = check_with(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 10), dec!(990)),
            ],
            vec![add(date!(2026 - 03 - 01), dec!(-10))],
            vec![
                movement(1, date!(2026 - 01 - 20), dec!(1000)),
                movement(2, date!(2026 - 03 - 01), dec!(-10)),
            ],
        );
        assert_eq!(check.starting_off_by, dec!(-10));
        assert!(check.changes.is_empty());
        assert!(check.unlisted[0].explains_difference);
    }

    #[test]
    fn a_bank_confirmed_row_counts_on_its_own_date_and_an_unconfirmed_one_three_days_later() {
        let mut confirmed = movement(2, date!(2026 - 10 - 10), dec!(-5));
        confirmed.connector_binding_ids = vec![Uuid::from_u128(9)];
        let unconfirmed = movement(3, date!(2026 - 10 - 10), dec!(-7));
        let check = check_with(
            vec![
                closing(date!(2026 - 10 - 10), dec!(0)),
                closing(date!(2026 - 10 - 12), dec!(0)),
                in_next_file(closing(date!(2026 - 10 - 14), dec!(0))),
            ],
            Vec::new(),
            vec![confirmed, unconfirmed],
        );
        assert_eq!(check.starting_off_by, dec!(-5));
        assert_eq!(check.changes[0].after, date!(2026 - 10 - 12));
        assert_eq!(check.changes[0].change, dec!(-7));
    }

    #[test]
    fn a_row_the_statement_does_not_list_shows_in_the_period_of_its_own_date() {
        let check = check_with(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                snapshot(date!(2026 - 03 - 03), dec!(980)),
                snapshot(date!(2026 - 03 - 07), dec!(970)),
                snapshot(date!(2026 - 03 - 10), dec!(965)),
            ],
            vec![
                add(date!(2026 - 03 - 03), dec!(-20)),
                add(date!(2026 - 03 - 07), dec!(-10)),
                add(date!(2026 - 03 - 10), dec!(-5)),
            ],
            vec![
                movement(1, date!(2026 - 01 - 20), dec!(1000)),
                movement(2, date!(2026 - 03 - 05), dec!(-40)),
            ],
        );
        assert_eq!(
            check.changes,
            vec![ImportBalanceChangeDto {
                after: date!(2026 - 03 - 03),
                through: date!(2026 - 03 - 07),
                change: dec!(-40),
                off_by: dec!(-40),
                explained: true,
                awaits_answer: false,
            }]
        );
        assert_eq!(check.final_off_by, dec!(-40));
    }

    #[test]
    fn a_row_typed_as_the_statement_ends_is_off_once_the_next_statement_shows_it_never_booked() {
        let march = || {
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 11), dec!(980)),
            ]
        };
        let rows = || vec![add(date!(2026 - 03 - 06), dec!(-20))];
        let ledger = || {
            vec![
                movement(1, date!(2026 - 01 - 20), dec!(1000)),
                movement(2, date!(2026 - 03 - 10), dec!(-40)),
            ]
        };

        let alone = check_with(march(), rows(), ledger());
        assert_eq!(alone.final_off_by, dec!(0));

        let mut both = march();
        both.push(in_next_file(opening(date!(2026 - 03 - 12), dec!(980))));
        both.push(in_next_file(closing(date!(2026 - 03 - 31), dec!(980))));
        let with_the_next = check_with(both, rows(), ledger());
        assert_eq!(
            with_the_next.changes,
            vec![ImportBalanceChangeDto {
                after: date!(2026 - 02 - 28),
                through: date!(2026 - 03 - 11),
                change: dec!(-40),
                off_by: dec!(-40),
                explained: true,
                awaits_answer: false,
            }]
        );
    }

    #[test]
    fn a_booking_date_on_a_source_row_wins_over_the_ledger_date() {
        let balances = balances(vec![closing(date!(2026 - 09 - 30), dec!(0))], Vec::new());
        let result = balances.check(
            &[],
            &LedgerActivityDto {
                movements: vec![movement(2, date!(2026 - 09 - 29), dec!(-5))],
                sources: vec![TransactionSourceDto {
                    transaction_id: Uuid::from_u128(2),
                    account_id: ACCOUNT,
                    asset_id: GBP,
                    date: date!(2026 - 10 - 02),
                    description: None,
                    kind: SourceKindDto::Connector,
                    stream: "connector:x".to_string(),
                    external_ref: None,
                    itemised_fee: None,
                }],
            },
        );
        assert_eq!(result.checks[0].final_off_by, dec!(0));
    }

    #[test]
    fn the_earliest_statement_date_books_a_confirmed_transaction() {
        let check = check_with(
            vec![
                closing(date!(2026 - 10 - 05), dec!(-5)),
                closing(date!(2026 - 10 - 06), dec!(-5)),
            ],
            vec![
                row(ImportVerbDto::Skip, date!(2026 - 10 - 06), dec!(-5), &[7]),
                row(ImportVerbDto::Skip, date!(2026 - 10 - 05), dec!(-5), &[7]),
            ],
            vec![movement(7, date!(2026 - 09 - 30), dec!(-5))],
        );
        assert_eq!(check.starting_off_by, dec!(0));
        assert!(check.changes.is_empty());
    }

    #[test]
    fn a_skipped_row_counts_what_the_ledger_holds_not_the_statement() {
        let check = check_with(
            vec![closing(date!(2026 - 10 - 31), dec!(-5))],
            vec![row(
                ImportVerbDto::Skip,
                date!(2026 - 10 - 05),
                dec!(-5),
                &[7],
            )],
            vec![movement(7, date!(2026 - 10 - 04), dec!(-6))],
        );
        assert_eq!(check.final_off_by, dec!(-1));
    }

    #[test]
    fn a_claim_on_a_bank_fed_group_member_dates_only_that_member() {
        let member = |id: u128, on: Date, quantity: Decimal| {
            let mut member = movement(id, on, quantity);
            member.group_id = Some(Uuid::from_u128(50));
            member.connector_binding_ids = vec![Uuid::from_u128(9)];
            member
        };
        let check = check_with(
            vec![
                closing(date!(2026 - 03 - 05), dec!(-120)),
                closing(date!(2026 - 03 - 08), dec!(-420)),
            ],
            vec![row(
                ImportVerbDto::Skip,
                date!(2026 - 03 - 08),
                dec!(-300),
                &[3],
            )],
            vec![
                member(2, date!(2026 - 03 - 03), dec!(-120)),
                member(3, date!(2026 - 03 - 05), dec!(-300)),
            ],
        );
        assert_eq!(check.starting_off_by, dec!(0));
        assert!(check.changes.is_empty());
    }

    #[test]
    fn an_amended_row_counts_the_statements_amount() {
        let check = check_with(
            vec![closing(date!(2026 - 10 - 31), dec!(-50))],
            vec![row(
                ImportVerbDto::Amend,
                date!(2026 - 10 - 05),
                dec!(-50),
                &[7],
            )],
            vec![movement(7, date!(2026 - 10 - 04), dec!(-60))],
        );
        assert_eq!(check.final_off_by, dec!(0));
    }

    #[test]
    fn balances_that_cannot_be_compared_are_counted_once_per_account() {
        let mut unmapped = closing(date!(2026 - 10 - 30), dec!(1));
        unmapped.account_ref = "Pocket".to_string();
        let mut same_account_next_day = unmapped.clone();
        same_account_next_day.date = date!(2026 - 10 - 31);
        let mut no_asset = closing(date!(2026 - 10 - 31), dec!(1));
        no_asset.asset_ref = None;
        let value = assertion(
            ImportAssertionKindDto::ValueTotal,
            date!(2026 - 10 - 31),
            dec!(1),
        );
        let result = balances(
            vec![unmapped, same_account_next_day, no_asset, value],
            Vec::new(),
        )
        .check(&[], &LedgerActivityDto::default());
        assert_eq!(result.unchecked, 3);
        assert!(result.checks.is_empty());
    }

    fn entered(on: Date, quantity: Decimal) -> ImportEnteredBalanceDto {
        ImportEnteredBalanceDto {
            account_id: ACCOUNT,
            asset_id: GBP,
            date: on,
            quantity,
        }
    }

    fn with_entered(
        rows: Vec<ImportProposalRowDto>,
        assertions: Vec<ImportAssertionDto>,
        balance: ImportEnteredBalanceDto,
    ) -> SessionBalances {
        let mut balances = balances(assertions, rows);
        balances.add_entered_balances(vec![balance]);
        balances
    }

    #[test]
    fn an_entered_balance_is_checked_like_a_printed_closing() {
        let rows = || {
            vec![
                add(date!(2026 - 10 - 05), dec!(10)),
                add(date!(2026 - 10 - 20), dec!(-3)),
            ]
        };
        let right = entered(date!(2026 - 10 - 20), dec!(7));
        let checked =
            with_entered(rows(), Vec::new(), right).check(&[], &LedgerActivityDto::default());
        assert_eq!(checked.unchecked, 0);
        assert_eq!(checked.enterable[0].entered, Some(right));
        let [check] = checked.checks.as_slice() else {
            panic!("expected one check, got {:?}", checked.checks);
        };
        assert_eq!((check.balances_checked, check.final_off_by), (1, dec!(0)));

        let typo = with_entered(rows(), Vec::new(), entered(date!(2026 - 10 - 20), dec!(9)))
            .check(&[], &LedgerActivityDto::default());
        assert_eq!(typo.checks[0].final_off_by, dec!(-2));
    }

    #[test]
    fn an_entered_balance_steps_aside_once_a_file_prints_one() {
        let checked = with_entered(
            vec![add(date!(2026 - 10 - 05), dec!(10))],
            vec![closing(date!(2026 - 10 - 31), dec!(10))],
            entered(date!(2026 - 10 - 05), dec!(99)),
        )
        .check(&[], &LedgerActivityDto::default());

        assert!(checked.enterable.is_empty());
        assert_eq!(
            (
                checked.checks[0].balances_checked,
                checked.checks[0].final_off_by
            ),
            (1, dec!(0))
        );
    }

    #[test]
    fn rows_with_no_stated_balance_are_unchecked_once_per_account_and_asset() {
        let mut dollars = add(date!(2026 - 10 - 06), dec!(-3));
        dollars.movements[0].asset_id = USD;
        let rows = || {
            vec![
                add(date!(2026 - 10 - 05), dec!(-5)),
                add(date!(2026 - 10 - 07), dec!(-2)),
                dollars.clone(),
            ]
        };

        let no_balances = balances(Vec::new(), rows()).check(&[], &LedgerActivityDto::default());
        assert!(no_balances.checks.is_empty());
        assert_eq!(no_balances.unchecked, 2);

        let market_value = assertion(
            ImportAssertionKindDto::ValueTotal,
            date!(2026 - 10 - 31),
            dec!(1),
        );
        let only_a_market_value =
            balances(vec![market_value], rows()).check(&[], &LedgerActivityDto::default());
        assert_eq!(only_a_market_value.unchecked, 2);

        let pounds_stated = balances(vec![closing(date!(2026 - 10 - 31), dec!(-7))], rows())
            .check(&[], &LedgerActivityDto::default());
        assert_eq!(pounds_stated.checks.len(), 1);
        assert_eq!(pounds_stated.unchecked, 1);
    }

    #[test]
    fn an_account_left_out_of_the_import_is_not_reported_as_unchecked() {
        let mut left_out = closing(date!(2026 - 10 - 31), dec!(1));
        left_out.account_ref = "Deposit".to_string();
        let mut balances = balances(vec![left_out], Vec::new());
        balances.add_account_mappings(&[ImportAccountMappingModel {
            file_id: FILE,
            account_ref: "Deposit".to_string(),
            account_id: None,
        }]);
        let result = balances.check(&[], &LedgerActivityDto::default());
        assert_eq!(result.unchecked, 0);
    }

    #[test]
    fn an_asset_left_out_of_the_import_is_not_reported_as_unchecked() {
        let mut left_out = closing(date!(2026 - 10 - 31), dec!(2));
        left_out.asset_ref = Some("SPX500".to_string());
        let mut balances = balances(vec![left_out], Vec::new());
        balances.add_asset_mappings(&[ImportAssetMappingModel {
            asset_ref: "SPX500".to_string(),
            asset_id: None,
        }]);
        let result = balances.check(&[], &LedgerActivityDto::default());
        assert!(result.checks.is_empty());
        assert_eq!(result.unchecked, 0);
    }

    #[test]
    fn a_closing_and_the_next_opening_disagreeing_read_in_that_order() {
        let check = check_with(
            vec![
                opening(date!(2026 - 11 - 01), dec!(5)),
                closing(date!(2026 - 10 - 31), dec!(9)),
            ],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(check.starting_off_by, dec!(-9));
        assert_eq!(check.final_off_by, dec!(-5));
    }

    fn starting(on: Date, quantity: Decimal) -> ImportStartingBalanceDto {
        ImportStartingBalanceDto {
            account_id: ACCOUNT,
            asset_id: GBP,
            file_id: FILE,
            date: on,
            quantity,
        }
    }

    fn starting_balances(
        assertions: Vec<ImportAssertionDto>,
        rows: Vec<ImportProposalRowDto>,
        ledger: Vec<LedgerMovementDto>,
    ) -> Vec<ImportStartingBalanceDto> {
        balances(assertions, rows).starting_balances(
            &[],
            &LedgerActivityDto {
                movements: ledger,
                sources: Vec::new(),
            },
        )
    }

    fn starting_row(verb: ImportVerbDto, on: Date, quantity: Decimal) -> ImportProposalRowDto {
        let mut row = row(verb, on, quantity, &[]);
        row.kind = ImportRowKindDto::StartingBalance(starting(on, quantity));
        row.movements.clear();
        row
    }

    fn balance_source(transaction: u128, on: Date) -> TransactionSourceDto {
        TransactionSourceDto {
            transaction_id: Uuid::from_u128(transaction),
            account_id: ACCOUNT,
            asset_id: GBP,
            date: on,
            description: None,
            kind: SourceKindDto::Balance,
            stream: "statement:earlier".to_string(),
            external_ref: Some("starting-balance".to_string()),
            itemised_fee: None,
        }
    }

    fn in_dollars(mut assertion: ImportAssertionDto) -> ImportAssertionDto {
        assertion.asset_ref = Some("USD".to_string());
        assertion
    }

    fn dollars(mut row: ImportProposalRowDto) -> ImportProposalRowDto {
        row.movements[0].asset_id = USD;
        row
    }

    fn starting_balances_in_two_currencies(
        assertions: Vec<ImportAssertionDto>,
        rows: Vec<ImportProposalRowDto>,
    ) -> Vec<ImportStartingBalanceDto> {
        let mut balances = balances(assertions, rows);
        balances.add_asset_mappings(&[ImportAssetMappingModel {
            asset_ref: "USD".to_string(),
            asset_id: Some(USD),
        }]);
        balances.starting_balances(&[], &LedgerActivityDto::default())
    }

    #[test]
    fn a_position_that_first_moves_later_in_its_file_starts_before_the_files_first_row() {
        let found = starting_balances_in_two_currencies(
            vec![
                in_dollars(opening(date!(2026 - 09 - 04), dec!(63.20))),
                in_dollars(closing(date!(2026 - 10 - 22), dec!(73.20))),
            ],
            vec![
                add(date!(2026 - 09 - 01), dec!(-8.20)),
                dollars(add(date!(2026 - 09 - 04), dec!(10))),
            ],
        );
        let mut held = starting(date!(2026 - 08 - 31), dec!(63.2));
        held.asset_id = USD;
        assert_eq!(found, vec![held]);
    }

    #[test]
    fn another_files_rows_do_not_date_the_starting_balance_of_a_position_they_do_not_move() {
        let mut earlier_file = add(date!(2026 - 08 - 10), dec!(-8.20));
        earlier_file.movements[0].file_id = NEXT_FILE;
        let found = starting_balances_in_two_currencies(
            vec![in_dollars(opening(date!(2026 - 09 - 04), dec!(63.20)))],
            vec![earlier_file, dollars(add(date!(2026 - 09 - 04), dec!(10)))],
        );
        let mut held = starting(date!(2026 - 09 - 03), dec!(63.2));
        held.asset_id = USD;
        assert_eq!(found, vec![held]);
    }

    #[test]
    fn statements_that_follow_each_other_start_once_at_the_earliest() {
        let found = starting_balances(
            vec![
                in_next_file(opening(date!(2026 - 10 - 01), dec!(80))),
                in_next_file(closing(date!(2026 - 10 - 31), dec!(50))),
                opening(date!(2026 - 09 - 01), dec!(100)),
                closing(date!(2026 - 09 - 30), dec!(80)),
            ],
            vec![
                add(date!(2026 - 09 - 10), dec!(-20)),
                add(date!(2026 - 10 - 10), dec!(-30)),
            ],
            Vec::new(),
        );
        assert_eq!(found, vec![starting(date!(2026 - 08 - 31), dec!(100))]);
    }

    #[test]
    fn a_typed_entry_the_statement_books_inside_its_period_is_not_history_before_it() {
        let found = starting_balances(
            vec![opening(date!(2026 - 10 - 01), dec!(100))],
            vec![row(
                ImportVerbDto::Skip,
                date!(2026 - 10 - 02),
                dec!(-30),
                &[7],
            )],
            vec![movement(7, date!(2026 - 09 - 29), dec!(-30))],
        );
        assert_eq!(found, vec![starting(date!(2026 - 09 - 30), dec!(100))]);
    }

    #[test]
    fn a_ledger_row_the_statements_own_row_claims_is_not_history_before_a_closing_balance() {
        let mut synced = movement(7, date!(2026 - 10 - 20), dec!(-100));
        synced.connector_binding_ids = vec![Uuid::from_u128(70)];
        let found = starting_balances(
            vec![closing(date!(2026 - 10 - 31), dec!(500))],
            vec![
                add(date!(2026 - 10 - 05), dec!(200)),
                row(ImportVerbDto::Skip, date!(2026 - 10 - 20), dec!(-100), &[7]),
            ],
            vec![synced],
        );
        assert_eq!(found, vec![starting(date!(2026 - 10 - 04), dec!(400))]);
    }

    #[test]
    fn a_holding_typed_on_or_shortly_before_a_lone_balance_is_history() {
        let stated = || vec![snapshot(date!(2026 - 10 - 31), dec!(10))];
        for typed_on in [
            date!(2026 - 10 - 31),
            date!(2026 - 10 - 29),
            date!(2026 - 10 - 28),
        ] {
            let found =
                starting_balances(stated(), Vec::new(), vec![movement(1, typed_on, dec!(10))]);
            assert!(found.is_empty(), "typed on {typed_on}");
        }

        let typed_the_day_after = starting_balances(
            stated(),
            Vec::new(),
            vec![movement(1, date!(2026 - 11 - 01), dec!(10))],
        );
        assert_eq!(
            typed_the_day_after,
            vec![starting(date!(2026 - 10 - 31), dec!(10))]
        );
    }

    #[test]
    fn an_opening_balance_typed_on_the_statements_first_day_is_history() {
        let found = starting_balances(
            vec![
                opening(date!(2026 - 10 - 01), dec!(500)),
                closing(date!(2026 - 10 - 31), dec!(600)),
            ],
            vec![add(date!(2026 - 10 - 05), dec!(100))],
            vec![movement(1, date!(2026 - 10 - 01), dec!(500))],
        );
        assert!(found.is_empty());
    }

    #[test]
    fn a_starting_balance_in_the_ledger_inside_the_statements_period_is_not_history() {
        let stated = || {
            balances(
                vec![closing(date!(2026 - 01 - 31), dec!(580))],
                vec![
                    add(date!(2026 - 01 - 10), dec!(100)),
                    add(date!(2026 - 01 - 31), dec!(-20)),
                ],
            )
        };
        let started_on = |on: Date, quantity: Decimal| LedgerActivityDto {
            movements: vec![movement(9, on, quantity)],
            sources: vec![balance_source(9, on)],
        };

        let from_the_next_statement =
            stated().starting_balances(&[], &started_on(date!(2026 - 01 - 31), dec!(580)));
        let from_this_statement =
            stated().starting_balances(&[], &started_on(date!(2026 - 01 - 09), dec!(500)));

        assert_eq!(
            from_the_next_statement,
            vec![starting(date!(2026 - 01 - 09), dec!(500))]
        );
        assert!(from_this_statement.is_empty());
    }

    fn in_the_pot(mut assertion: ImportAssertionDto) -> ImportAssertionDto {
        assertion.account_ref = "Savings pot".to_string();
        assertion
    }

    fn with_the_pot(
        assertions: Vec<ImportAssertionDto>,
        rows: Vec<ImportProposalRowDto>,
    ) -> SessionBalances {
        let mut balances = balances(assertions, rows);
        balances.add_account_mappings(&[ImportAccountMappingModel {
            file_id: FILE,
            account_ref: "Savings pot".to_string(),
            account_id: Some(ACCOUNT),
        }]);
        balances
    }

    #[test]
    fn printed_accounts_of_one_file_on_one_account_are_compared_as_their_sum() {
        let stated = || {
            vec![
                opening(date!(2026 - 10 - 01), dec!(100)),
                in_the_pot(opening(date!(2026 - 10 - 01), dec!(300))),
                in_the_pot(snapshot(date!(2026 - 10 - 15), dec!(310))),
                closing(date!(2026 - 10 - 31), dec!(150)),
                in_the_pot(closing(date!(2026 - 10 - 31), dec!(320))),
            ]
        };
        let rows = || {
            vec![
                add(date!(2026 - 10 - 05), dec!(50)),
                add(date!(2026 - 10 - 06), dec!(20)),
            ]
        };

        let found =
            with_the_pot(stated(), rows()).starting_balances(&[], &LedgerActivityDto::default());
        assert_eq!(found, vec![starting(date!(2026 - 09 - 30), dec!(400))]);

        let mut started = rows();
        started.push(starting_row(
            ImportVerbDto::Add,
            date!(2026 - 09 - 30),
            dec!(400),
        ));
        let check = with_the_pot(stated(), started)
            .check(&[], &LedgerActivityDto::default())
            .checks
            .remove(0);
        assert_eq!(check.balances_checked, 2);
        assert_eq!(check.starting_off_by, dec!(0));
        assert_eq!(check.final_off_by, dec!(0));
    }

    #[test]
    fn printed_accounts_that_never_state_the_same_moment_are_not_compared() {
        let stated = || {
            with_the_pot(
                vec![
                    opening(date!(2026 - 10 - 01), dec!(100)),
                    in_the_pot(closing(date!(2026 - 10 - 31), dec!(300))),
                ],
                Vec::new(),
            )
        };
        assert!(stated()
            .starting_balances(&[], &LedgerActivityDto::default())
            .is_empty());
        let result = stated().check(&[], &LedgerActivityDto::default());
        assert!(result.checks.is_empty());
        assert_eq!(result.unchecked, 1);
    }

    #[test]
    fn two_files_stating_different_balances_for_one_moment_start_nothing() {
        let found = starting_balances(
            vec![
                opening(date!(2026 - 10 - 01), dec!(100)),
                in_next_file(opening(date!(2026 - 10 - 01), dec!(300))),
            ],
            Vec::new(),
            Vec::new(),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn ledger_history_short_of_the_first_balance_is_offered_the_difference() {
        let stated = || balances(vec![opening(date!(2026 - 10 - 01), dec!(100))], Vec::new());
        let in_the_window = LedgerActivityDto {
            movements: vec![movement(1, date!(2026 - 09 - 10), dec!(5))],
            sources: Vec::new(),
        };
        let further_back = |quantity: Decimal| {
            vec![LedgerTotalDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                quantity,
            }]
        };

        assert_eq!(
            stated().starting_balances(&[], &in_the_window),
            vec![starting(date!(2026 - 09 - 30), dec!(95))]
        );
        assert_eq!(
            stated().check(&[], &in_the_window).checks[0].starting_off_by,
            dec!(-95)
        );
        assert_eq!(
            stated().starting_balances(&further_back(dec!(0)), &LedgerActivityDto::default()),
            vec![starting(date!(2026 - 09 - 30), dec!(100))]
        );
        assert!(stated()
            .starting_balances(&further_back(dec!(100)), &LedgerActivityDto::default())
            .is_empty());
        let further_back = further_back(dec!(0));
        assert_eq!(
            stated()
                .check(&further_back, &LedgerActivityDto::default())
                .checks[0]
                .starting_off_by,
            dec!(-100)
        );
    }

    #[test]
    fn the_same_first_balance_in_two_files_is_taken_from_the_same_file_on_every_read() {
        let stated = || {
            [
                closing(date!(2026 - 09 - 30), dec!(100)),
                in_next_file(closing(date!(2026 - 09 - 30), dec!(100))),
            ]
        };
        let [first, second] = stated();
        let one_order = starting_balances(vec![first, second], Vec::new(), Vec::new());
        let [first, second] = stated();
        let other_order = starting_balances(vec![second, first], Vec::new(), Vec::new());

        assert_eq!(one_order, vec![starting(date!(2026 - 09 - 30), dec!(100))]);
        assert_eq!(one_order, other_order);
    }

    #[test]
    fn a_starting_balance_row_makes_the_first_balance_true_unless_it_is_skipped() {
        let check = |verb: ImportVerbDto| {
            check_with(
                vec![
                    opening(date!(2026 - 10 - 01), dec!(100)),
                    closing(date!(2026 - 10 - 31), dec!(60)),
                ],
                vec![
                    starting_row(verb, date!(2026 - 09 - 30), dec!(100)),
                    add(date!(2026 - 10 - 05), dec!(-40)),
                ],
                Vec::new(),
            )
        };

        let added = check(ImportVerbDto::Add);
        let skipped = check(ImportVerbDto::Skip);

        assert_eq!(added.starting_off_by, dec!(0));
        assert_eq!(added.final_off_by, dec!(0));
        assert_eq!(skipped.starting_off_by, dec!(-100));
        assert_eq!(skipped.final_off_by, dec!(-100));
    }

    #[test]
    fn a_starting_balance_in_the_ledger_inside_the_statements_period_is_named_with_the_difference()
    {
        let check = balances(
            vec![
                opening(date!(2026 - 09 - 01), dec!(70)),
                closing(date!(2026 - 09 - 30), dec!(100)),
            ],
            vec![
                starting_row(ImportVerbDto::Add, date!(2026 - 08 - 31), dec!(70)),
                add(date!(2026 - 09 - 10), dec!(30)),
            ],
        )
        .check(
            &[],
            &LedgerActivityDto {
                movements: vec![movement(9, date!(2026 - 09 - 30), dec!(100))],
                sources: vec![balance_source(9, date!(2026 - 09 - 30))],
            },
        )
        .checks
        .remove(0);

        assert_eq!(check.starting_off_by, dec!(0));
        assert_eq!(check.final_off_by, dec!(100));
        assert_eq!(
            check.unlisted,
            vec![ImportUnlistedMovementDto {
                transaction_id: Uuid::from_u128(9),
                date: date!(2026 - 09 - 30),
                quantity: dec!(100),
                description: None,
                may_book_later: false,
                explains_difference: true,
                ask_key: Some(not_on_statement_ask_key(Uuid::from_u128(9))),
                deleting: false,
                other_account_ids: Vec::new(),
            }]
        );
    }

    fn typed(id: u128, on: Date, quantity: Decimal, description: &str) -> LedgerMovementDto {
        LedgerMovementDto {
            description: Some(description.to_string()),
            ..movement(id, on, quantity)
        }
    }

    fn unlisted(
        id: u128,
        on: Date,
        quantity: Decimal,
        description: &str,
    ) -> ImportUnlistedMovementDto {
        ImportUnlistedMovementDto {
            transaction_id: Uuid::from_u128(id),
            date: on,
            quantity,
            description: Some(description.to_string()),
            may_book_later: false,
            explains_difference: true,
            ask_key: Some(not_on_statement_ask_key(Uuid::from_u128(id))),
            deleting: false,
            other_account_ids: Vec::new(),
        }
    }

    #[test]
    fn a_typed_row_the_statement_does_not_list_is_named_with_the_difference_it_makes() {
        let check = check_with(
            vec![
                opening(date!(2026 - 06 - 01), dec!(1000)),
                snapshot(date!(2026 - 06 - 05), dec!(980)),
                closing(date!(2026 - 06 - 30), dec!(970)),
            ],
            vec![
                add(date!(2026 - 06 - 03), dec!(-20)),
                add(date!(2026 - 06 - 12), dec!(-10)),
            ],
            vec![
                movement(1, date!(2026 - 05 - 01), dec!(1000)),
                typed(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)"),
                typed(3, date!(2026 - 06 - 18), dec!(-50), "lent Tom 50"),
            ],
        );
        assert_eq!(check.final_off_by, dec!(-85));
        assert_eq!(
            check.unlisted,
            vec![
                unlisted(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)"),
                unlisted(3, date!(2026 - 06 - 18), dec!(-50), "lent Tom 50"),
            ]
        );
    }

    #[test]
    fn typed_rows_that_only_together_make_the_difference_are_all_marked() {
        let cinema = |id: u128, quantity: Decimal| {
            let mut member = typed(id, date!(2026 - 10 - 11), quantity, "cinema");
            member.group_id = Some(Uuid::from_u128(50));
            member
        };
        let check = check_with(
            vec![
                opening(date!(2026 - 10 - 01), dec!(1000)),
                closing(date!(2026 - 10 - 31), dec!(969.50)),
            ],
            vec![add(date!(2026 - 10 - 11), dec!(-30.50))],
            vec![
                movement(1, date!(2026 - 09 - 01), dec!(1000)),
                cinema(2, dec!(-18)),
                cinema(3, dec!(-12.50)),
            ],
        );
        assert_eq!(
            check.unlisted,
            [
                unlisted(2, date!(2026 - 10 - 11), dec!(-18), "cinema"),
                unlisted(3, date!(2026 - 10 - 11), dec!(-12.5), "cinema"),
            ]
            .map(|part| ImportUnlistedMovementDto {
                ask_key: Some(not_on_statement_ask_key(Uuid::from_u128(50))),
                ..part
            })
        );
    }

    #[test]
    fn typed_rows_that_do_not_add_up_to_the_difference_are_named_without_the_mark() {
        let check = check_with(
            vec![
                opening(date!(2026 - 06 - 01), dec!(1000)),
                closing(date!(2026 - 06 - 30), dec!(900)),
            ],
            Vec::new(),
            vec![
                movement(1, date!(2026 - 05 - 01), dec!(1000)),
                typed(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)"),
                typed(3, date!(2026 - 06 - 18), dec!(-50), "lent Tom 50"),
            ],
        );
        assert_eq!(check.final_off_by, dec!(15));
        assert_eq!(
            check.unlisted,
            vec![
                ImportUnlistedMovementDto {
                    explains_difference: false,
                    ..unlisted(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)")
                },
                ImportUnlistedMovementDto {
                    explains_difference: false,
                    ..unlisted(3, date!(2026 - 06 - 18), dec!(-50), "lent Tom 50")
                },
            ]
        );
    }

    #[test]
    fn a_typed_row_in_the_statements_last_days_is_named_as_possibly_booked_later() {
        let check = check_with(
            vec![
                opening(date!(2026 - 06 - 01), dec!(1000)),
                closing(date!(2026 - 06 - 30), dec!(1000)),
            ],
            Vec::new(),
            vec![
                movement(1, date!(2026 - 05 - 01), dec!(1000)),
                typed(2, date!(2026 - 06 - 28), dec!(-6), "raffle tickets"),
            ],
        );
        assert_eq!(check.final_off_by, dec!(0));
        assert_eq!(
            check.unlisted,
            vec![ImportUnlistedMovementDto {
                may_book_later: true,
                explains_difference: false,
                ask_key: None,
                ..unlisted(2, date!(2026 - 06 - 28), dec!(-6), "raffle tickets")
            }]
        );
    }

    #[test]
    fn rows_the_bank_confirms_a_row_claims_or_dated_outside_the_statement_are_not_named() {
        let mut synced = movement(2, date!(2026 - 06 - 04), dec!(-5));
        synced.connector_binding_ids = vec![Uuid::from_u128(9)];
        let result = balances(
            vec![
                opening(date!(2026 - 06 - 01), dec!(1000)),
                closing(date!(2026 - 06 - 30), dec!(968)),
            ],
            vec![
                row(ImportVerbDto::Skip, date!(2026 - 06 - 10), dec!(-7), &[4]),
                row(ImportVerbDto::Amend, date!(2026 - 06 - 11), dec!(-9), &[5]),
            ],
        )
        .check(
            &[],
            &LedgerActivityDto {
                movements: vec![
                    movement(1, date!(2026 - 05 - 20), dec!(1000)),
                    synced,
                    movement(3, date!(2026 - 06 - 05), dec!(-11)),
                    movement(4, date!(2026 - 06 - 09), dec!(-7)),
                    movement(5, date!(2026 - 06 - 11), dec!(-8)),
                ],
                sources: vec![TransactionSourceDto {
                    kind: SourceKindDto::Statement,
                    ..balance_source(3, date!(2026 - 06 - 05))
                }],
            },
        );
        assert_eq!(result.checks[0].final_off_by, dec!(0));
        assert!(result.checks[0].unlisted.is_empty());
    }

    fn candidate(id: u128, on: Date, quantity: Decimal) -> MatchCandidateDto {
        MatchCandidateDto {
            id: Uuid::from_u128(id),
            in_ledger: true,
            confirmed: false,
            several_transactions: false,
            leaves_movement_unrecorded: false,
            account_id: Some(ACCOUNT),
            asset_id: GBP,
            quantity,
            date: on,
            description: None,
        }
    }

    #[test]
    fn a_difference_an_open_question_may_settle_offers_no_adjustment_and_no_delete() {
        let day = date!(2026 - 06 - 18);
        let mut slip = add(day, dec!(-2));
        slip.near_miss = vec![ImportNearMissCandidateDto {
            candidate: candidate(2, day, dec!(-20)),
            amend: true,
            parts: Vec::new(),
        }];
        let parts = vec![
            candidate(3, day, dec!(-30)),
            candidate(4, day, dec!(-20.40)),
            candidate(5, day, dec!(-12)),
        ];
        let mut purchase = add(day, dec!(-62.40));
        purchase.near_miss = vec![ImportNearMissCandidateDto {
            candidate: MatchCandidateDto {
                several_transactions: true,
                quantity: dec!(-62.40),
                ..candidate(3, day, dec!(-62.40))
            },
            amend: false,
            parts,
        }];
        let result = balances(
            vec![
                snapshot(date!(2026 - 06 - 17), dec!(1000)),
                snapshot(day, dec!(935.60)),
                closing(date!(2026 - 06 - 30), dec!(935.60)),
            ],
            vec![slip, purchase],
        )
        .check(
            &[],
            &LedgerActivityDto {
                movements: vec![
                    movement(1, date!(2026 - 05 - 01), dec!(1000)),
                    typed(2, day, dec!(-20), "typed"),
                    typed(3, day, dec!(-30), "milk"),
                    typed(4, day, dec!(-20.40), "meat"),
                    typed(5, day, dec!(-12), "fruit"),
                ],
                sources: Vec::new(),
            },
        );

        let check = &result.checks[0];
        assert!(check.unlisted.is_empty());
        assert!(matches!(
            check.changes[..],
            [ImportBalanceChangeDto {
                awaits_answer: true,
                explained: false,
                ..
            }]
        ));
        assert!(adjustments(&result).is_empty());
    }

    #[test]
    fn history_before_the_range_comes_from_the_totals() {
        let result = balances(vec![closing(date!(2026 - 10 - 31), dec!(40))], Vec::new()).check(
            &vec![LedgerTotalDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                quantity: dec!(40),
            }],
            &LedgerActivityDto::default(),
        );
        assert_eq!(result.checks[0].final_off_by, dec!(0));
    }

    fn deleting(id: u128) -> ImportProposalRowDto {
        let mut delete = row(ImportVerbDto::Delete, date!(2026 - 06 - 04), dec!(0), &[]);
        delete.transaction.transaction_id = Some(Uuid::from_u128(id));
        delete.movements.clear();
        delete
    }

    #[test]
    fn a_split_the_matcher_took_as_one_payment_is_not_listed_part_by_part() {
        let member = |movement: LedgerMovementDto| LedgerMovementDto {
            group_id: Some(Uuid::from_u128(50)),
            ..movement
        };
        let synced = LedgerMovementDto {
            connector_binding_ids: vec![Uuid::from_u128(9)],
            ..member(movement(2, date!(2026 - 03 - 03), dec!(-30)))
        };
        let check = check_with(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 07), dec!(950)),
            ],
            vec![row(
                ImportVerbDto::Skip,
                date!(2026 - 03 - 04),
                dec!(-50),
                &[2],
            )],
            vec![
                movement(1, date!(2026 - 02 - 01), dec!(1000)),
                synced,
                member(movement(3, date!(2026 - 03 - 03), dec!(-20))),
            ],
        );
        assert!(check.unlisted.is_empty());
        assert_eq!(check.final_off_by, dec!(0));
    }

    #[test]
    fn a_row_the_user_deletes_stops_counting_and_stays_listed_so_it_can_be_kept() {
        let assertions = || {
            vec![
                opening(date!(2026 - 06 - 01), dec!(1000)),
                closing(date!(2026 - 06 - 30), dec!(1000)),
            ]
        };
        let ledger = || {
            vec![
                movement(1, date!(2026 - 05 - 01), dec!(1000)),
                typed(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)"),
            ]
        };
        let kept = check_with(assertions(), Vec::new(), ledger());
        assert_eq!(kept.final_off_by, dec!(-35));
        assert_eq!(
            kept.unlisted,
            vec![unlisted(
                2,
                date!(2026 - 06 - 04),
                dec!(-35),
                "haircut (cash)"
            )]
        );

        let deleted = check_with(assertions(), vec![deleting(2)], ledger());
        assert_eq!(deleted.final_off_by, dec!(0));
        assert_eq!(
            deleted.unlisted,
            vec![ImportUnlistedMovementDto {
                deleting: true,
                explains_difference: false,
                ..unlisted(2, date!(2026 - 06 - 04), dec!(-35), "haircut (cash)")
            }]
        );
    }

    #[test]
    fn a_transfer_whose_other_side_a_statement_confirms_is_not_offered_for_deletion() {
        const SAVINGS: Uuid = Uuid::from_u128(60);
        let ledger = || {
            vec![
                movement(1, date!(2026 - 06 - 01), dec!(1000)),
                movement(4, date!(2026 - 06 - 10), dec!(-50)),
                LedgerMovementDto {
                    account_id: SAVINGS,
                    ..movement(4, date!(2026 - 06 - 10), dec!(50))
                },
            ]
        };
        let assertions = || {
            vec![
                opening(date!(2026 - 06 - 02), dec!(1000)),
                closing(date!(2026 - 06 - 30), dec!(1000)),
            ]
        };
        let unconfirmed = check_with(assertions(), Vec::new(), ledger());
        assert_eq!(
            unconfirmed.unlisted[0].ask_key,
            Some(not_on_statement_ask_key(Uuid::from_u128(4)))
        );
        assert_eq!(unconfirmed.unlisted[0].other_account_ids, vec![SAVINGS]);

        let mut arrived = row(ImportVerbDto::Skip, date!(2026 - 06 - 10), dec!(50), &[]);
        arrived.claims = vec![ImportClaimDto {
            transaction_id: Uuid::from_u128(4),
            account_id: SAVINGS,
            asset_id: GBP,
            booked_on: date!(2026 - 06 - 10),
        }];
        arrived.movements[0].account_id = SAVINGS;
        let confirmed = check_with(assertions(), vec![arrived], ledger());
        assert_eq!(confirmed.unlisted[0].ask_key, None);
    }

    #[test]
    fn only_a_difference_no_listed_row_explains_is_offered_as_an_adjustment() {
        let checks = |assertions: Vec<ImportAssertionDto>, ledger: Vec<LedgerMovementDto>| {
            balances(assertions, Vec::new()).check(
                &[],
                &LedgerActivityDto {
                    movements: ledger,
                    sources: Vec::new(),
                },
            )
        };
        let opening_only = || vec![movement(1, date!(2026 - 02 - 01), dec!(1000))];

        let missing_rows = checks(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 31), dec!(900)),
            ],
            opening_only(),
        );
        let offered = adjustments(&missing_rows);
        assert_eq!(
            offered,
            vec![ImportAdjustmentDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                date: date!(2026 - 03 - 31),
                quantity: dec!(-100),
            }]
        );

        let mut adjusted = balances(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 31), dec!(900)),
            ],
            Vec::new(),
        );
        adjusted.add_rows(&[ImportProposalRowDto {
            kind: ImportRowKindDto::Adjustment(offered[0]),
            movements: Vec::new(),
            ..add(date!(2026 - 03 - 31), dec!(0))
        }]);
        let adjusted = adjusted.check(
            &[],
            &LedgerActivityDto {
                movements: opening_only(),
                sources: Vec::new(),
            },
        );
        assert_eq!(adjusted.checks[0].final_off_by, dec!(0));

        let explained = checks(
            vec![
                opening(date!(2026 - 03 - 01), dec!(1000)),
                closing(date!(2026 - 03 - 31), dec!(1000)),
            ],
            vec![
                movement(1, date!(2026 - 02 - 01), dec!(1000)),
                typed(2, date!(2026 - 03 - 10), dec!(-35), "haircut (cash)"),
            ],
        );
        assert!(adjustments(&explained).is_empty());

        let disagreeing = checks(
            vec![
                closing(date!(2026 - 03 - 31), dec!(1000)),
                in_next_file(opening(date!(2026 - 04 - 01), dec!(1050))),
            ],
            vec![movement(1, date!(2026 - 02 - 01), dec!(1000))],
        );
        assert_eq!(disagreeing.checks[0].changes.len(), 1);
        assert!(adjustments(&disagreeing).is_empty());
    }
}
