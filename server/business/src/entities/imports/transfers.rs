use std::collections::{BTreeMap, HashMap, HashSet};

use rust_decimal::Decimal;
use time::{Date, OffsetDateTime, Time, UtcOffset};
use uuid::Uuid;

use super::staging::SessionStage;
use crate::dtos::entry_dto::EntryDto;
use crate::dtos::imports::import_staging_dto::{
    ImportRowMatchDto, ImportStagedDecisionDto, StagedTransactionDto,
};
use crate::dtos::transaction_dto::{
    CashBalanceTransferMetadataDto, TransactionDto, TransactionTypeDto,
};

struct Side<'a> {
    row: usize,
    entry: &'a EntryDto,
    day: Date,
    at: Option<OffsetDateTime>,
    called_transfer: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Evidence {
    SameDay,
    SameTime,
}

pub fn join_decided(stage: &mut SessionStage, decisions: &[ImportStagedDecisionDto]) {
    let decided = decided(decisions);
    let staged = &stage.staged;
    let mut joined: HashSet<usize> = HashSet::new();
    let mut pairs = Vec::new();
    for group in sides(staged, |row| !decided.contains(&row.id)) {
        for out in group
            .iter()
            .filter(|side| side.entry.quantity < Decimal::ZERO)
        {
            for incoming in &group {
                if evidence(out, incoming).is_some()
                    && decided.contains(&joined_id(staged[out.row].id, staged[incoming.row].id))
                    && !joined.contains(&out.row)
                    && !joined.contains(&incoming.row)
                {
                    joined.extend([out.row, incoming.row]);
                    pairs.push((out.row, incoming.row));
                }
            }
        }
    }
    join(stage, pairs);
}

pub fn join_new(
    stage: &mut SessionStage,
    matches: &HashMap<Uuid, ImportRowMatchDto>,
    decisions: &[ImportStagedDecisionDto],
) {
    let decided = decided(decisions);
    let new = ImportRowMatchDto::default();
    let mut pairs = Vec::new();
    for group in sides(&stage.staged, |row| {
        !decided.contains(&row.id) && matches.get(&row.id) == Some(&new)
    }) {
        let mut joined: HashSet<usize> = HashSet::new();
        for least in [Evidence::SameTime, Evidence::SameDay] {
            let open: Vec<&Side> = group
                .iter()
                .filter(|side| !joined.contains(&side.row))
                .collect();
            let partners = |side: &Side| -> Vec<usize> {
                open.iter()
                    .filter(|other| evidence(side, other).is_some_and(|found| found >= least))
                    .map(|other| other.row)
                    .collect()
            };
            for &out in open
                .iter()
                .filter(|side| side.entry.quantity < Decimal::ZERO)
            {
                let [partner] = partners(out)[..] else {
                    continue;
                };
                let incoming = open
                    .iter()
                    .copied()
                    .find(|side| side.row == partner)
                    .expect("a partner is an open side");
                if partners(incoming) == [out.row] {
                    joined.extend([out.row, partner]);
                    pairs.push((out.row, partner));
                }
            }
        }
    }
    join(stage, pairs);
}

fn decided(decisions: &[ImportStagedDecisionDto]) -> HashSet<Uuid> {
    decisions
        .iter()
        .filter(|decision| decision.ask_key.is_none())
        .map(|decision| decision.id)
        .collect()
}

fn sides(
    staged: &[StagedTransactionDto],
    joinable: impl Fn(&StagedTransactionDto) -> bool,
) -> Vec<Vec<Side<'_>>> {
    let mut groups: BTreeMap<(Date, i32, Decimal), Vec<Side>> = BTreeMap::new();
    for (row, staged) in staged.iter().enumerate() {
        let Some((entry, called_transfer)) = cash_entry(&staged.transaction.transaction_type)
        else {
            continue;
        };
        let [movement] = staged.movements.as_slice() else {
            continue;
        };
        if !joinable(staged)
            || !staged.transaction.fee_entries.is_empty()
            || movement.itemised_fee.is_some()
            || entry.quantity.is_zero()
        {
            continue;
        }
        let at = staged.transaction.date;
        groups
            .entry((movement.date, entry.asset_id, entry.quantity.abs()))
            .or_default()
            .push(Side {
                row,
                entry,
                day: movement.date,
                at: (at.to_offset(UtcOffset::UTC).time() != Time::MIDNIGHT).then_some(at),
                called_transfer,
            });
    }
    groups.into_values().collect()
}

fn cash_entry(transaction_type: &TransactionTypeDto) -> Option<(&EntryDto, bool)> {
    match transaction_type {
        TransactionTypeDto::Regular(metadata) => Some((&metadata.entry, false)),
        TransactionTypeDto::CashTransferIn(metadata) => Some((&metadata.entry, true)),
        TransactionTypeDto::CashTransferOut(metadata) => Some((&metadata.entry, true)),
        _ => None,
    }
}

fn evidence(a: &Side, b: &Side) -> Option<Evidence> {
    if a.entry.account_id == b.entry.account_id
        || a.entry.asset_id != b.entry.asset_id
        || a.entry.quantity != -b.entry.quantity
        || a.day != b.day
    {
        return None;
    }
    match (a.at, b.at) {
        (Some(a_at), Some(b_at)) if a_at == b_at => Some(Evidence::SameTime),
        _ if a.called_transfer && b.called_transfer => Some(Evidence::SameDay),
        _ => None,
    }
}

fn joined_id(out: Uuid, incoming: Uuid) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        &[out.as_bytes().as_slice(), incoming.as_bytes()].concat(),
    )
}

fn join(stage: &mut SessionStage, pairs: Vec<(usize, usize)>) {
    if pairs.is_empty() {
        return;
    }
    let mut rows: Vec<Option<StagedTransactionDto>> = std::mem::take(&mut stage.staged)
        .into_iter()
        .map(Some)
        .collect();
    for (out, incoming) in pairs {
        let (Some(out_row), Some(in_row)) = (rows[out].take(), rows[incoming].take()) else {
            unreachable!("each row joins one move");
        };
        let entry = |row: &StagedTransactionDto| {
            cash_entry(&row.transaction.transaction_type)
                .expect("a side is a one-entry cash row")
                .0
                .clone()
        };
        let transaction_type =
            TransactionTypeDto::CashBalanceTransfer(CashBalanceTransferMetadataDto {
                outgoing_change: entry(&out_row),
                incoming_change: entry(&in_row),
            });
        let mut file_ids = out_row.file_ids;
        file_ids.extend(in_row.file_ids);
        file_ids.sort();
        file_ids.dedup();
        let mut movements = out_row.movements;
        movements.extend(in_row.movements);
        let id = joined_id(out_row.id, in_row.id);
        stage.joined.insert(id, [out_row.id, in_row.id]);
        rows[out.min(incoming)] = Some(StagedTransactionDto {
            id,
            file_ids,
            transaction: TransactionDto {
                transaction_type,
                ..out_row.transaction
            },
            movements,
        });
    }
    stage.staged = rows.into_iter().flatten().collect();
}

#[cfg(test)]
mod tests {
    use time::macros::{date, datetime};

    use super::*;
    use crate::dtos::fee_entry_dto::FeeEntryDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::imports::import_staging_dto::{
        ImportNearMissCandidateDto, ImportVerbDto, StagedMovementDto,
    };
    use crate::dtos::ledger_matching_dto::MatchCandidateDto;
    use crate::dtos::transaction_dto::{
        CashTransferInMetadataDto, CashTransferOutMetadataDto, RegularTransactionMetadataDto,
        TransactionVisibilityDto,
    };

    const CURRENT: Uuid = Uuid::from_u128(1);
    const JOINT: Uuid = Uuid::from_u128(2);
    const SAVINGS: Uuid = Uuid::from_u128(3);
    const EUR: i32 = 45;
    const GBP: i32 = 46;

    fn file(account_id: Uuid) -> Uuid {
        Uuid::from_u128(account_id.as_u128() + 100)
    }

    fn row(
        id: u128,
        account_id: Uuid,
        at: OffsetDateTime,
        quantity: &str,
        called_transfer: bool,
    ) -> StagedTransactionDto {
        let quantity: Decimal = quantity.parse().unwrap();
        let entry = EntryDto::new(EUR, account_id, quantity);
        StagedTransactionDto {
            id: Uuid::from_u128(id),
            file_ids: vec![file(account_id)],
            transaction: TransactionDto {
                transaction_id: None,
                date: at,
                visibility: TransactionVisibilityDto::Default,
                fee_entries: Vec::new(),
                transaction_type: match (called_transfer, quantity > Decimal::ZERO) {
                    (true, true) => {
                        TransactionTypeDto::CashTransferIn(CashTransferInMetadataDto { entry })
                    }
                    (true, false) => {
                        TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto { entry })
                    }
                    (false, _) => TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                        description: Some("Transfer".to_string()),
                        entry,
                        category_id: 1,
                    }),
                },
            },
            movements: vec![StagedMovementDto {
                account_id,
                asset_id: EUR,
                quantity,
                file_id: file(account_id),
                date: at.date(),
                description: Some("Transfer".to_string()),
                itemised_fee: None,
            }],
        }
    }

    fn all_new(rows: &[StagedTransactionDto]) -> HashMap<Uuid, ImportRowMatchDto> {
        rows.iter()
            .map(|row| (row.id, ImportRowMatchDto::default()))
            .collect()
    }

    fn stage_of(rows: Vec<StagedTransactionDto>) -> SessionStage {
        SessionStage {
            staged: rows,
            unstageable: Vec::new(),
            joined: HashMap::new(),
        }
    }

    fn joined_new(rows: Vec<StagedTransactionDto>) -> Vec<StagedTransactionDto> {
        let matches = all_new(&rows);
        let mut stage = stage_of(rows);
        join_new(&mut stage, &matches, &[]);
        stage.staged
    }

    fn moves(rows: &[StagedTransactionDto]) -> Vec<(Uuid, Uuid, Decimal)> {
        rows.iter()
            .filter_map(|row| match &row.transaction.transaction_type {
                TransactionTypeDto::CashBalanceTransfer(transfer) => Some((
                    transfer.outgoing_change.account_id,
                    transfer.incoming_change.account_id,
                    transfer.incoming_change.quantity,
                )),
                _ => None,
            })
            .collect()
    }

    fn decision(id: Uuid, verb: ImportVerbDto) -> ImportStagedDecisionDto {
        ImportStagedDecisionDto {
            id,
            ask_key: None,
            verb,
            transaction_id: None,
        }
    }

    #[test]
    fn money_out_of_one_account_and_into_another_on_one_day_is_one_balance_transfer() {
        let out = row(1, CURRENT, datetime!(2026-11-15 10:00 UTC), "-200", true);
        let incoming = row(2, JOINT, datetime!(2026-11-15 14:30 UTC), "200", true);
        let rows = joined_new(vec![out.clone(), incoming.clone()]);

        let [transfer] = rows.as_slice() else {
            panic!("expected one row, got {rows:?}");
        };
        assert_eq!(transfer.id, joined_id(out.id, incoming.id));
        assert_eq!(transfer.file_ids, vec![file(CURRENT), file(JOINT)]);
        assert_eq!(transfer.transaction.date, out.transaction.date);
        assert_eq!(
            transfer.movements,
            [out.movements, incoming.movements].concat()
        );
        assert!(transfer.transaction.fee_entries.is_empty());
        let TransactionTypeDto::CashBalanceTransfer(metadata) =
            &transfer.transaction.transaction_type
        else {
            panic!("expected a balance transfer");
        };
        let entry = |entry: &EntryDto| (entry.asset_id, entry.account_id, entry.quantity);
        assert_eq!(
            (
                entry(&metadata.outgoing_change),
                entry(&metadata.incoming_change)
            ),
            (
                (EUR, CURRENT, "-200".parse().unwrap()),
                (EUR, JOINT, "200".parse().unwrap())
            )
        );
    }

    #[test]
    fn the_same_printed_time_joins_any_cash_rows_and_the_same_day_only_rows_called_transfers() {
        let rows = joined_new(vec![
            row(1, CURRENT, datetime!(2026-11-15 09:15 UTC), "-80", false),
            row(2, JOINT, datetime!(2026-11-15 09:15 UTC), "80", false),
            row(3, CURRENT, datetime!(2026-11-16 09:15 UTC), "-30", false),
            row(4, JOINT, datetime!(2026-11-16 18:40 UTC), "30", true),
            row(5, CURRENT, datetime!(2026-11-17 0:00 UTC), "-20", false),
            row(6, JOINT, datetime!(2026-11-17 0:00 UTC), "20", false),
            row(7, CURRENT, datetime!(2026-11-18 0:00 UTC), "-10", true),
            row(8, JOINT, datetime!(2026-11-18 0:00 UTC), "10", true),
        ]);

        assert_eq!(
            moves(&rows),
            vec![
                (CURRENT, JOINT, "80".parse().unwrap()),
                (CURRENT, JOINT, "10".parse().unwrap())
            ]
        );
        assert_eq!(rows.len(), 6);
    }

    #[test]
    fn a_side_with_two_possible_partners_joins_only_the_one_its_printed_time_names() {
        let timed = joined_new(vec![
            row(1, CURRENT, datetime!(2026-11-15 10:00 UTC), "-50", true),
            row(2, JOINT, datetime!(2026-11-15 10:00 UTC), "50", true),
            row(3, SAVINGS, datetime!(2026-11-15 15:00 UTC), "50", true),
        ]);
        assert_eq!(moves(&timed), vec![(CURRENT, JOINT, "50".parse().unwrap())]);

        let undated = joined_new(vec![
            row(1, CURRENT, datetime!(2026-11-15 0:00 UTC), "-50", true),
            row(2, JOINT, datetime!(2026-11-15 0:00 UTC), "50", true),
            row(3, SAVINGS, datetime!(2026-11-15 0:00 UTC), "50", true),
        ]);
        assert_eq!(moves(&undated), Vec::new());
        assert_eq!(undated.len(), 3);
    }

    #[test]
    fn another_amount_currency_day_or_the_same_account_is_not_the_other_side() {
        let at = datetime!(2026-11-15 10:00 UTC);
        let mut pounds = row(4, JOINT, at, "40", true);
        let TransactionTypeDto::CashTransferIn(metadata) = &mut pounds.transaction.transaction_type
        else {
            unreachable!();
        };
        metadata.entry.asset_id = GBP;
        pounds.movements[0].asset_id = GBP;
        let rows = joined_new(vec![
            row(1, CURRENT, at, "-200", true),
            row(2, JOINT, at, "199.99", true),
            row(3, CURRENT, at, "-40", true),
            pounds,
            row(5, CURRENT, at, "-60", true),
            row(6, JOINT, datetime!(2026-11-16 10:00 UTC), "60", true),
            row(7, CURRENT, at, "-70", true),
            row(8, CURRENT, at, "70", true),
        ]);

        assert_eq!(moves(&rows), Vec::new());
        assert_eq!(rows.len(), 8);
    }

    #[test]
    fn a_row_with_a_fee_is_not_a_side() {
        let at = datetime!(2026-11-15 10:00 UTC);
        let mut charged = row(1, CURRENT, at, "-200", true);
        charged.transaction.fee_entries.push(FeeEntryDto {
            entry: EntryDto::new(EUR, CURRENT, "-1".parse().unwrap()),
            entry_type: FeeEntryTypesDto::Transaction,
        });
        let mut itemised = row(3, CURRENT, at, "-90", true);
        itemised.movements[0].itemised_fee = Some("-1".parse().unwrap());

        let rows = joined_new(vec![
            charged,
            row(2, JOINT, at, "200", true),
            itemised,
            row(4, JOINT, at, "90", true),
        ]);

        assert_eq!(moves(&rows), Vec::new());
    }

    #[test]
    fn a_side_already_recorded_asked_about_or_decided_on_its_own_stays_apart() {
        let at = datetime!(2026-11-15 10:00 UTC);
        let rows = vec![
            row(1, CURRENT, at, "-10", true),
            row(2, JOINT, at, "10", true),
            row(3, CURRENT, at, "-20", true),
            row(4, JOINT, at, "20", true),
            row(5, CURRENT, at, "-30", true),
            row(6, JOINT, at, "30", true),
        ];
        let mut matches = all_new(&rows);
        matches.insert(
            rows[0].id,
            ImportRowMatchDto {
                verb: ImportVerbDto::Skip,
                transaction_id: Some(Uuid::from_u128(77)),
                ..ImportRowMatchDto::default()
            },
        );
        matches.get_mut(&rows[3].id).unwrap().near_miss = vec![ImportNearMissCandidateDto {
            amend: false,
            parts: Vec::new(),
            candidate: MatchCandidateDto {
                id: Uuid::from_u128(78),
                in_ledger: true,
                confirmed: false,
                several_transactions: false,
                leaves_movement_unrecorded: false,
                account_id: Some(JOINT),
                asset_id: EUR,
                quantity: "20".parse().unwrap(),
                date: date!(2026 - 11 - 15),
                description: None,
            },
        }];
        let decisions = [decision(rows[4].id, ImportVerbDto::Add)];

        let mut stage = stage_of(rows);
        join_new(&mut stage, &matches, &decisions);

        assert_eq!(moves(&stage.staged), Vec::new());
        assert_eq!(stage.staged.len(), 6);
        assert!(stage.joined.is_empty());
    }

    #[test]
    fn a_decided_move_stays_one_row_whatever_matching_says_now() {
        let out = row(1, CURRENT, datetime!(2026-11-15 10:00 UTC), "-200", true);
        let incoming = row(2, JOINT, datetime!(2026-11-15 10:00 UTC), "200", true);
        let id = joined_id(out.id, incoming.id);
        let decisions = [decision(id, ImportVerbDto::Skip)];

        let mut stage = stage_of(vec![out.clone(), incoming.clone()]);
        join_decided(&mut stage, &decisions);
        let ids = |stage: &SessionStage| stage.staged.iter().map(|row| row.id).collect::<Vec<_>>();
        assert_eq!(ids(&stage), vec![id]);
        assert_eq!(stage.joined[&id], [out.id, incoming.id]);

        let mut matches = all_new(&[out.clone(), incoming.clone()]);
        matches.get_mut(&out.id).unwrap().verb = ImportVerbDto::Skip;
        join_new(&mut stage, &matches, &decisions);
        assert_eq!(ids(&stage), vec![id]);

        let mut undecided = stage_of(vec![out, incoming]);
        join_decided(&mut undecided, &[]);
        assert_eq!(undecided.staged.len(), 2);
    }

    #[test]
    fn a_move_joins_the_same_way_whatever_order_the_rows_come_in() {
        let rows = vec![
            row(1, CURRENT, datetime!(2026-11-15 10:00 UTC), "-50", true),
            row(2, JOINT, datetime!(2026-11-15 10:00 UTC), "50", true),
            row(3, SAVINGS, datetime!(2026-11-15 12:00 UTC), "-50", true),
            row(4, CURRENT, datetime!(2026-11-15 12:00 UTC), "50", true),
        ];
        let ids = |rows: Vec<StagedTransactionDto>| {
            let mut ids: Vec<Uuid> = joined_new(rows).iter().map(|row| row.id).collect();
            ids.sort();
            ids
        };
        let reversed: Vec<StagedTransactionDto> = rows.iter().rev().cloned().collect();

        assert_eq!(ids(rows.clone()), ids(reversed));
        assert_eq!(moves(&joined_new(rows)).len(), 2);
    }
}
