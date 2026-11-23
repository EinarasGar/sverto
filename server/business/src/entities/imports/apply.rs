use std::collections::{HashMap, HashSet};

use anyhow::Result;
use uuid::Uuid;

use super::reconciliation::{external_refs, statement_stream};
use crate::dtos::imports::import_source_dto::ImportExternalLineDto;
use crate::dtos::imports::import_staging_dto::{
    ImportAmendRefusalDto, ImportPartsDto, ImportProposalDto, ImportProposalRowDto,
    ImportRecordedOnDto, ImportRowKindDto, ImportVerbDto,
};
use crate::dtos::ledger_matching_dto::{SourceKindDto, TransactionSourceDto};
use crate::dtos::transaction_dto::TransactionDto;
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};
use crate::entities::transactions::transaction::Transaction;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

pub struct ImportApply {
    rows: Vec<ImportProposalRowDto>,
    refs: Vec<Vec<String>>,
}

impl ImportApply {
    pub fn new(proposal: ImportProposalDto) -> Self {
        let mut refs =
            external_refs(proposal.rows.iter().flat_map(|row| &row.movements)).into_iter();
        let refs = proposal
            .rows
            .iter()
            .map(|row| refs.by_ref().take(row.movements.len()).collect())
            .collect();
        Self {
            rows: proposal.rows,
            refs,
        }
    }

    pub fn contradiction(&self) -> Option<&'static str> {
        let deleted: HashSet<Uuid> = self.deleted().into_iter().collect();
        let mut amended: HashSet<Uuid> = HashSet::new();
        for row in self
            .rows
            .iter()
            .filter(|row| row.verb != ImportVerbDto::Delete)
        {
            if row.verb == ImportVerbDto::Amend {
                if let Some(target) = row.transaction.transaction_id {
                    if !amended.insert(target) {
                        return Some("Two rows correct the same ledger transaction.");
                    }
                }
            }
            let recorded_on = row.recorded_on.iter().flatten().filter_map(|on| match on {
                ImportRecordedOnDto::Transaction(id) => Some(*id),
                ImportRecordedOnDto::Row(_) => None,
            });
            if row
                .transaction
                .transaction_id
                .into_iter()
                .chain(recorded_on)
                .chain(
                    row.parts
                        .iter()
                        .flat_map(|parts| parts.transaction_ids.iter().copied()),
                )
                .any(|target| deleted.contains(&target))
            {
                return Some("A row is recorded on a ledger transaction the import deletes.");
            }
        }
        None
    }

    pub fn groups(&self) -> Vec<&ImportPartsDto> {
        self.rows
            .iter()
            .filter(|row| row.verb == ImportVerbDto::Skip)
            .filter_map(|row| row.parts.as_ref())
            .collect()
    }

    pub fn deleted(&self) -> Vec<Uuid> {
        self.rows
            .iter()
            .filter(|row| row.verb == ImportVerbDto::Delete)
            .filter_map(|row| row.transaction.transaction_id)
            .collect()
    }

    pub fn inserts(&self, user_id: Uuid) -> Result<(Vec<Uuid>, Vec<Transaction>)> {
        let mut row_ids = Vec::new();
        let mut transactions = Vec::new();
        for row in self
            .rows
            .iter()
            .filter(|row| row.verb == ImportVerbDto::Add)
        {
            let transaction = create_transaction_from_dto(row.transaction.clone(), user_id)
                .map_err(|error| row_error(row, error))?;
            row_ids.push(row.id);
            transactions.push(transaction);
        }
        Ok((row_ids, transactions))
    }

    pub fn amended(&self) -> Result<Vec<(Uuid, TransactionDto)>> {
        let mut amended = Vec::new();
        for row in self
            .rows
            .iter()
            .filter(|row| row.verb == ImportVerbDto::Amend)
        {
            if let Some(refusal) = row.amend_refusal {
                let why = match refusal {
                    ImportAmendRefusalDto::TargetGone => "the transaction it matched is gone",
                    ImportAmendRefusalDto::NotOneEntry => {
                        "it doesn't have exactly one entry on that account and asset"
                    }
                    ImportAmendRefusalDto::Unreadable => "it can't be read as a transaction",
                };
                return Err(BusinessValidationErrorDto {
                    errors: vec![BusinessFieldErrorDto {
                        field: "items".to_string(),
                        message: format!(
                            "The row dated {} can't correct its ledger transaction: {why}.",
                            row.transaction.date.date()
                        ),
                    }],
                }
                .into());
            }
            if let Some(target_id) = row.transaction.transaction_id {
                amended.push((target_id, row.transaction.clone()));
            }
        }
        Ok(amended)
    }

    pub fn sources(&self, created: &HashMap<Uuid, Uuid>) -> Vec<TransactionSourceDto> {
        let mut sources = Vec::new();
        for (row, refs) in self.rows.iter().zip(&self.refs) {
            if let (ImportRowKindDto::StartingBalance(starting), Some(transaction_id)) =
                (row.kind, created.get(&row.id))
            {
                sources.push(TransactionSourceDto {
                    transaction_id: *transaction_id,
                    account_id: starting.account_id,
                    asset_id: starting.asset_id,
                    date: starting.date,
                    description: None,
                    kind: SourceKindDto::Balance,
                    stream: statement_stream(starting.source_id),
                    external_ref: Some("starting-balance".to_string()),
                    itemised_fee: None,
                });
            }
            for (index, (movement, external_ref)) in row.movements.iter().zip(refs).enumerate() {
                let Some(transaction_id) = movement_transaction(row, index, created) else {
                    continue;
                };
                sources.push(TransactionSourceDto {
                    transaction_id,
                    account_id: movement.account_id,
                    asset_id: movement.asset_id,
                    date: movement.date,
                    description: movement.description.clone(),
                    kind: SourceKindDto::Statement,
                    stream: statement_stream(movement.source_id),
                    external_ref: Some(external_ref.clone()),
                    itemised_fee: movement.itemised_fee,
                });
            }
        }
        sources
    }

    pub fn external_lines(&self, created: &HashMap<Uuid, Uuid>) -> Vec<ImportExternalLineDto> {
        self.rows
            .iter()
            .flat_map(|row| {
                row.movements
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, movement)| {
                        Some(ImportExternalLineDto {
                            source_id: movement.source_id,
                            external_id: movement.external_id.clone()?,
                            transaction_id: movement_transaction(row, index, created),
                        })
                    })
            })
            .collect()
    }
}

fn movement_transaction(
    row: &ImportProposalRowDto,
    movement_index: usize,
    created: &HashMap<Uuid, Uuid>,
) -> Option<Uuid> {
    match row.verb {
        ImportVerbDto::Add => created.get(&row.id).copied(),
        ImportVerbDto::Amend => row.transaction.transaction_id,
        ImportVerbDto::Skip => match row.recorded_on.get(movement_index).copied().flatten() {
            Some(ImportRecordedOnDto::Transaction(id)) => Some(id),
            Some(ImportRecordedOnDto::Row(id)) => created.get(&id).copied(),
            None => None,
        },
        ImportVerbDto::Delete => None,
    }
}

fn row_error(row: &ImportProposalRowDto, error: anyhow::Error) -> BusinessValidationErrorDto {
    BusinessValidationErrorDto {
        errors: vec![BusinessFieldErrorDto {
            field: "items".to_string(),
            message: format!(
                "The row dated {} can't be written: {error}",
                row.transaction.date.date()
            ),
        }],
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};

    use super::*;
    use crate::dtos::entry_dto::EntryDto;
    use crate::dtos::fee_entry_dto::FeeEntryDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::imports::import_balance_dto::ImportStartingBalanceDto;
    use crate::dtos::imports::import_staging_dto::StagedMovementDto;
    use crate::dtos::transaction_dto::{
        RegularTransactionMetadataDto, TransactionTypeDto, TransactionVisibilityDto,
    };

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const SOURCE: Uuid = Uuid::from_u128(10);
    const GBP: i32 = 10;
    const CATEGORY: i32 = 42;

    fn regular(id: Option<Uuid>, quantity: Decimal, fees: &[Decimal]) -> TransactionDto {
        TransactionDto {
            transaction_id: id,
            date: datetime!(2026-03-02 0:00 UTC),
            visibility: TransactionVisibilityDto::Default,
            fee_entries: fees
                .iter()
                .map(|fee| FeeEntryDto {
                    entry: EntryDto::new(GBP, ACCOUNT, *fee),
                    entry_type: FeeEntryTypesDto::Transaction,
                })
                .collect(),
            transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: Some("TESCO".to_string()),
                entry: EntryDto::new(GBP, ACCOUNT, quantity),
                category_id: CATEGORY,
            }),
        }
    }

    fn movement(quantity: Decimal) -> StagedMovementDto {
        StagedMovementDto {
            account_id: ACCOUNT,
            asset_id: GBP,
            quantity,
            source_id: SOURCE,
            date: date!(2026 - 03 - 03),
            description: Some("TESCO STORES".to_string()),
            itemised_fee: None,
            external_id: None,
        }
    }

    fn row(
        id: u128,
        verb: ImportVerbDto,
        transaction: TransactionDto,
        quantity: Decimal,
        recorded_on: Option<ImportRecordedOnDto>,
    ) -> ImportProposalRowDto {
        ImportProposalRowDto {
            id: Uuid::from_u128(id),
            kind: ImportRowKindDto::Movement,
            source_ids: vec![SOURCE],
            ask_key: None,
            verb,
            verb_locked: false,
            transaction,
            near_miss: Vec::new(),
            movements: vec![movement(quantity)],
            claims: Vec::new(),
            recorded_on: vec![recorded_on],
            amend_refusal: None,
            parts: None,
        }
    }

    fn proposal(rows: Vec<ImportProposalRowDto>) -> ImportProposalDto {
        ImportProposalDto {
            rows,
            unstageable: Vec::new(),
            fingerprint: Uuid::nil(),
            balance_checks: None,
            read_as_owed: Vec::new(),
        }
    }

    #[test]
    fn every_movement_is_recorded_on_the_transaction_it_describes() {
        let ledger = Uuid::from_u128(500);
        let new = Uuid::from_u128(900);
        let apply = ImportApply::new(proposal(vec![
            row(
                1,
                ImportVerbDto::Add,
                regular(None, dec!(-35), &[]),
                dec!(-35),
                None,
            ),
            row(
                2,
                ImportVerbDto::Skip,
                regular(Some(ledger), dec!(-4), &[]),
                dec!(-4),
                Some(ImportRecordedOnDto::Transaction(ledger)),
            ),
            row(
                3,
                ImportVerbDto::Skip,
                regular(None, dec!(-35), &[]),
                dec!(-35),
                Some(ImportRecordedOnDto::Row(Uuid::from_u128(1))),
            ),
            row(
                4,
                ImportVerbDto::Skip,
                regular(None, dec!(-9), &[]),
                dec!(-9),
                None,
            ),
        ]));

        let sources = apply.sources(&HashMap::from([(Uuid::from_u128(1), new)]));

        let recorded: Vec<_> = sources
            .iter()
            .map(|source| (source.transaction_id, source.external_ref.clone()))
            .collect();
        assert_eq!(
            recorded,
            vec![
                (new, Some("2026-03-03|-35|1".to_string())),
                (ledger, Some("2026-03-03|-4|1".to_string())),
                (new, Some("2026-03-03|-35|2".to_string())),
            ]
        );
        assert!(sources
            .iter()
            .all(|source| source.stream == format!("statement:{SOURCE}")
                && source.kind == SourceKindDto::Statement));
    }

    #[test]
    fn every_bank_line_reports_the_transaction_it_ended_up_on_or_none() {
        let ledger = Uuid::from_u128(500);
        let new = Uuid::from_u128(900);
        let bank_row = |id, verb, transaction, recorded_on, external_id: &str| {
            let mut row = row(id, verb, transaction, dec!(-4), recorded_on);
            row.movements[0].external_id = Some(external_id.to_string());
            row
        };
        let apply = ImportApply::new(proposal(vec![
            bank_row(
                1,
                ImportVerbDto::Add,
                regular(None, dec!(-4), &[]),
                None,
                "added",
            ),
            bank_row(
                2,
                ImportVerbDto::Amend,
                regular(Some(ledger), dec!(-4), &[]),
                Some(ImportRecordedOnDto::Transaction(ledger)),
                "amended",
            ),
            bank_row(
                3,
                ImportVerbDto::Skip,
                regular(None, dec!(-4), &[]),
                Some(ImportRecordedOnDto::Row(Uuid::from_u128(1))),
                "matched",
            ),
            bank_row(
                4,
                ImportVerbDto::Skip,
                regular(None, dec!(-4), &[]),
                None,
                "skipped",
            ),
            row(
                5,
                ImportVerbDto::Add,
                regular(None, dec!(-9), &[]),
                dec!(-9),
                None,
            ),
        ]));

        let lines = apply.external_lines(&HashMap::from([
            (Uuid::from_u128(1), new),
            (Uuid::from_u128(5), Uuid::from_u128(901)),
        ]));

        let recorded: Vec<_> = lines
            .iter()
            .map(|line| (line.external_id.as_str(), line.transaction_id))
            .collect();
        assert_eq!(
            recorded,
            vec![
                ("added", Some(new)),
                ("amended", Some(ledger)),
                ("matched", Some(new)),
                ("skipped", None),
            ]
        );
        assert!(lines.iter().all(|line| line.source_id == SOURCE));
    }

    fn starting_balance(id: u128, verb: ImportVerbDto) -> ImportProposalRowDto {
        let mut row = row(id, verb, regular(None, dec!(1000), &[]), dec!(1000), None);
        row.kind = ImportRowKindDto::StartingBalance(ImportStartingBalanceDto {
            account_id: ACCOUNT,
            asset_id: GBP,
            source_id: SOURCE,
            date: date!(2026 - 02 - 28),
            quantity: dec!(1000),
        });
        row.movements.clear();
        row.recorded_on.clear();
        row
    }

    #[test]
    fn a_starting_balance_is_recorded_as_the_statements_balance_and_a_skipped_one_nowhere() {
        let seeded = Uuid::from_u128(900);
        let paid = Uuid::from_u128(901);
        let apply = ImportApply::new(proposal(vec![
            starting_balance(1, ImportVerbDto::Add),
            starting_balance(2, ImportVerbDto::Skip),
            row(
                3,
                ImportVerbDto::Add,
                regular(None, dec!(-35), &[]),
                dec!(-35),
                None,
            ),
        ]));

        let sources = apply.sources(&HashMap::from([
            (Uuid::from_u128(1), seeded),
            (Uuid::from_u128(3), paid),
        ]));

        let recorded: Vec<_> = sources
            .iter()
            .map(|source| {
                (
                    source.transaction_id,
                    source.kind,
                    source.date,
                    source.external_ref.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            recorded,
            vec![
                (
                    seeded,
                    SourceKindDto::Balance,
                    date!(2026 - 02 - 28),
                    Some("starting-balance")
                ),
                (
                    paid,
                    SourceKindDto::Statement,
                    date!(2026 - 03 - 03),
                    Some("2026-03-03|-35|1")
                ),
            ]
        );
        assert!(sources
            .iter()
            .all(|source| source.stream == format!("statement:{SOURCE}")));
    }

    #[test]
    fn an_amend_the_proposal_could_not_build_is_refused() {
        let target_id = Uuid::from_u128(500);
        let mut statement_row = row(
            1,
            ImportVerbDto::Amend,
            regular(Some(target_id), dec!(-35), &[]),
            dec!(-35),
            Some(ImportRecordedOnDto::Transaction(target_id)),
        );
        statement_row.amend_refusal = Some(ImportAmendRefusalDto::TargetGone);
        let apply = ImportApply::new(proposal(vec![statement_row]));

        let error = apply.amended().unwrap_err();

        let refusal = error
            .downcast_ref::<BusinessValidationErrorDto>()
            .expect("a validation error");
        assert!(refusal.errors[0]
            .message
            .contains("the transaction it matched is gone"));
    }

    #[test]
    fn a_row_recorded_on_a_deleted_transaction_is_a_contradiction() {
        let doomed = Uuid::from_u128(7);
        let apply = ImportApply::new(proposal(vec![
            row(
                1,
                ImportVerbDto::Delete,
                regular(Some(doomed), dec!(-1), &[]),
                dec!(-1),
                None,
            ),
            row(
                2,
                ImportVerbDto::Skip,
                regular(Some(doomed), dec!(-1), &[]),
                dec!(-1),
                Some(ImportRecordedOnDto::Transaction(doomed)),
            ),
        ]));

        assert!(apply.contradiction().is_some());
    }

    #[test]
    fn two_rows_correcting_one_transaction_are_a_contradiction() {
        let target = Some(Uuid::from_u128(8));
        let apply = ImportApply::new(proposal(vec![
            row(
                1,
                ImportVerbDto::Amend,
                regular(target, dec!(-1), &[]),
                dec!(-1),
                None,
            ),
            row(
                2,
                ImportVerbDto::Amend,
                regular(target, dec!(-2), &[]),
                dec!(-2),
                None,
            ),
        ]));

        assert!(apply.contradiction().is_some());
    }

    #[test]
    fn the_parts_of_a_purchase_are_grouped_and_none_of_them_may_be_deleted() {
        let parts = [7, 8].map(Uuid::from_u128).to_vec();
        let mut tesco = row(
            1,
            ImportVerbDto::Skip,
            regular(Some(parts[0]), dec!(-50), &[]),
            dec!(-50),
            Some(ImportRecordedOnDto::Transaction(parts[0])),
        );
        let group = ImportPartsDto {
            transaction_ids: parts.clone(),
            description: "TESCO".to_string(),
            category_id: CATEGORY,
        };
        tesco.parts = Some(group.clone());
        let apply = ImportApply::new(proposal(vec![tesco.clone()]));
        assert_eq!(apply.groups(), vec![&group]);
        assert_eq!(apply.contradiction(), None);

        let mut deleting = row(
            2,
            ImportVerbDto::Delete,
            regular(Some(parts[1]), dec!(-30), &[]),
            dec!(-30),
            None,
        );
        deleting.movements.clear();
        let apply = ImportApply::new(proposal(vec![tesco, deleting]));
        assert_eq!(
            apply.contradiction(),
            Some("A row is recorded on a ledger transaction the import deletes.")
        );
    }
}
