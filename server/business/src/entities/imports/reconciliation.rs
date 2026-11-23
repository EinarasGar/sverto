use std::collections::{BTreeMap, HashMap, HashSet};

use rust_decimal::Decimal;
use time::Date;
use uuid::Uuid;

use crate::dtos::imports::import_staging_dto::{
    ImportClaimDto, ImportNearMissCandidateDto, ImportPartsDto, ImportRecordedOnDto,
    ImportRowMatchDto, ImportStagedDecisionDto, ImportVerbDto, StagedMovementDto,
    StagedTransactionDto,
};
use crate::dtos::ledger_matching_dto::{
    IncomingMovementDto, LedgerMatchBatchDto, MatchCandidateDto, MatchOutcomeDto,
    SettledMovementDto, SourceKindDto,
};
use crate::dtos::transaction_dto::TransactionTypeDto;
use crate::entities::ledger_matching::may_amend;

pub fn parts_ask_key(row_id: Uuid) -> String {
    format!("parts:{row_id}")
}

pub fn answered_parts(decisions: &[ImportStagedDecisionDto]) -> HashMap<String, Vec<Uuid>> {
    let mut parts: HashMap<String, Vec<Uuid>> = HashMap::new();
    for decision in decisions
        .iter()
        .filter(|decision| decision.verb == ImportVerbDto::Skip)
    {
        if let (Some(ask_key), Some(transaction_id)) = (&decision.ask_key, decision.transaction_id)
        {
            parts
                .entry(ask_key.clone())
                .or_default()
                .push(transaction_id);
        }
    }
    for transaction_ids in parts.values_mut() {
        transaction_ids.sort();
    }
    parts
}

fn purchase_group(row: &StagedTransactionDto) -> Option<(String, i32)> {
    let [movement] = &row.movements[..] else {
        return None;
    };
    let TransactionTypeDto::Regular(purchase) = &row.transaction.transaction_type else {
        return None;
    };
    let description = purchase
        .description
        .clone()
        .or_else(|| movement.description.clone())?;
    Some((description, purchase.category_id))
}

pub fn parts_of(
    row: &StagedTransactionDto,
    answered: &HashMap<String, Vec<Uuid>>,
) -> Option<ImportPartsDto> {
    let transaction_ids = answered
        .get(&parts_ask_key(row.id))
        .filter(|transaction_ids| transaction_ids.len() >= 2)?;
    let (description, category_id) = purchase_group(row)?;
    Some(ImportPartsDto {
        transaction_ids: transaction_ids.clone(),
        description,
        category_id,
    })
}

pub struct SessionReconciliation<'a> {
    rows: &'a [StagedTransactionDto],
    positions: Vec<Vec<(usize, usize)>>,
    batches: Vec<LedgerMatchBatchDto>,
}

impl<'a> SessionReconciliation<'a> {
    pub fn new(rows: &'a [StagedTransactionDto], decisions: &[ImportStagedDecisionDto]) -> Self {
        let parts = answered_parts(decisions);
        let decisions: HashMap<Uuid, &ImportStagedDecisionDto> = decisions
            .iter()
            .filter(|decision| decision.ask_key.is_none())
            .map(|decision| (decision.id, decision))
            .collect();

        let mut refs = external_refs(rows.iter().flat_map(|row| &row.movements)).into_iter();
        let mut first_dates: HashMap<Uuid, Date> = HashMap::new();
        let mut incoming: Vec<Vec<IncomingMovementDto>> = Vec::with_capacity(rows.len());
        for row in rows {
            let mut movements = Vec::with_capacity(row.movements.len());
            for movement in &row.movements {
                let first = first_dates
                    .entry(movement.source_id)
                    .or_insert(movement.date);
                *first = (*first).min(movement.date);
                let external_ref = refs.next().expect("one reference per movement");
                movements.push(observation(row, movement, external_ref));
            }
            incoming.push(movements);
        }

        let mut by_source: BTreeMap<(Date, Uuid), (LedgerMatchBatchDto, Vec<(usize, usize)>)> =
            BTreeMap::new();
        for (row_index, (row, movements)) in rows.iter().zip(incoming).enumerate() {
            let decision = decisions.get(&row.id);
            let row_parts = parts_of(row, &parts).filter(|_| decision.is_none());
            for (movement_index, (staged, movement)) in
                row.movements.iter().zip(movements).enumerate()
            {
                let (batch, batch_positions) = by_source
                    .entry((first_dates[&staged.source_id], staged.source_id))
                    .or_default();
                if let Some(row_parts) = &row_parts {
                    batch
                        .settled
                        .extend(
                            row_parts
                                .transaction_ids
                                .iter()
                                .map(|part| SettledMovementDto {
                                    movement: movement.clone(),
                                    transaction_id: Some(*part),
                                }),
                        );
                    continue;
                }
                match decision.map(|decision| (decision.verb, decision.transaction_id)) {
                    None => {
                        batch.incoming.push(movement);
                        batch_positions.push((row_index, movement_index));
                    }
                    Some((ImportVerbDto::Add, _)) => batch.settled.push(SettledMovementDto {
                        movement,
                        transaction_id: None,
                    }),
                    Some((_, Some(transaction_id))) => batch.settled.push(SettledMovementDto {
                        movement,
                        transaction_id: Some(transaction_id),
                    }),
                    Some((_, None)) => {}
                }
            }
        }
        let (batches, positions) = by_source.into_values().unzip();
        Self {
            rows,
            positions,
            batches,
        }
    }

    pub fn batches(&self) -> &[LedgerMatchBatchDto] {
        &self.batches
    }

    pub fn resolve(&self, outcomes: Vec<Vec<MatchOutcomeDto>>) -> HashMap<Uuid, ImportRowMatchDto> {
        let mut by_row: BTreeMap<usize, Vec<(usize, MatchOutcomeDto)>> = BTreeMap::new();
        for (batch, batch_positions) in outcomes.into_iter().zip(&self.positions) {
            for (outcome, &(row, movement)) in batch.into_iter().zip(batch_positions) {
                by_row.entry(row).or_default().push((movement, outcome));
            }
        }
        by_row
            .into_iter()
            .map(|(row, outcomes)| {
                let row = &self.rows[row];
                (row.id, resolve_row(row, &outcomes))
            })
            .collect()
    }
}

fn resolve_row(
    row: &StagedTransactionDto,
    outcomes: &[(usize, MatchOutcomeDto)],
) -> ImportRowMatchDto {
    let parts = outcomes.iter().find_map(|(_, outcome)| match outcome {
        MatchOutcomeDto::Parts { parts } => Some(parts),
        _ => None,
    });
    if let (Some(parts), Some(_)) = (parts, purchase_group(row)) {
        let first = &parts[0];
        return ImportRowMatchDto {
            near_miss: vec![ImportNearMissCandidateDto {
                candidate: MatchCandidateDto {
                    id: first.id,
                    in_ledger: true,
                    confirmed: false,
                    several_transactions: true,
                    leaves_movement_unrecorded: false,
                    account_id: first.account_id,
                    asset_id: first.asset_id,
                    quantity: parts.iter().map(|part| part.quantity).sum(),
                    date: first.date,
                    description: None,
                },
                amend: false,
                parts: parts.clone(),
            }],
            ..ImportRowMatchDto::default()
        };
    }
    let settled_elsewhere = outcomes.iter().any(|(_, outcome)| {
        matches!(
            outcome,
            MatchOutcomeDto::Same { .. } | MatchOutcomeDto::Recorded { .. }
        )
    });
    let any_new = outcomes.iter().any(|(_, outcome)| {
        matches!(
            outcome,
            MatchOutcomeDto::New | MatchOutcomeDto::Parts { .. }
        )
    });
    let any_ambiguous = outcomes
        .iter()
        .any(|(_, outcome)| matches!(outcome, MatchOutcomeDto::Ambiguous { .. }));

    if any_ambiguous || (any_new && settled_elsewhere) {
        return ImportRowMatchDto {
            near_miss: near_miss(row, outcomes),
            ..ImportRowMatchDto::default()
        };
    }
    if settled_elsewhere {
        return ImportRowMatchDto {
            verb: ImportVerbDto::Skip,
            transaction_id: outcomes
                .iter()
                .find_map(|(_, outcome)| ledger_transaction(outcome)),
            claims: outcomes
                .iter()
                .flat_map(|(movement, outcome)| claims(&row.movements[*movement], outcome))
                .collect(),
            near_miss: Vec::new(),
            recorded_on: recorded_on(row, outcomes),
        };
    }
    ImportRowMatchDto::default()
}

pub fn statement_stream(source_id: Uuid) -> String {
    format!("statement:{source_id}")
}

pub fn external_refs<'a>(
    movements: impl IntoIterator<Item = &'a StagedMovementDto>,
) -> Vec<String> {
    let mut occurrences: HashMap<(Uuid, Uuid, i32, Date, Decimal), usize> = HashMap::new();
    movements
        .into_iter()
        .map(|movement| {
            let occurrence = occurrences
                .entry((
                    movement.source_id,
                    movement.account_id,
                    movement.asset_id,
                    movement.date,
                    movement.quantity,
                ))
                .or_insert(0);
            *occurrence += 1;
            format!(
                "{}|{}|{occurrence}",
                movement.date,
                movement.quantity.normalize()
            )
        })
        .collect()
}

fn recorded_on(
    row: &StagedTransactionDto,
    outcomes: &[(usize, MatchOutcomeDto)],
) -> Vec<Option<ImportRecordedOnDto>> {
    let mut recorded_on = vec![None; row.movements.len()];
    for (movement, outcome) in outcomes {
        let MatchOutcomeDto::Same { targets } = outcome else {
            continue;
        };
        let largest = |in_ledger: bool| {
            targets
                .iter()
                .filter(|target| target.in_ledger == in_ledger)
                .max_by_key(|target| target.quantity.abs())
        };
        recorded_on[*movement] = match (largest(true), largest(false)) {
            (Some(target), _) => Some(ImportRecordedOnDto::Transaction(target.id)),
            (None, Some(target)) => Some(ImportRecordedOnDto::Row(target.id)),
            (None, None) => None,
        };
    }
    recorded_on
}

fn observation(
    row: &StagedTransactionDto,
    movement: &StagedMovementDto,
    external_ref: String,
) -> IncomingMovementDto {
    let fees: Decimal = row
        .transaction
        .fee_entries
        .iter()
        .filter(|fee| {
            fee.entry.account_id == movement.account_id && fee.entry.asset_id == movement.asset_id
        })
        .map(|fee| fee.entry.quantity)
        .sum();
    IncomingMovementDto {
        id: row.id,
        kind: SourceKindDto::Statement,
        stream: statement_stream(movement.source_id),
        external_ref: Some(external_ref),
        account_id: Some(movement.account_id),
        asset_id: movement.asset_id,
        quantity: movement.quantity,
        date: movement.date,
        description: movement.description.clone(),
        itemised_fee: movement.itemised_fee,
        is_fee: matches!(
            row.transaction.transaction_type,
            TransactionTypeDto::AccountFees(_)
        ) || fees == movement.quantity,
    }
}

fn ledger_transaction(outcome: &MatchOutcomeDto) -> Option<Uuid> {
    match outcome {
        MatchOutcomeDto::Recorded { transaction_id } => Some(*transaction_id),
        MatchOutcomeDto::Same { targets } => targets
            .iter()
            .find(|target| target.in_ledger)
            .map(|target| target.id),
        _ => None,
    }
}

fn claims(staged: &StagedMovementDto, outcome: &MatchOutcomeDto) -> Vec<ImportClaimDto> {
    let claim = |transaction_id: Uuid, account_id: Option<Uuid>, asset_id: i32| ImportClaimDto {
        transaction_id,
        account_id: account_id.unwrap_or(staged.account_id),
        asset_id,
        booked_on: staged.date,
    };
    match outcome {
        MatchOutcomeDto::Recorded { transaction_id } => {
            vec![claim(*transaction_id, None, staged.asset_id)]
        }
        MatchOutcomeDto::Same { targets } => targets
            .iter()
            .filter(|target| target.in_ledger)
            .map(|target| claim(target.id, target.account_id, target.asset_id))
            .collect(),
        _ => Vec::new(),
    }
}

fn near_miss(
    row: &StagedTransactionDto,
    outcomes: &[(usize, MatchOutcomeDto)],
) -> Vec<ImportNearMissCandidateDto> {
    let mut offered: Vec<(usize, &MatchCandidateDto)> = Vec::new();
    for (movement, outcome) in outcomes {
        let targets: &[MatchCandidateDto] = match outcome {
            MatchOutcomeDto::Ambiguous { ranked } => ranked,
            MatchOutcomeDto::Same { targets } => targets,
            _ => &[],
        };
        offered.extend(targets.iter().map(|target| (*movement, target)));
    }
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut candidates = Vec::new();
    for (_, target) in &offered {
        if !seen.insert(target.id) {
            continue;
        }
        let legs = || offered.iter().filter(|(_, leg)| leg.id == target.id);
        let correctable = legs().all(|(_, leg)| may_amend(leg));
        let agrees = row.movements.iter().enumerate().all(|(index, staged)| {
            legs().any(|(movement, leg)| *movement == index && leg.quantity == staged.quantity)
        });
        candidates.push(ImportNearMissCandidateDto {
            amend: correctable && !agrees,
            candidate: (*target).clone(),
            parts: Vec::new(),
        });
    }
    candidates
}

#[cfg(test)]
mod tests {
    use time::macros::{date, datetime};

    use super::*;
    use crate::dtos::entry_dto::EntryDto;
    use crate::dtos::fee_entry_dto::FeeEntryDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::transaction_dto::{
        AccountFeesMetadataDto, CashTransferOutMetadataDto, RegularTransactionMetadataDto,
        TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
    };

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const SOURCE_A: Uuid = Uuid::from_u128(10);
    const SOURCE_B: Uuid = Uuid::from_u128(11);

    fn day() -> Date {
        date!(2026 - 03 - 02)
    }

    fn movement(source_id: Uuid, on: Date, quantity: &str) -> StagedMovementDto {
        StagedMovementDto {
            account_id: ACCOUNT,
            asset_id: 45,
            quantity: quantity.parse().unwrap(),
            source_id,
            date: on,
            description: Some("TESCO".to_string()),
            itemised_fee: None,
            external_id: None,
        }
    }

    fn row(id: u128, movements: Vec<StagedMovementDto>) -> StagedTransactionDto {
        StagedTransactionDto {
            id: Uuid::from_u128(id),
            source_ids: Vec::new(),
            transaction: TransactionDto {
                transaction_id: None,
                date: datetime!(2026-03-01 0:00 UTC),
                visibility: TransactionVisibilityDto::Default,
                fee_entries: Vec::new(),
                transaction_type: TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                    entry: EntryDto::new(45, ACCOUNT, "-5".parse().unwrap()),
                }),
            },
            movements,
        }
    }

    fn decision(id: u128, verb: ImportVerbDto, target: Option<u128>) -> ImportStagedDecisionDto {
        ImportStagedDecisionDto {
            id: Uuid::from_u128(id),
            ask_key: None,
            verb,
            transaction_id: target.map(Uuid::from_u128),
        }
    }

    fn candidate(id: u128, in_ledger: bool, confirmed: bool, quantity: &str) -> MatchCandidateDto {
        MatchCandidateDto {
            id: Uuid::from_u128(id),
            in_ledger,
            confirmed,
            several_transactions: false,
            leaves_movement_unrecorded: false,
            account_id: Some(ACCOUNT),
            asset_id: 45,
            quantity: quantity.parse().unwrap(),
            date: day(),
            description: Some("Tesco".to_string()),
        }
    }

    fn resolve_one(
        rows: &[StagedTransactionDto],
        outcomes: Vec<MatchOutcomeDto>,
    ) -> ImportRowMatchDto {
        SessionReconciliation::new(rows, &[])
            .resolve(vec![outcomes])
            .remove(&rows[0].id)
            .unwrap()
    }

    #[test]
    fn a_matched_row_is_skipped_against_the_ledger_transaction() {
        let rows = [row(1, vec![movement(SOURCE_A, day(), "-5")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Same {
                targets: vec![candidate(7, true, true, "-5")],
            }],
        );
        assert_eq!(resolved.verb, ImportVerbDto::Skip);
        assert_eq!(resolved.transaction_id, Some(Uuid::from_u128(7)));
    }

    #[test]
    fn a_skipped_row_claims_every_ledger_movement_it_matched() {
        let rows = [row(1, vec![movement(SOURCE_A, day(), "-5")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Same {
                targets: vec![
                    candidate(7, true, true, "-4"),
                    candidate(8, true, true, "-1"),
                    candidate(9, false, true, "-5"),
                ],
            }],
        );
        let ids: Vec<Uuid> = resolved.claims.iter().map(|c| c.transaction_id).collect();
        assert_eq!(ids, [7, 8].map(Uuid::from_u128));
    }

    #[test]
    fn a_row_matching_another_file_is_skipped_with_no_transaction() {
        let rows = [row(1, vec![movement(SOURCE_A, day(), "-5")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Same {
                targets: vec![candidate(2, false, true, "-5")],
            }],
        );
        assert_eq!(resolved.verb, ImportVerbDto::Skip);
        assert_eq!(resolved.transaction_id, None);
    }

    #[test]
    fn an_ambiguous_row_stays_add_and_lists_its_candidates() {
        let rows = [row(1, vec![movement(SOURCE_A, day(), "-5")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Ambiguous {
                ranked: vec![
                    candidate(7, true, true, "-5"),
                    candidate(8, true, false, "-4"),
                ],
            }],
        );
        assert_eq!(resolved.verb, ImportVerbDto::Add);
        assert!(!resolved.near_miss[0].amend);
        assert!(resolved.near_miss[1].amend);
    }

    #[test]
    fn correcting_is_offered_when_any_movement_of_the_row_differs_from_the_transaction() {
        let rows = [row(
            1,
            vec![
                StagedMovementDto {
                    asset_id: 42,
                    ..movement(SOURCE_A, day(), "10")
                },
                movement(SOURCE_A, day(), "-1001.50"),
            ],
        )];
        let units = MatchOutcomeDto::Same {
            targets: vec![MatchCandidateDto {
                asset_id: 42,
                ..candidate(7, true, false, "10")
            }],
        };
        let unmatched_cash = resolve_one(&rows, vec![units.clone(), MatchOutcomeDto::New]);
        assert_eq!(unmatched_cash.near_miss.len(), 1);
        assert!(unmatched_cash.near_miss[0].amend);

        let rounded_cash = resolve_one(
            &rows,
            vec![
                units,
                MatchOutcomeDto::Ambiguous {
                    ranked: vec![candidate(7, true, false, "-1000")],
                },
            ],
        );
        assert_eq!(rounded_cash.near_miss.len(), 1);
        assert!(rounded_cash.near_miss[0].amend);
    }

    #[test]
    fn a_target_standing_for_several_transactions_is_never_corrected() {
        let rows = [row(1, vec![movement(SOURCE_A, day(), "-50.40")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Ambiguous {
                ranked: vec![MatchCandidateDto {
                    several_transactions: true,
                    ..candidate(7, true, false, "-50")
                }],
            }],
        );
        assert!(!resolved.near_miss[0].amend);
    }

    #[test]
    fn a_row_only_partly_matched_is_a_near_miss() {
        let rows = [row(
            1,
            vec![
                movement(SOURCE_A, day(), "-5"),
                StagedMovementDto {
                    asset_id: 42,
                    ..movement(SOURCE_A, day(), "6")
                },
            ],
        )];
        let resolved = resolve_one(
            &rows,
            vec![
                MatchOutcomeDto::Same {
                    targets: vec![candidate(7, true, true, "-5")],
                },
                MatchOutcomeDto::New,
            ],
        );
        assert_eq!(resolved.verb, ImportVerbDto::Add);
        assert_eq!(resolved.near_miss.len(), 1);
    }

    #[test]
    fn a_transaction_with_no_entry_for_a_movement_nothing_matched_is_asked_about_and_never_corrected(
    ) {
        let rows = [row(
            1,
            vec![
                movement(SOURCE_A, day(), "-500"),
                StagedMovementDto {
                    account_id: Uuid::from_u128(2),
                    ..movement(SOURCE_A, day(), "500")
                },
            ],
        )];
        for confirmed in [false, true] {
            let one_sided = MatchOutcomeDto::Same {
                targets: vec![MatchCandidateDto {
                    leaves_movement_unrecorded: true,
                    ..candidate(7, true, confirmed, "-500")
                }],
            };
            let resolved = resolve_one(&rows, vec![one_sided, MatchOutcomeDto::New]);
            assert_eq!(resolved.verb, ImportVerbDto::Add);
            assert_eq!(resolved.near_miss.len(), 1);
            assert!(resolved.near_miss[0].candidate.leaves_movement_unrecorded);
            assert!(!resolved.near_miss[0].amend);
        }
    }

    #[test]
    fn a_row_staged_as_account_fees_or_made_of_a_fee_entry_is_a_fee() {
        let other_asset = StagedMovementDto {
            asset_id: 42,
            ..movement(SOURCE_A, day(), "-0.01")
        };
        let mut purchase = row(1, vec![movement(SOURCE_A, day(), "-5"), other_asset]);
        purchase.transaction.fee_entries = vec![FeeEntryDto {
            entry: EntryDto::new(42, ACCOUNT, "-0.01".parse().unwrap()),
            entry_type: FeeEntryTypesDto::Transaction,
        }];
        let mut charge = row(2, vec![movement(SOURCE_A, day(), "-1.50")]);
        charge.transaction.transaction_type =
            TransactionTypeDto::AccountFees(AccountFeesMetadataDto {
                entry: EntryDto::new(45, ACCOUNT, "-1.50".parse().unwrap()),
            });

        let rows = [purchase, charge];
        let reconciliation = SessionReconciliation::new(&rows, &[]);
        let fees: Vec<bool> = reconciliation.batches()[0]
            .incoming
            .iter()
            .map(|movement| movement.is_fee)
            .collect();
        assert_eq!(fees, [false, true, true]);
    }

    #[test]
    fn decided_rows_are_settled_in_their_files_place_and_never_rematched() {
        let later = date!(2026 - 03 - 05);
        let rows = [
            row(1, vec![movement(SOURCE_A, day(), "-5")]),
            row(2, vec![movement(SOURCE_B, later, "-5")]),
            row(3, vec![movement(SOURCE_B, later, "-6")]),
            row(4, vec![movement(SOURCE_B, later, "-7")]),
        ];
        let decisions = [
            decision(2, ImportVerbDto::Add, None),
            decision(3, ImportVerbDto::Skip, Some(7)),
            decision(4, ImportVerbDto::Skip, None),
        ];
        let reconciliation = SessionReconciliation::new(&rows, &decisions);
        let batches = reconciliation.batches();

        assert_eq!(batches[0].incoming.len(), 1);
        assert!(batches[0].settled.is_empty());
        assert!(batches[1].incoming.is_empty());
        assert_eq!(batches[1].settled.len(), 2);
        assert_eq!(batches[1].settled[0].transaction_id, None);
        assert_eq!(
            batches[1].settled[1].transaction_id,
            Some(Uuid::from_u128(7))
        );
    }

    #[test]
    fn identical_rows_get_different_references_and_files_go_in_date_order() {
        let later = date!(2026 - 03 - 05);
        let rows = [
            row(1, vec![movement(SOURCE_B, later, "-5")]),
            row(2, vec![movement(SOURCE_A, day(), "-5")]),
            row(3, vec![movement(SOURCE_A, day(), "-5")]),
        ];
        let reconciliation = SessionReconciliation::new(&rows, &[]);
        let batches = reconciliation.batches();
        assert_eq!(
            batches[0].incoming[0].stream,
            format!("statement:{SOURCE_A}")
        );
        assert_ne!(
            batches[0].incoming[0].external_ref,
            batches[0].incoming[1].external_ref
        );
        assert_eq!(
            batches[1].incoming[0].stream,
            format!("statement:{SOURCE_B}")
        );
    }

    fn purchase(id: u128, movements: Vec<StagedMovementDto>) -> StagedTransactionDto {
        let mut purchase = row(id, movements);
        purchase.transaction.transaction_type =
            TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: Some("TESCO".to_string()),
                entry: EntryDto::new(45, ACCOUNT, "-50".parse().unwrap()),
                category_id: 7,
            });
        purchase
    }

    #[test]
    fn a_purchase_typed_item_by_item_is_one_candidate_standing_for_its_parts() {
        let parts = vec![
            candidate(7, true, false, "-20"),
            candidate(8, true, false, "-30"),
        ];
        let rows = [purchase(1, vec![movement(SOURCE_A, day(), "-50")])];
        let resolved = resolve_one(
            &rows,
            vec![MatchOutcomeDto::Parts {
                parts: parts.clone(),
            }],
        );
        assert_eq!(resolved.verb, ImportVerbDto::Add);
        let [near] = &resolved.near_miss[..] else {
            panic!("expected one candidate, got {:?}", resolved.near_miss);
        };
        assert_eq!(near.candidate.id, Uuid::from_u128(7));
        assert_eq!(near.candidate.quantity, "-50".parse::<Decimal>().unwrap());
        assert!(near.candidate.several_transactions && !near.amend);
        assert_eq!(near.parts, parts);

        let transfer = [row(1, vec![movement(SOURCE_A, day(), "-50")])];
        let resolved = resolve_one(&transfer, vec![MatchOutcomeDto::Parts { parts }]);
        assert!(resolved.near_miss.is_empty());
    }

    #[test]
    fn a_row_answered_as_parts_is_settled_against_each_of_them() {
        let rows = [purchase(1, vec![movement(SOURCE_A, day(), "-50")])];
        let answer = |part: u128| ImportStagedDecisionDto {
            id: Uuid::from_u128(100 + part),
            ask_key: Some(parts_ask_key(Uuid::from_u128(1))),
            verb: ImportVerbDto::Skip,
            transaction_id: Some(Uuid::from_u128(part)),
        };
        let reconciliation = SessionReconciliation::new(&rows, &[answer(8), answer(7)]);
        let batch = &reconciliation.batches()[0];
        assert!(batch.incoming.is_empty());
        let settled: Vec<Option<Uuid>> = batch
            .settled
            .iter()
            .map(|settled| settled.transaction_id)
            .collect();
        assert_eq!(
            settled,
            vec![Some(Uuid::from_u128(7)), Some(Uuid::from_u128(8))]
        );
    }

    #[test]
    fn parts_found_for_one_movement_of_a_row_leave_it_new_like_any_other() {
        let rows = [row(
            1,
            vec![
                movement(SOURCE_A, day(), "-50"),
                movement(SOURCE_A, day(), "-1"),
            ],
        )];
        let resolved = resolve_one(
            &rows,
            vec![
                MatchOutcomeDto::Parts {
                    parts: vec![
                        candidate(7, true, false, "-20"),
                        candidate(8, true, false, "-30"),
                    ],
                },
                MatchOutcomeDto::Same {
                    targets: vec![candidate(9, true, true, "-1")],
                },
            ],
        );
        let offered: Vec<Uuid> = resolved
            .near_miss
            .iter()
            .map(|near| near.candidate.id)
            .collect();
        assert_eq!(offered, vec![Uuid::from_u128(9)]);
    }

    #[test]
    fn a_parts_answer_with_one_part_left_is_matched_afresh() {
        let rows = [purchase(1, vec![movement(SOURCE_A, day(), "-50")])];
        let answer = ImportStagedDecisionDto {
            id: Uuid::from_u128(107),
            ask_key: Some(parts_ask_key(Uuid::from_u128(1))),
            verb: ImportVerbDto::Skip,
            transaction_id: Some(Uuid::from_u128(7)),
        };
        let reconciliation = SessionReconciliation::new(&rows, &[answer]);
        let batch = &reconciliation.batches()[0];
        assert_eq!(batch.incoming.len(), 1);
        assert!(batch.settled.is_empty());
    }
}
