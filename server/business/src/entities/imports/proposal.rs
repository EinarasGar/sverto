use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use rust_decimal::Decimal;
use uuid::Uuid;

use super::reconciliation::{answered_parts, parts_of};
use super::staging::SessionStage;
use crate::dtos::entry_dto::EntryDto;
use crate::dtos::imports::import_balance_dto::{ImportAdjustmentDto, ImportStartingBalanceDto};
use crate::dtos::imports::import_staging_dto::{
    ImportAmendRefusalDto, ImportClaimDto, ImportProposalDto, ImportProposalRowDto,
    ImportRecordedOnDto, ImportRowKindDto, ImportRowMatchDto, ImportStagedDecisionDto,
    ImportVerbDto, StagedMovementDto, StagedTransactionDto, UnstageableGroupDto,
};
use crate::dtos::transaction_dto::{
    AssetTransferInMetadataDto, AssetTransferOutMetadataDto, CashTransferInMetadataDto,
    CashTransferOutMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
};
use crate::entities::entries::entry::Entry;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

pub fn merge(
    stage: SessionStage,
    decisions: Vec<ImportStagedDecisionDto>,
    targets: Vec<TransactionDto>,
    mut matches: HashMap<Uuid, ImportRowMatchDto>,
    user_id: Uuid,
) -> ImportProposalDto {
    let parts = answered_parts(&decisions);
    let mut decisions: BTreeMap<Uuid, ImportStagedDecisionDto> = decisions
        .into_iter()
        .map(|decision| (decision.id, decision))
        .collect();
    let targets: HashMap<Uuid, TransactionDto> = targets
        .into_iter()
        .filter_map(|transaction| Some((transaction.transaction_id?, transaction)))
        .collect();
    let mut rows: Vec<ImportProposalRowDto> = Vec::new();

    for staged in stage.staged {
        let answered = parts_of(&staged, &parts);
        let StagedTransactionDto {
            id,
            file_ids,
            mut transaction,
            movements,
        } = staged;
        let kind = stage
            .joined
            .get(&id)
            .map_or(ImportRowKindDto::Movement, |sides| {
                ImportRowKindDto::JoinedTransfer(*sides)
            });
        let decision = match decisions.get(&id) {
            Some(decision) if decision.ask_key.is_none() => decisions.remove(&id),
            _ => None,
        };
        let row = match (decision, answered) {
            (None, Some(parts)) => {
                transaction.transaction_id = Some(parts.transaction_ids[0]);
                let claims = parts
                    .transaction_ids
                    .iter()
                    .flat_map(|part| {
                        movements.iter().map(move |movement| ImportClaimDto {
                            transaction_id: *part,
                            account_id: movement.account_id,
                            asset_id: movement.asset_id,
                            booked_on: movement.date,
                        })
                    })
                    .collect();
                ImportProposalRowDto {
                    id,
                    kind,
                    file_ids,
                    ask_key: None,
                    verb: ImportVerbDto::Skip,
                    verb_locked: true,
                    transaction,
                    near_miss: Vec::new(),
                    claims,
                    recorded_on: vec![
                        Some(ImportRecordedOnDto::Transaction(
                            parts.transaction_ids[0]
                        ));
                        movements.len()
                    ],
                    movements,
                    amend_refusal: None,
                    parts: Some(parts),
                }
            }
            (None, None) => {
                let matched = matches.remove(&id).unwrap_or_default();
                transaction.transaction_id = matched.transaction_id;
                ImportProposalRowDto {
                    id,
                    kind,
                    file_ids,
                    ask_key: None,
                    verb: matched.verb,
                    verb_locked: false,
                    transaction,
                    near_miss: matched.near_miss,
                    movements,
                    claims: matched.claims,
                    recorded_on: matched.recorded_on,
                    amend_refusal: None,
                    parts: None,
                }
            }
            (Some(decision), _) => {
                transaction.transaction_id = decision.transaction_id;
                let amend_refusal = if decision.verb == ImportVerbDto::Amend {
                    match decision.transaction_id.and_then(|id| targets.get(&id)) {
                        None => Some(ImportAmendRefusalDto::TargetGone),
                        Some(target) => {
                            match amended(target.clone(), &transaction, &movements, user_id) {
                                Ok(amended) => {
                                    transaction = amended;
                                    None
                                }
                                Err(refusal) => Some(refusal),
                            }
                        }
                    }
                } else {
                    None
                };
                let claims = decision
                    .transaction_id
                    .into_iter()
                    .flat_map(|transaction_id| {
                        movements.iter().map(move |movement| ImportClaimDto {
                            transaction_id,
                            account_id: movement.account_id,
                            asset_id: movement.asset_id,
                            booked_on: movement.date,
                        })
                    })
                    .collect();
                let recorded_on = vec![
                    decision
                        .transaction_id
                        .map(ImportRecordedOnDto::Transaction);
                    movements.len()
                ];
                ImportProposalRowDto {
                    id,
                    kind,
                    file_ids,
                    ask_key: None,
                    verb: decision.verb,
                    verb_locked: true,
                    transaction,
                    near_miss: Vec::new(),
                    movements,
                    claims,
                    recorded_on,
                    amend_refusal,
                    parts: None,
                }
            }
        };
        rows.push(row);
    }

    for decision in decisions.into_values() {
        if decision.verb != ImportVerbDto::Delete {
            continue;
        }
        if let Some(transaction) = decision
            .transaction_id
            .and_then(|id| targets.get(&id).cloned())
        {
            rows.push(ImportProposalRowDto {
                id: decision.id,
                kind: ImportRowKindDto::Movement,
                file_ids: Vec::new(),
                ask_key: decision.ask_key,
                verb: ImportVerbDto::Delete,
                verb_locked: true,
                transaction,
                near_miss: Vec::new(),
                movements: Vec::new(),
                claims: Vec::new(),
                recorded_on: Vec::new(),
                amend_refusal: None,
                parts: None,
            });
        }
    }

    proposed(rows, stage.unstageable)
}

pub fn add_starting_balances(
    proposal: ImportProposalDto,
    session_id: Uuid,
    starting_balances: Vec<ImportStartingBalanceDto>,
    decisions: &[ImportStagedDecisionDto],
    currency_assets: &HashSet<i32>,
) -> ImportProposalDto {
    if starting_balances.is_empty() {
        return proposal;
    }
    let mut rows = proposal.rows;
    for starting in starting_balances {
        let is_cash = currency_assets.contains(&starting.asset_id);
        if !is_cash && starting.quantity.is_sign_negative() {
            continue;
        }
        let id = Uuid::new_v5(
            &session_id,
            format!(
                "starting-balance:{}:{}:{}:{}",
                starting.account_id,
                starting.asset_id,
                starting.date,
                starting.quantity.normalize()
            )
            .as_bytes(),
        );
        let pin = decisions
            .iter()
            .find(|decision| decision.id == id && decision.ask_key.is_none());
        rows.push(ImportProposalRowDto {
            id,
            kind: ImportRowKindDto::StartingBalance(starting),
            file_ids: vec![starting.file_id],
            ask_key: None,
            verb: match pin.map(|pin| pin.verb) {
                Some(ImportVerbDto::Skip) => ImportVerbDto::Skip,
                _ => ImportVerbDto::Add,
            },
            verb_locked: pin.is_some(),
            transaction: TransactionDto {
                transaction_id: None,
                date: starting.date.midnight().assume_utc(),
                visibility: TransactionVisibilityDto::Default,
                fee_entries: Vec::new(),
                transaction_type: transfer(
                    is_cash,
                    EntryDto::new(starting.asset_id, starting.account_id, starting.quantity),
                ),
            },
            near_miss: Vec::new(),
            movements: Vec::new(),
            claims: Vec::new(),
            recorded_on: Vec::new(),
            amend_refusal: None,
            parts: None,
        });
    }
    proposed(rows, proposal.unstageable)
}

pub fn add_adjustments(
    proposal: ImportProposalDto,
    session_id: Uuid,
    adjustments: Vec<ImportAdjustmentDto>,
    decisions: &[ImportStagedDecisionDto],
    currency_assets: &HashSet<i32>,
) -> ImportProposalDto {
    let adjustments: Vec<ImportAdjustmentDto> = adjustments
        .into_iter()
        .filter(|adjustment| currency_assets.contains(&adjustment.asset_id))
        .collect();
    if adjustments.is_empty() {
        return proposal;
    }
    let mut rows = proposal.rows;
    for adjustment in adjustments {
        let id = Uuid::new_v5(
            &session_id,
            format!(
                "adjustment:{}:{}:{}:{}",
                adjustment.account_id,
                adjustment.asset_id,
                adjustment.date,
                adjustment.quantity.normalize()
            )
            .as_bytes(),
        );
        let pin = decisions
            .iter()
            .find(|decision| decision.id == id && decision.ask_key.is_none());
        rows.push(ImportProposalRowDto {
            id,
            kind: ImportRowKindDto::Adjustment(adjustment),
            file_ids: Vec::new(),
            ask_key: None,
            verb: match pin.map(|pin| pin.verb) {
                Some(ImportVerbDto::Add) => ImportVerbDto::Add,
                _ => ImportVerbDto::Skip,
            },
            verb_locked: pin.is_some(),
            transaction: TransactionDto {
                transaction_id: None,
                date: adjustment.date.midnight().assume_utc(),
                visibility: TransactionVisibilityDto::Default,
                fee_entries: Vec::new(),
                transaction_type: transfer(
                    true,
                    EntryDto::new(
                        adjustment.asset_id,
                        adjustment.account_id,
                        adjustment.quantity,
                    ),
                ),
            },
            near_miss: Vec::new(),
            movements: Vec::new(),
            claims: Vec::new(),
            recorded_on: Vec::new(),
            amend_refusal: None,
            parts: None,
        });
    }
    proposed(rows, proposal.unstageable)
}

fn transfer(is_cash: bool, entry: EntryDto) -> TransactionTypeDto {
    match (is_cash, entry.quantity.is_sign_positive()) {
        (true, true) => TransactionTypeDto::CashTransferIn(CashTransferInMetadataDto { entry }),
        (true, false) => TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto { entry }),
        (false, true) => TransactionTypeDto::AssetTransferIn(AssetTransferInMetadataDto { entry }),
        (false, false) => {
            TransactionTypeDto::AssetTransferOut(AssetTransferOutMetadataDto { entry })
        }
    }
}

fn amended(
    target: TransactionDto,
    statement: &TransactionDto,
    movements: &[StagedMovementDto],
    user_id: Uuid,
) -> Result<TransactionDto, ImportAmendRefusalDto> {
    let mut transaction = create_transaction_from_dto(target, user_id)
        .map_err(|_| ImportAmendRefusalDto::Unreadable)?;
    let entries = transaction.get_entries_mut();
    for movement in movements {
        let moves = |account_id: Uuid, asset_id: i32| {
            account_id == movement.account_id && asset_id == movement.asset_id
        };
        let itemised = statement
            .fee_entries
            .iter()
            .filter(|fee| moves(fee.entry.account_id, fee.entry.asset_id))
            .cloned()
            .map(Entry::try_from)
            .collect::<anyhow::Result<Vec<Entry>>>()
            .map_err(|_| ImportAmendRefusalDto::Unreadable)?;
        if !itemised.is_empty() {
            entries.retain(|entry| !(entry.is_fee() && moves(entry.account_id, entry.asset_id)));
            entries.extend(itemised);
        }
        let fees: Decimal = entries
            .iter()
            .filter(|entry| entry.is_fee() && moves(entry.account_id, entry.asset_id))
            .map(|entry| entry.quantity)
            .sum();
        let mut mains = entries
            .iter_mut()
            .filter(|entry| !entry.is_fee() && moves(entry.account_id, entry.asset_id));
        match (mains.next(), mains.next()) {
            (Some(main), None) => main.quantity = movement.quantity - fees,
            _ => return Err(ImportAmendRefusalDto::NotOneEntry),
        }
    }
    transaction
        .try_into_dto()
        .map_err(|_| ImportAmendRefusalDto::Unreadable)
}

fn proposed(
    mut rows: Vec<ImportProposalRowDto>,
    unstageable: Vec<UnstageableGroupDto>,
) -> ImportProposalDto {
    rows.sort_by_key(|row| row.transaction.date);
    let mut text = String::new();
    for row in &rows {
        writeln!(
            text,
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            row.id, row.kind, row.verb, row.transaction, row.movements, row.recorded_on, row.parts
        )
        .expect("writing to a String cannot fail");
    }
    ImportProposalDto {
        fingerprint: Uuid::new_v5(&Uuid::NAMESPACE_OID, text.as_bytes()),
        rows,
        unstageable,
        balance_checks: None,
        read_as_owed: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};
    use time::Date;

    use dal::models::import_models::{ImportAccountMappingModel, ImportAssetMappingModel};

    use super::*;
    use crate::dtos::fee_entry_dto::FeeEntryDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::imports::import_balance_dto::{
        ImportAssertionDto, ImportAssertionKindDto, ImportBalanceChecksDto,
    };
    use crate::dtos::imports::import_staging_dto::ImportPartsDto;
    use crate::dtos::ledger_matching_dto::{
        LedgerActivityDto, LedgerMovementDto, SourceKindDto, TransactionSourceDto,
    };
    use crate::dtos::transaction_dto::RegularTransactionMetadataDto;
    use crate::entities::imports::balances::SessionBalances;
    use crate::entities::imports::reconciliation::parts_ask_key;

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const SESSION: Uuid = Uuid::from_u128(2);
    const FILE: Uuid = Uuid::from_u128(3);
    const GBP: i32 = 10;
    const USD: i32 = 11;
    const SHARES: i32 = 12;
    const LEDGER: u128 = 500;

    fn transaction(id: Option<Uuid>) -> TransactionDto {
        TransactionDto {
            transaction_id: id,
            date: datetime!(2026-02-01 0:00 UTC),
            visibility: TransactionVisibilityDto::Default,
            fee_entries: Vec::new(),
            transaction_type: TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                entry: EntryDto::new(1, Uuid::nil(), "-5".parse().unwrap()),
            }),
        }
    }

    fn staged(id: u128) -> StagedTransactionDto {
        StagedTransactionDto {
            id: Uuid::from_u128(id),
            file_ids: vec![Uuid::nil()],
            transaction: transaction(None),
            movements: Vec::new(),
        }
    }

    fn payment(description: &str, quantity: Decimal, fees: &[(i32, Decimal)]) -> TransactionDto {
        TransactionDto {
            transaction_id: None,
            date: datetime!(2026-03-03 0:00 UTC),
            visibility: TransactionVisibilityDto::Default,
            fee_entries: fees
                .iter()
                .map(|(asset_id, fee)| FeeEntryDto {
                    entry: EntryDto::new(*asset_id, ACCOUNT, *fee),
                    entry_type: FeeEntryTypesDto::Transaction,
                })
                .collect(),
            transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: Some(description.to_string()),
                entry: EntryDto::new(GBP, ACCOUNT, quantity),
                category_id: 42,
            }),
        }
    }

    fn ledger_payment(quantity: Decimal, fees: &[(i32, Decimal)]) -> TransactionDto {
        let mut ledger = payment("Tesco", quantity, fees);
        ledger.transaction_id = Some(Uuid::from_u128(LEDGER));
        ledger.date = datetime!(2026-03-01 0:00 UTC);
        ledger
    }

    fn statement_row(transaction: TransactionDto, net: Decimal) -> StagedTransactionDto {
        StagedTransactionDto {
            id: Uuid::from_u128(1),
            file_ids: vec![Uuid::nil()],
            transaction,
            movements: vec![StagedMovementDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                quantity: net,
                file_id: Uuid::nil(),
                date: date!(2026 - 03 - 03),
                description: Some("TESCO STORES".to_string()),
                itemised_fee: None,
            }],
        }
    }

    fn amend(statement: StagedTransactionDto, ledger: Vec<TransactionDto>) -> ImportProposalDto {
        crate::test_support::load_dynamic_enums();
        merge(
            stage(vec![statement]),
            vec![decision(1, ImportVerbDto::Amend, Some(LEDGER))],
            ledger,
            HashMap::new(),
            Uuid::nil(),
        )
    }

    fn regular(transaction: &TransactionDto) -> &RegularTransactionMetadataDto {
        let TransactionTypeDto::Regular(metadata) = &transaction.transaction_type else {
            panic!("expected a regular transaction");
        };
        metadata
    }

    fn fees(transaction: &TransactionDto) -> Vec<(i32, Decimal)> {
        let mut fees: Vec<_> = transaction
            .fee_entries
            .iter()
            .map(|fee| (fee.entry.asset_id, fee.entry.quantity))
            .collect();
        fees.sort();
        fees
    }

    fn stage(staged: Vec<StagedTransactionDto>) -> SessionStage {
        SessionStage {
            staged,
            unstageable: Vec::new(),
            joined: HashMap::new(),
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

    fn delete_directive(id: u128, target: u128) -> ImportStagedDecisionDto {
        ImportStagedDecisionDto {
            id: Uuid::from_u128(id),
            ask_key: Some(format!("reverse:{target}")),
            verb: ImportVerbDto::Delete,
            transaction_id: Some(Uuid::from_u128(target)),
        }
    }

    fn matched(id: u128, target: u128) -> HashMap<Uuid, ImportRowMatchDto> {
        HashMap::from([(
            Uuid::from_u128(id),
            ImportRowMatchDto {
                verb: ImportVerbDto::Skip,
                transaction_id: Some(Uuid::from_u128(target)),
                claims: Vec::new(),
                near_miss: Vec::new(),
                recorded_on: Vec::new(),
            },
        )])
    }

    #[test]
    fn the_users_decision_outranks_the_match() {
        let proposal = merge(
            stage(vec![staged(1)]),
            vec![decision(1, ImportVerbDto::Add, None)],
            Vec::new(),
            matched(1, 7),
            Uuid::nil(),
        );
        assert_eq!(proposal.rows[0].verb, ImportVerbDto::Add);
        assert_eq!(proposal.rows[0].transaction.transaction_id, None);
        assert!(proposal.rows[0].verb_locked);
    }

    #[test]
    fn a_delete_directive_shows_its_ledger_transaction_and_is_dropped_when_it_is_gone() {
        let target = Uuid::from_u128(77);
        let proposal = merge(
            stage(Vec::new()),
            vec![delete_directive(5, 77), delete_directive(6, 78)],
            vec![transaction(Some(target))],
            HashMap::new(),
            Uuid::nil(),
        );

        assert_eq!(proposal.rows.len(), 1);
        assert_eq!(proposal.rows[0].verb, ImportVerbDto::Delete);
        assert_eq!(proposal.rows[0].ask_key.as_deref(), Some("reverse:77"));
        assert_eq!(proposal.rows[0].transaction.transaction_id, Some(target));
    }

    #[test]
    fn a_delete_directive_is_never_applied_to_a_derived_row() {
        let target = Uuid::from_u128(77);
        let proposal = merge(
            stage(vec![staged(5)]),
            vec![delete_directive(5, 77)],
            vec![transaction(Some(target))],
            HashMap::new(),
            Uuid::nil(),
        );

        let rows: Vec<_> = proposal
            .rows
            .iter()
            .map(|row| {
                (
                    row.verb,
                    row.ask_key.as_deref(),
                    row.transaction.transaction_id,
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                (ImportVerbDto::Add, None, None),
                (ImportVerbDto::Delete, Some("reverse:77"), Some(target)),
            ]
        );
    }

    #[test]
    fn the_fingerprint_changes_with_anything_apply_writes() {
        let base = merge(
            stage(vec![staged(1)]),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            Uuid::nil(),
        );
        let again = merge(
            stage(vec![staged(1)]),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            Uuid::nil(),
        );
        let skipped = merge(
            stage(vec![staged(1)]),
            Vec::new(),
            Vec::new(),
            matched(1, 7),
            Uuid::nil(),
        );
        let mut later = staged(1);
        later.transaction.date = datetime!(2026-02-02 0:00 UTC);
        let moved = merge(
            stage(vec![later]),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            Uuid::nil(),
        );

        assert_eq!(base.fingerprint, again.fingerprint);
        assert_ne!(base.fingerprint, skipped.fingerprint);
        assert_ne!(base.fingerprint, moved.fingerprint);
    }

    #[test]
    fn an_amend_keeps_the_ledger_fees_where_the_statement_itemises_none() {
        let proposal = amend(
            statement_row(payment("TESCO STORES", dec!(-35.5), &[]), dec!(-35.5)),
            vec![ledger_payment(
                dec!(-30),
                &[(GBP, dec!(-0.5)), (USD, dec!(-1))],
            )],
        );

        let transaction = &proposal.rows[0].transaction;
        assert_eq!(regular(transaction).entry.quantity, dec!(-35));
        assert_eq!(fees(transaction), vec![(GBP, dec!(-0.5)), (USD, dec!(-1))]);
    }

    #[test]
    fn an_amend_takes_the_statements_fees_on_its_own_account_and_asset_only() {
        let proposal = amend(
            statement_row(
                payment("TESCO STORES", dec!(-35), &[(GBP, dec!(-1))]),
                dec!(-36),
            ),
            vec![ledger_payment(
                dec!(-30),
                &[(GBP, dec!(-0.5)), (USD, dec!(-1))],
            )],
        );

        let transaction = &proposal.rows[0].transaction;
        assert_eq!(regular(transaction).entry.quantity, dec!(-35));
        assert_eq!(fees(transaction), vec![(GBP, dec!(-1)), (USD, dec!(-1))]);
    }

    #[test]
    fn an_amend_the_ledger_cannot_take_keeps_the_statement_row_and_says_why() {
        let statement = || statement_row(payment("TESCO STORES", dec!(-35), &[]), dec!(-35));
        let gone = amend(statement(), Vec::new());
        let mut elsewhere = ledger_payment(dec!(-30), &[]);
        let TransactionTypeDto::Regular(metadata) = &mut elsewhere.transaction_type else {
            panic!("expected a regular transaction");
        };
        metadata.entry.account_id = Uuid::from_u128(2);
        let on_another_account = amend(statement(), vec![elsewhere]);

        for (proposal, why) in [
            (gone, ImportAmendRefusalDto::TargetGone),
            (on_another_account, ImportAmendRefusalDto::NotOneEntry),
        ] {
            let row = &proposal.rows[0];
            assert_eq!(row.verb, ImportVerbDto::Amend);
            assert_eq!(row.amend_refusal, Some(why));
            assert_eq!(
                row.transaction.transaction_id,
                Some(Uuid::from_u128(LEDGER))
            );
            assert_eq!(
                regular(&row.transaction).description.as_deref(),
                Some("TESCO STORES")
            );
        }
    }

    fn stated(asset_id: i32, on: Date, quantity: Decimal) -> ImportStartingBalanceDto {
        ImportStartingBalanceDto {
            account_id: ACCOUNT,
            asset_id,
            file_id: FILE,
            date: on,
            quantity,
        }
    }

    fn started(
        starting_balances: Vec<ImportStartingBalanceDto>,
        decisions: &[ImportStagedDecisionDto],
    ) -> ImportProposalDto {
        crate::test_support::load_dynamic_enums();
        add_starting_balances(
            merge(
                stage(Vec::new()),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                Uuid::nil(),
            ),
            SESSION,
            starting_balances,
            decisions,
            &HashSet::from([GBP, USD]),
        )
    }

    fn transfer(transaction: &TransactionDto) -> (&'static str, i32, Decimal) {
        let (name, entry) = match &transaction.transaction_type {
            TransactionTypeDto::CashTransferIn(metadata) => ("cash transfer in", &metadata.entry),
            TransactionTypeDto::CashTransferOut(metadata) => ("cash transfer out", &metadata.entry),
            TransactionTypeDto::AssetTransferIn(metadata) => ("asset transfer in", &metadata.entry),
            TransactionTypeDto::AssetTransferOut(metadata) => {
                ("asset transfer out", &metadata.entry)
            }
            _ => panic!("expected a transfer"),
        };
        assert_eq!(entry.account_id, ACCOUNT);
        (name, entry.asset_id, entry.quantity)
    }

    fn moved(id: u128, on: Date, quantity: Decimal) -> StagedTransactionDto {
        let mut transaction = payment("TESCO STORES", quantity, &[]);
        transaction.date = on.midnight().assume_utc();
        StagedTransactionDto {
            id: Uuid::from_u128(id),
            file_ids: vec![FILE],
            transaction,
            movements: vec![StagedMovementDto {
                account_id: ACCOUNT,
                asset_id: GBP,
                quantity,
                file_id: FILE,
                date: on,
                description: Some("TESCO STORES".to_string()),
                itemised_fee: None,
            }],
        }
    }

    fn held(id: u128, on: Date, quantity: Decimal) -> LedgerMovementDto {
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

    fn recorded(id: u128, on: Date, kind: SourceKindDto) -> TransactionSourceDto {
        TransactionSourceDto {
            transaction_id: Uuid::from_u128(id),
            account_id: ACCOUNT,
            asset_id: GBP,
            date: on,
            description: None,
            kind,
            stream: "statement:earlier".to_string(),
            external_ref: Some(id.to_string()),
            itemised_fee: None,
        }
    }

    fn confirmed(row: u128, target: u128, on: Date) -> (Uuid, ImportRowMatchDto) {
        let target = Uuid::from_u128(target);
        (
            Uuid::from_u128(row),
            ImportRowMatchDto {
                verb: ImportVerbDto::Skip,
                transaction_id: Some(target),
                claims: vec![ImportClaimDto {
                    transaction_id: target,
                    account_id: ACCOUNT,
                    asset_id: GBP,
                    booked_on: on,
                }],
                near_miss: Vec::new(),
                recorded_on: vec![Some(ImportRecordedOnDto::Transaction(target))],
            },
        )
    }

    fn balance(
        kind: ImportAssertionKindDto,
        asset: &str,
        on: Date,
        quantity: Decimal,
    ) -> ImportAssertionDto {
        ImportAssertionDto {
            file_id: FILE,
            account_ref: "Current".to_string(),
            asset_ref: Some(asset.to_string()),
            kind,
            date: on,
            quantity,
        }
    }

    fn read(
        statement: Vec<StagedTransactionDto>,
        matches: HashMap<Uuid, ImportRowMatchDto>,
        assertions: Vec<ImportAssertionDto>,
        ledger: LedgerActivityDto,
    ) -> (ImportProposalDto, ImportBalanceChecksDto) {
        crate::test_support::load_dynamic_enums();
        let proposal = merge(
            stage(statement),
            Vec::new(),
            Vec::new(),
            matches,
            Uuid::nil(),
        );
        let mut balances = SessionBalances::new(assertions);
        balances.add_account_mappings(&[ImportAccountMappingModel {
            file_id: FILE,
            account_ref: "Current".to_string(),
            account_id: Some(ACCOUNT),
        }]);
        balances.add_asset_mappings(&[("GBP", GBP), ("ACME", SHARES)].map(
            |(asset_ref, asset_id)| ImportAssetMappingModel {
                asset_ref: asset_ref.to_string(),
                asset_id: Some(asset_id),
            },
        ));
        balances.add_rows(&proposal.rows);
        let proposal = add_starting_balances(
            proposal,
            SESSION,
            balances.starting_balances(&[], &ledger),
            &[],
            &HashSet::from([GBP]),
        );
        balances.add_rows(
            proposal
                .rows
                .iter()
                .filter(|row| matches!(row.kind, ImportRowKindDto::StartingBalance(_))),
        );
        let checks = balances.check(&[], &ledger);
        (proposal, checks)
    }

    fn assert_balanced(checks: &ImportBalanceChecksDto) {
        assert!(!checks.checks.is_empty());
        for check in &checks.checks {
            assert_eq!(check.starting_off_by, dec!(0));
            assert_eq!(check.final_off_by, dec!(0));
            assert!(check.changes.is_empty());
        }
    }

    struct Month {
        opening_on: Date,
        opening: Decimal,
        rows: Vec<(Date, Decimal)>,
        closing_on: Date,
    }

    impl Month {
        fn closing(&self) -> Decimal {
            self.opening
                + self
                    .rows
                    .iter()
                    .map(|(_, quantity)| *quantity)
                    .sum::<Decimal>()
        }
    }

    fn months() -> [Month; 3] {
        [
            Month {
                opening_on: date!(2026 - 01 - 01),
                opening: dec!(500),
                rows: vec![
                    (date!(2026 - 01 - 10), dec!(100)),
                    (date!(2026 - 01 - 31), dec!(-20)),
                ],
                closing_on: date!(2026 - 01 - 31),
            },
            Month {
                opening_on: date!(2026 - 02 - 01),
                opening: dec!(580),
                rows: vec![
                    (date!(2026 - 02 - 01), dec!(300)),
                    (date!(2026 - 02 - 28), dec!(-80)),
                ],
                closing_on: date!(2026 - 02 - 28),
            },
            Month {
                opening_on: date!(2026 - 03 - 01),
                opening: dec!(800),
                rows: vec![
                    (date!(2026 - 03 - 01), dec!(-50)),
                    (date!(2026 - 03 - 15), dec!(250)),
                ],
                closing_on: date!(2026 - 03 - 31),
            },
        ]
    }

    fn bank(on: Date) -> Decimal {
        let months = months();
        months[0].opening
            + months
                .iter()
                .flat_map(|month| &month.rows)
                .filter(|(date, _)| *date <= on)
                .map(|(_, quantity)| *quantity)
                .sum::<Decimal>()
    }

    #[derive(Default)]
    struct Imported {
        ledger: LedgerActivityDto,
        starting_balances: Vec<TransactionDto>,
        written: u128,
    }

    impl Imported {
        fn held_through(&self, on: Date) -> Decimal {
            self.ledger
                .movements
                .iter()
                .filter(|movement| movement.date <= on)
                .map(|movement| movement.quantity)
                .sum()
        }

        fn import(
            &mut self,
            month: &Month,
            with_opening: bool,
        ) -> (ImportProposalDto, ImportBalanceChecksDto) {
            let mut assertions = vec![balance(
                ImportAssertionKindDto::Closing,
                "GBP",
                month.closing_on,
                month.closing(),
            )];
            if with_opening {
                assertions.push(balance(
                    ImportAssertionKindDto::Opening,
                    "GBP",
                    month.opening_on,
                    month.opening,
                ));
            }
            let mut matches = HashMap::new();
            let mut statement = Vec::new();
            for (index, (on, quantity)) in month.rows.iter().enumerate() {
                let row = index as u128 + 1;
                statement.push(moved(row, *on, *quantity));
                let same = self.ledger.movements.iter().find(|held| {
                    held.date == *on
                        && held.quantity == *quantity
                        && !self
                            .starting_balances
                            .iter()
                            .any(|start| start.transaction_id == Some(held.transaction_id))
                });
                if let Some(same) = same {
                    matches.extend([confirmed(row, same.transaction_id.as_u128(), *on)]);
                }
            }
            let (proposal, checks) = loop {
                let (proposal, checks) = read(
                    statement.clone(),
                    matches.clone(),
                    assertions.clone(),
                    self.ledger.clone(),
                );
                let named: Vec<Uuid> = checks
                    .checks
                    .iter()
                    .flat_map(|check| &check.unlisted)
                    .filter(|row| row.explains_difference)
                    .map(|row| row.transaction_id)
                    .collect();
                if named.is_empty() {
                    break (proposal, checks);
                }
                self.ledger
                    .movements
                    .retain(|movement| !named.contains(&movement.transaction_id));
                self.ledger
                    .sources
                    .retain(|source| !named.contains(&source.transaction_id));
                self.starting_balances
                    .retain(|start| !start.transaction_id.is_some_and(|id| named.contains(&id)));
            };

            for row in &proposal.rows {
                match (row.kind, row.verb) {
                    (ImportRowKindDto::Movement, ImportVerbDto::Add) => {
                        for movement in &row.movements {
                            self.written += 1;
                            self.ledger.movements.push(held(
                                self.written,
                                movement.date,
                                movement.quantity,
                            ));
                            self.ledger.sources.push(recorded(
                                self.written,
                                movement.date,
                                SourceKindDto::Statement,
                            ));
                        }
                    }
                    (ImportRowKindDto::Movement, ImportVerbDto::Skip) => {
                        for claim in &row.claims {
                            self.ledger.sources.push(recorded(
                                claim.transaction_id.as_u128(),
                                claim.booked_on,
                                SourceKindDto::Statement,
                            ));
                        }
                    }
                    (ImportRowKindDto::StartingBalance(starting), ImportVerbDto::Add) => {
                        self.written += 1;
                        self.ledger.movements.push(held(
                            self.written,
                            starting.date,
                            starting.quantity,
                        ));
                        self.ledger.sources.push(recorded(
                            self.written,
                            starting.date,
                            SourceKindDto::Balance,
                        ));
                        let mut transaction = row.transaction.clone();
                        transaction.transaction_id = Some(Uuid::from_u128(self.written));
                        self.starting_balances.push(transaction);
                    }
                    other => panic!("apply writes no such row: {other:?}"),
                }
            }
            (proposal, checks)
        }

        fn assert_equal_to_the_bank(&self, month: &Month, with_opening: bool) {
            assert_eq!(self.held_through(month.closing_on), month.closing());
            let mut day = if with_opening {
                month.opening_on - time::Duration::days(1)
            } else {
                month.rows[0].0
            };
            while day <= month.closing_on {
                assert_eq!(self.held_through(day), bank(day), "on {day}");
                day += time::Duration::days(1);
            }
        }
    }

    #[test]
    fn money_held_starts_as_a_cash_transfer_in_and_money_owed_as_a_cash_transfer_out() {
        let proposal = started(
            vec![
                stated(GBP, date!(2026 - 02 - 28), dec!(1000)),
                stated(USD, date!(2026 - 02 - 28), dec!(-150000)),
            ],
            &[],
        );

        let transfers: Vec<_> = proposal
            .rows
            .iter()
            .map(|row| transfer(&row.transaction))
            .collect();
        assert_eq!(
            transfers,
            vec![
                ("cash transfer in", GBP, dec!(1000)),
                ("cash transfer out", USD, dec!(-150000)),
            ]
        );
        for row in &proposal.rows {
            create_transaction_from_dto(row.transaction.clone(), Uuid::nil()).unwrap();
        }
    }

    #[test]
    fn a_holding_starts_as_an_asset_transfer_in_and_one_below_zero_is_not_derived() {
        let proposal = started(
            vec![
                stated(SHARES, date!(2026 - 02 - 28), dec!(12.5)),
                stated(SHARES + 1, date!(2026 - 02 - 28), dec!(-3)),
            ],
            &[],
        );

        let [row] = proposal.rows.as_slice() else {
            panic!("expected one row, got {:?}", proposal.rows);
        };
        assert_eq!(
            transfer(&row.transaction),
            ("asset transfer in", SHARES, dec!(12.5))
        );
        create_transaction_from_dto(row.transaction.clone(), Uuid::nil()).unwrap();
    }

    #[test]
    fn a_starting_balance_reads_the_same_every_time_and_keeps_its_pin() {
        let starting = || {
            vec![
                stated(GBP, date!(2026 - 02 - 28), dec!(1000)),
                stated(USD, date!(2026 - 02 - 28), dec!(20)),
            ]
        };
        let first = started(starting(), &[]);
        let again = started(starting(), &[]);
        let pin = ImportStagedDecisionDto {
            id: first.rows[0].id,
            ask_key: None,
            verb: ImportVerbDto::Skip,
            transaction_id: None,
        };
        let pinned = started(starting(), &[pin]);

        assert_ne!(first.rows[0].id, first.rows[1].id);
        assert_eq!(first.rows[0].id, again.rows[0].id);
        assert_eq!(first.fingerprint, again.fingerprint);
        assert_eq!(pinned.rows[0].id, first.rows[0].id);
        assert_eq!(pinned.rows[0].verb, ImportVerbDto::Skip);
        assert!(pinned.rows[0].verb_locked);
        assert_eq!(pinned.rows[1].verb, ImportVerbDto::Add);
        assert_ne!(pinned.fingerprint, first.fingerprint);
    }

    #[test]
    fn statements_imported_in_any_order_end_equal_to_the_bank_on_every_day() {
        for with_opening in [true, false] {
            for order in [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ] {
                let months = months();
                let mut imported = Imported::default();
                for month in order {
                    imported.import(&months[month], with_opening);
                }
                for month in &months {
                    imported.assert_equal_to_the_bank(month, with_opening);
                }
                assert_eq!(imported.starting_balances.len(), 1, "{order:?}");
            }
        }
    }

    #[test]
    fn a_later_start_no_statement_covers_is_not_seen_until_one_does() {
        for with_opening in [true, false] {
            let [january, february, march] = months();
            let mut imported = Imported::default();
            imported.import(&march, with_opening);
            let (_, checks) = imported.import(&january, with_opening);

            assert_balanced(&checks);
            assert!(checks.checks.iter().all(|check| check.unlisted.is_empty()));
            assert_eq!(imported.starting_balances.len(), 2);
            imported.assert_equal_to_the_bank(&january, with_opening);
            assert_eq!(
                imported.held_through(march.closing_on),
                march.closing() + january.closing()
            );

            imported.import(&february, with_opening);

            assert_eq!(imported.starting_balances.len(), 1);
            for month in [&january, &february, &march] {
                imported.assert_equal_to_the_bank(month, with_opening);
            }
        }
    }

    #[test]
    fn a_later_statement_after_a_gap_offers_the_missing_month_as_an_undecided_start() {
        for with_opening in [true, false] {
            let [january, _, march] = months();
            let mut imported = Imported::default();
            imported.import(&january, with_opening);
            let (proposal, _) = imported.import(&march, with_opening);

            let offered: Vec<_> = proposal
                .rows
                .iter()
                .filter_map(|row| match row.kind {
                    ImportRowKindDto::StartingBalance(starting) => {
                        Some((starting.date, starting.quantity, row.verb_locked))
                    }
                    ImportRowKindDto::Movement
                    | ImportRowKindDto::JoinedTransfer(_)
                    | ImportRowKindDto::Adjustment(_) => None,
                })
                .collect();
            assert_eq!(offered, vec![(date!(2026 - 02 - 28), dec!(220), false)]);
            imported.assert_equal_to_the_bank(&january, with_opening);
            imported.assert_equal_to_the_bank(&march, with_opening);
        }
    }

    #[test]
    fn a_starting_balance_that_changes_is_asked_about_again() {
        let decided = started(vec![stated(GBP, date!(2026 - 01 - 31), dec!(580))], &[]);
        let pin = ImportStagedDecisionDto {
            id: decided.rows[0].id,
            verb: ImportVerbDto::Add,
            transaction_id: None,
            ask_key: None,
        };

        let same = started(
            vec![stated(GBP, date!(2026 - 01 - 31), dec!(580))],
            std::slice::from_ref(&pin),
        );
        let other_amount = started(
            vec![stated(GBP, date!(2026 - 01 - 31), dec!(500))],
            std::slice::from_ref(&pin),
        );
        let other_date = started(
            vec![stated(GBP, date!(2025 - 12 - 31), dec!(580))],
            std::slice::from_ref(&pin),
        );

        assert!(!decided.rows[0].verb_locked);
        assert!(same.rows[0].verb_locked);
        assert!(!other_amount.rows[0].verb_locked);
        assert!(!other_date.rows[0].verb_locked);
    }

    #[test]
    fn importing_the_same_statement_again_changes_nothing() {
        for with_opening in [true, false] {
            let [january, february, march] = months();
            let mut imported = Imported::default();
            imported.import(&march, with_opening);
            imported.import(&february, with_opening);
            for month in [&february, &march, &february] {
                let (proposal, checks) = imported.import(month, with_opening);
                assert!(proposal
                    .rows
                    .iter()
                    .all(|row| row.kind == ImportRowKindDto::Movement
                        && row.verb == ImportVerbDto::Skip));
                assert_balanced(&checks);
            }
            imported.import(&january, with_opening);
            for month in [&january, &february, &march] {
                imported.assert_equal_to_the_bank(month, with_opening);
            }
        }
    }

    #[test]
    fn history_that_reached_the_ledger_another_way_is_confirmed_by_its_statement() {
        for bank_fed in [true, false] {
            let [_, february, march] = months();
            let mut imported = Imported::default();
            imported.import(&march, true);
            let mut backfilled = held(77, date!(2026 - 02 - 28), dec!(-80));
            if bank_fed {
                backfilled.connector_binding_ids = vec![Uuid::from_u128(70)];
            }
            imported.ledger.movements.push(backfilled);

            let (proposal, checks) = imported.import(&february, true);

            let verbs: Vec<_> = proposal
                .rows
                .iter()
                .map(|row| (row.verb, row.transaction.date.date()))
                .collect();
            assert_eq!(
                verbs,
                vec![
                    (ImportVerbDto::Add, date!(2026 - 01 - 31)),
                    (ImportVerbDto::Add, date!(2026 - 02 - 01)),
                    (ImportVerbDto::Skip, date!(2026 - 02 - 28)),
                ]
            );
            assert_balanced(&checks);
            imported.assert_equal_to_the_bank(&february, true);
            imported.assert_equal_to_the_bank(&march, true);

            let (again, _) = imported.import(&february, true);
            assert!(again.rows.iter().all(|row| row.verb == ImportVerbDto::Skip));
        }
    }

    #[test]
    fn holdings_stated_with_no_transactions_start_each_holding_and_the_cash() {
        let (proposal, checks) = read(
            Vec::new(),
            HashMap::new(),
            vec![
                balance(
                    ImportAssertionKindDto::Snapshot,
                    "ACME",
                    date!(2026 - 03 - 31),
                    dec!(10),
                ),
                balance(
                    ImportAssertionKindDto::Snapshot,
                    "GBP",
                    date!(2026 - 03 - 31),
                    dec!(500),
                ),
            ],
            LedgerActivityDto::default(),
        );

        let mut transfers: Vec<_> = proposal
            .rows
            .iter()
            .map(|row| {
                assert_eq!(row.verb, ImportVerbDto::Add);
                assert_eq!(row.transaction.date, datetime!(2026-03-31 0:00 UTC));
                transfer(&row.transaction)
            })
            .collect();
        transfers.sort();
        assert_eq!(
            transfers,
            vec![
                ("asset transfer in", SHARES, dec!(10)),
                ("cash transfer in", GBP, dec!(500)),
            ]
        );
        assert_balanced(&checks);
    }

    #[test]
    fn rows_are_ordered_by_date_keeping_the_staging_order_within_a_date() {
        let mut target = transaction(Some(Uuid::from_u128(77)));
        target.date = datetime!(2026-01-15 0:00 UTC);
        let proposal = merge(
            stage(vec![staged(3), staged(1), staged(2)]),
            vec![delete_directive(5, 77)],
            vec![target],
            HashMap::new(),
            Uuid::nil(),
        );

        let ids: Vec<_> = proposal.rows.iter().map(|row| row.id).collect();
        assert_eq!(ids, [5, 3, 1, 2].map(Uuid::from_u128));
    }

    #[test]
    fn a_row_answered_as_parts_is_skipped_against_them_and_names_them_for_grouping() {
        let part = |transaction: u128| ImportStagedDecisionDto {
            id: Uuid::now_v7(),
            ask_key: Some(parts_ask_key(Uuid::from_u128(1))),
            verb: ImportVerbDto::Skip,
            transaction_id: Some(Uuid::from_u128(transaction)),
        };
        let tesco = || stage(vec![moved(1, date!(2026 - 03 - 03), dec!(-50))]);
        let parts = [7, 8].map(Uuid::from_u128).to_vec();

        let proposal = merge(
            tesco(),
            vec![part(8), part(7)],
            Vec::new(),
            HashMap::new(),
            Uuid::nil(),
        );
        let row = &proposal.rows[0];
        assert_eq!((row.verb, row.verb_locked), (ImportVerbDto::Skip, true));
        assert_eq!(
            row.parts,
            Some(ImportPartsDto {
                transaction_ids: parts.clone(),
                description: "TESCO STORES".to_string(),
                category_id: 42,
            })
        );
        let claimed: Vec<Uuid> = row
            .claims
            .iter()
            .map(|claim| claim.transaction_id)
            .collect();
        assert_eq!(claimed, parts);
        assert_eq!(
            row.recorded_on,
            vec![Some(ImportRecordedOnDto::Transaction(parts[0]))]
        );

        let pinned = merge(
            tesco(),
            vec![part(7), decision(1, ImportVerbDto::Add, None)],
            Vec::new(),
            HashMap::new(),
            Uuid::nil(),
        );
        assert_eq!(pinned.rows[0].verb, ImportVerbDto::Add);
        assert_eq!(pinned.rows[0].parts, None);
    }

    #[test]
    fn an_adjustment_is_offered_for_money_only_and_left_out_until_added() {
        crate::test_support::load_dynamic_enums();
        let offered = |asset_id: i32, decisions: &[ImportStagedDecisionDto]| {
            add_adjustments(
                merge(
                    stage(Vec::new()),
                    Vec::new(),
                    Vec::new(),
                    HashMap::new(),
                    Uuid::nil(),
                ),
                SESSION,
                vec![ImportAdjustmentDto {
                    account_id: ACCOUNT,
                    asset_id,
                    date: date!(2026 - 03 - 31),
                    quantity: dec!(-100),
                }],
                decisions,
                &HashSet::from([GBP]),
            )
        };
        let left_out = offered(GBP, &[]);
        let row = &left_out.rows[0];
        assert_eq!((row.verb, row.verb_locked), (ImportVerbDto::Skip, false));
        assert_eq!(
            transfer(&row.transaction),
            ("cash transfer out", GBP, dec!(-100))
        );

        let add = ImportStagedDecisionDto {
            id: row.id,
            ask_key: None,
            verb: ImportVerbDto::Add,
            transaction_id: None,
        };
        let added = offered(GBP, &[add]);
        assert_eq!(
            (added.rows[0].verb, added.rows[0].verb_locked),
            (ImportVerbDto::Add, true)
        );
        assert_ne!(added.fingerprint, left_out.fingerprint);

        assert!(offered(USD, &[]).rows.is_empty());
    }
}
