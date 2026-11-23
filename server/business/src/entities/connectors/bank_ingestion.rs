use std::collections::{HashMap, HashSet};

use anyhow::{ensure, Context};
use dal::models::connector_models::{
    AddConnectorTransactionModel, AddHeldTransactionModel, ConnectorTransactionRow,
};
use itertools::Itertools;
use rust_decimal::Decimal;
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::dtos::connectors::HeldTransactionDto;
use crate::dtos::entry_dto::EntryDto;
use crate::dtos::ledger_matching_dto::{
    IncomingMovementDto, LedgerActivityDto, LedgerMatchBatchDto, LedgerMovementDto,
    MatchCandidateDto, MatchOutcomeDto, SourceKindDto, TransactionSourceDto,
};
use crate::dtos::transaction_dto::{
    CashBalanceTransferMetadataDto, RegularTransactionMetadataDto, TransactionDto,
    TransactionTypeDto, TransactionVisibilityDto,
};
use crate::entities::connectors::provider_transaction_import::IMPORT_CATEGORY_ID;
use crate::entities::ledger_matching::{may_amend, LedgerMatching};
use crate::entities::transactions::metadata::ConnectorLinkMeta;
use crate::entities::transactions::transaction::Transaction;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

const MAX_DAYS_BETWEEN_TRANSFER_LEGS: i64 = 3;

pub(crate) fn synced_visibility(write_mode: &str) -> TransactionVisibilityDto {
    match write_mode {
        "trusted" => TransactionVisibilityDto::Default,
        _ => TransactionVisibilityDto::Ghost,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BankRow {
    pub binding_id: Uuid,
    pub external_id: String,
    pub external_hash: String,
    pub account_id: Uuid,
    pub asset_id: i32,
    pub quantity: Decimal,
    pub date_transacted: OffsetDateTime,
    pub description: String,
    pub counterparty_iban: Option<String>,
}

impl From<&HeldTransactionDto> for BankRow {
    fn from(held: &HeldTransactionDto) -> Self {
        Self {
            binding_id: held.binding_id,
            external_id: held.external_id.clone(),
            external_hash: held.external_hash.clone(),
            account_id: held.account_id,
            asset_id: held.asset_id,
            quantity: held.quantity,
            date_transacted: held.date_transacted,
            description: held.description.clone(),
            counterparty_iban: None,
        }
    }
}

impl BankRow {
    fn stream(&self) -> String {
        format!("connector:{}", self.binding_id)
    }

    pub fn movement(&self) -> IncomingMovementDto {
        IncomingMovementDto {
            id: Uuid::new_v5(&self.binding_id, self.external_id.as_bytes()),
            kind: SourceKindDto::Connector,
            stream: self.stream(),
            external_ref: Some(self.external_id.clone()),
            account_id: Some(self.account_id),
            asset_id: self.asset_id,
            quantity: self.quantity,
            date: self.date_transacted.date(),
            description: Some(self.description.clone()),
            itemised_fee: None,
            is_fee: false,
        }
    }

    pub fn source(&self, transaction_id: Uuid) -> TransactionSourceDto {
        TransactionSourceDto {
            transaction_id,
            account_id: self.account_id,
            asset_id: self.asset_id,
            date: self.date_transacted.date(),
            description: Some(self.description.clone()),
            kind: SourceKindDto::Connector,
            stream: self.stream(),
            external_ref: Some(self.external_id.clone()),
            itemised_fee: None,
        }
    }

    pub fn link(&self, transaction_id: Uuid) -> AddConnectorTransactionModel {
        AddConnectorTransactionModel {
            binding_id: self.binding_id,
            transaction_id: Some(transaction_id),
            external_id: self.external_id.clone(),
            external_hash: self.external_hash.clone(),
        }
    }

    fn held(&self, user_id: Uuid, candidate_ids: Vec<Uuid>) -> AddHeldTransactionModel {
        AddHeldTransactionModel {
            user_id,
            binding_id: self.binding_id,
            external_id: self.external_id.clone(),
            external_hash: self.external_hash.clone(),
            asset_id: self.asset_id,
            quantity: self.quantity,
            date_transacted: self.date_transacted,
            description: self.description.clone(),
            candidate_ids,
        }
    }

    pub fn transaction(
        &self,
        user_id: Uuid,
        visibility: TransactionVisibilityDto,
    ) -> anyhow::Result<Transaction> {
        let mut transaction = create_transaction_from_dto(
            TransactionDto {
                transaction_id: None,
                date: self.date_transacted,
                visibility,
                fee_entries: Vec::new(),
                transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                    description: Some(self.description.clone()),
                    entry: EntryDto::new(self.asset_id, self.account_id, self.quantity),
                    category_id: IMPORT_CATEGORY_ID,
                }),
            },
            user_id,
        )?;
        transaction.set_connector_link(Some(ConnectorLinkMeta {
            binding_id: self.binding_id,
            external_id: self.external_id.clone(),
            external_hash: self.external_hash.clone(),
        }));
        Ok(transaction)
    }

    pub fn can_be_recorded_on(&self, linked_binding_ids: &[Uuid]) -> bool {
        !linked_binding_ids.contains(&self.binding_id)
    }

    pub fn same_as(
        &self,
        target: TransactionDto,
        ledger: LedgerActivityDto,
        user_id: Uuid,
    ) -> anyhow::Result<SameDecision> {
        let transaction_id = target
            .transaction_id
            .context("a ledger transaction has its id")?;
        let matching = LedgerMatching::new(ledger.movements, ledger.sources);
        let movement = self.movement();
        if matching.already_holds(transaction_id, &movement) {
            return Ok(SameDecision::Refused);
        }
        if !matching
            .summary(transaction_id, &movement)
            .is_some_and(|summary| may_amend(&summary))
        {
            return Ok(SameDecision::Attach);
        }
        let mut transaction = create_transaction_from_dto(target, user_id)?;
        let [entry] = transaction.get_entries_mut().as_mut_slice() else {
            return Ok(SameDecision::Attach);
        };
        if entry.account_id != self.account_id || entry.asset_id != self.asset_id {
            return Ok(SameDecision::Attach);
        }
        let unchanged = entry.quantity == self.quantity;
        entry.quantity = self.quantity;
        let mut amended = transaction.try_into_dto()?;
        if unchanged && amended.date == self.date_transacted {
            return Ok(SameDecision::Attach);
        }
        amended.date = self.date_transacted;
        Ok(SameDecision::Amend(amended))
    }

    fn other_leg_account(&self, own_accounts: &HashMap<String, Uuid>) -> Option<Uuid> {
        let account_id = *own_accounts.get(self.counterparty_iban.as_ref()?)?;
        (account_id != self.account_id).then_some(account_id)
    }

    fn is_other_leg(
        &self,
        own_accounts: &HashMap<String, Uuid>,
        entry: &EntryDto,
        date: Date,
    ) -> bool {
        self.other_leg_account(own_accounts) == Some(entry.account_id)
            && entry.asset_id == self.asset_id
            && entry.quantity == -self.quantity
            && (date - self.date_transacted.date()).abs()
                <= Duration::days(MAX_DAYS_BETWEEN_TRANSFER_LEGS)
    }

    fn transfer(&self, other_leg: &TransactionDto, other_entry: &EntryDto) -> TransactionDto {
        let own_entry = EntryDto::new(self.asset_id, self.account_id, self.quantity);
        let (outgoing_change, incoming_change, date) = if self.quantity.is_sign_negative() {
            (own_entry, other_entry.clone(), self.date_transacted)
        } else {
            (other_entry.clone(), own_entry, other_leg.date)
        };
        TransactionDto {
            transaction_id: other_leg.transaction_id,
            date,
            visibility: other_leg.visibility,
            fee_entries: Vec::new(),
            transaction_type: TransactionTypeDto::CashBalanceTransfer(
                CashBalanceTransferMetadataDto {
                    outgoing_change,
                    incoming_change,
                },
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SameDecision {
    Refused,
    Attach,
    Amend(TransactionDto),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BankRowAction {
    Recorded(Uuid),
    Attach(Uuid),
    Hold(Vec<Uuid>),
    Write,
}

impl From<MatchOutcomeDto> for BankRowAction {
    fn from(outcome: MatchOutcomeDto) -> Self {
        let largest = |targets: Vec<MatchCandidateDto>| {
            targets
                .into_iter()
                .filter(|target| target.in_ledger)
                .max_by_key(|target| target.quantity.abs())
                .map_or(Self::Write, |target| Self::Attach(target.id))
        };
        match outcome {
            MatchOutcomeDto::Recorded { transaction_id } => Self::Recorded(transaction_id),
            MatchOutcomeDto::Same { targets } => largest(targets),
            MatchOutcomeDto::Parts { parts } => largest(parts),
            MatchOutcomeDto::Ambiguous { ranked } => {
                let candidate_ids: Vec<Uuid> = ranked
                    .into_iter()
                    .filter(|candidate| candidate.in_ledger)
                    .map(|candidate| candidate.id)
                    .collect();
                if candidate_ids.is_empty() {
                    Self::Write
                } else {
                    Self::Hold(candidate_ids)
                }
            }
            MatchOutcomeDto::New => Self::Write,
        }
    }
}

#[derive(Default)]
pub(crate) struct BankIngestion {
    rows: Vec<(BankRow, Transaction)>,
    others: Vec<Transaction>,
}

pub(crate) struct BankIngestionWrites {
    pub rows: Vec<(BankRow, Transaction)>,
    pub others: Vec<Transaction>,
    pub transfers: Vec<TransactionDto>,
    pub links: Vec<AddConnectorTransactionModel>,
    pub held: Vec<AddHeldTransactionModel>,
    attached_sources: Vec<TransactionSourceDto>,
}

impl BankIngestion {
    pub fn add_row(&mut self, row: BankRow, transaction: Transaction) {
        self.rows.push((row, transaction));
    }

    pub fn add_other(&mut self, transaction: Transaction) {
        self.others.push(transaction);
    }

    pub fn batch(&self) -> LedgerMatchBatchDto {
        LedgerMatchBatchDto {
            incoming: self.rows.iter().map(|(row, _)| row.movement()).collect(),
            settled: Vec::new(),
        }
    }

    pub fn writes(
        self,
        user_id: Uuid,
        outcomes: Vec<MatchOutcomeDto>,
    ) -> anyhow::Result<BankIngestionWrites> {
        ensure!(
            outcomes.len() == self.rows.len(),
            "the matcher answered {} of {} bank rows",
            outcomes.len(),
            self.rows.len()
        );
        let mut writes = BankIngestionWrites {
            rows: Vec::new(),
            others: self.others,
            transfers: Vec::new(),
            links: Vec::new(),
            held: Vec::new(),
            attached_sources: Vec::new(),
        };
        for ((row, transaction), outcome) in self.rows.into_iter().zip(outcomes) {
            match BankRowAction::from(outcome) {
                BankRowAction::Recorded(transaction_id) => {
                    writes.links.push(row.link(transaction_id));
                }
                BankRowAction::Attach(transaction_id) => {
                    writes.links.push(row.link(transaction_id));
                    writes.attached_sources.push(row.source(transaction_id));
                }
                BankRowAction::Hold(candidate_ids) => {
                    writes.held.push(row.held(user_id, candidate_ids));
                }
                BankRowAction::Write => writes.rows.push((row, transaction)),
            }
        }
        Ok(writes)
    }
}

impl BankIngestionWrites {
    pub fn attached(&self) -> usize {
        self.links.len()
    }

    pub fn new_transactions(&self) -> usize {
        self.rows.len() + self.others.len()
    }

    pub fn counterparty_ibans(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter_map(|(row, _)| row.counterparty_iban.clone())
            .unique()
            .collect()
    }

    pub fn transfer_window(&self, own_accounts: &HashMap<String, Uuid>) -> Option<(Date, Date)> {
        let (first, last) = self
            .rows
            .iter()
            .filter(|(row, _)| row.other_leg_account(own_accounts).is_some())
            .map(|(row, _)| row.date_transacted.date())
            .minmax()
            .into_option()?;
        let days = Duration::days(MAX_DAYS_BETWEEN_TRANSFER_LEGS);
        Some((first - days, last + days))
    }

    pub fn transfer_candidates(
        &self,
        own_accounts: &HashMap<String, Uuid>,
        movements: &[LedgerMovementDto],
    ) -> Vec<Uuid> {
        movements
            .iter()
            .filter(|movement| {
                let entry =
                    EntryDto::new(movement.asset_id, movement.account_id, movement.quantity);
                movement.group_id.is_none()
                    && self
                        .rows
                        .iter()
                        .any(|(row, _)| row.is_other_leg(own_accounts, &entry, movement.date))
            })
            .map(|movement| movement.transaction_id)
            .unique()
            .collect()
    }

    pub fn pair_transfers(
        &mut self,
        own_accounts: &HashMap<String, Uuid>,
        candidates: Vec<TransactionDto>,
        links: &[ConnectorTransactionRow],
    ) {
        let synced: HashSet<Uuid> = links
            .iter()
            .filter_map(|link| link.transaction_id)
            .collect();
        let edited: HashSet<Uuid> = links
            .iter()
            .filter(|link| link.edited_by_user)
            .filter_map(|link| link.transaction_id)
            .collect();
        let legs: Vec<(Uuid, &TransactionDto, &EntryDto)> = candidates
            .iter()
            .filter_map(|candidate| {
                let transaction_id = candidate.transaction_id?;
                let TransactionTypeDto::Regular(regular) = &candidate.transaction_type else {
                    return None;
                };
                (candidate.fee_entries.is_empty()
                    && synced.contains(&transaction_id)
                    && !edited.contains(&transaction_id))
                .then_some((transaction_id, candidate, &regular.entry))
            })
            .collect();
        let fits: Vec<(usize, usize)> = self
            .rows
            .iter()
            .enumerate()
            .flat_map(|(row_index, (row, _))| {
                legs.iter()
                    .enumerate()
                    .filter(move |(_, (_, leg, entry))| {
                        row.is_other_leg(own_accounts, entry, leg.date.date())
                    })
                    .map(move |(leg_index, _)| (row_index, leg_index))
            })
            .collect();
        let legs_per_row = fits.iter().counts_by(|(row_index, _)| *row_index);
        let rows_per_leg = fits.iter().counts_by(|(_, leg_index)| *leg_index);
        let pairs: HashMap<usize, usize> = fits
            .into_iter()
            .filter(|(row_index, leg_index)| {
                legs_per_row[row_index] == 1 && rows_per_leg[leg_index] == 1
            })
            .collect();

        for (row_index, (row, transaction)) in
            std::mem::take(&mut self.rows).into_iter().enumerate()
        {
            let Some(&leg_index) = pairs.get(&row_index) else {
                if let Some(fitting_legs) = legs_per_row.get(&row_index) {
                    tracing::info!(
                        external_id = %row.external_id,
                        fitting_legs,
                        "a transfer's other leg is not certain (several legs fit, or the leg fits another row too); leaving it unpaired"
                    );
                }
                self.rows.push((row, transaction));
                continue;
            };
            let (transaction_id, leg, entry) = legs[leg_index];
            self.links.push(row.link(transaction_id));
            self.attached_sources.push(row.source(transaction_id));
            self.transfers.push(row.transfer(leg, entry));
        }
    }

    pub fn sources(&self, written: &[(BankRow, Uuid)]) -> Vec<TransactionSourceDto> {
        self.attached_sources
            .iter()
            .cloned()
            .chain(
                written
                    .iter()
                    .map(|(row, transaction_id)| row.source(*transaction_id)),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    use super::*;
    use crate::dtos::fee_entry_dto::FeeEntryDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::ledger_matching_dto::LedgerMovementDto;
    use crate::test_support::load_dynamic_enums;

    const EUR: i32 = 45;
    const GROCERIES: i32 = 25;

    fn account() -> Uuid {
        Uuid::from_u128(10)
    }

    fn row(external_id: &str, quantity: Decimal) -> BankRow {
        BankRow {
            binding_id: Uuid::from_u128(1),
            external_id: external_id.to_string(),
            external_hash: format!("hash-{external_id}"),
            account_id: account(),
            asset_id: EUR,
            quantity,
            date_transacted: datetime!(2026-11-18 0:00 UTC),
            description: format!("Bank {external_id}"),
            counterparty_iban: None,
        }
    }

    fn candidate(id: u128, quantity: Decimal, in_ledger: bool) -> MatchCandidateDto {
        MatchCandidateDto {
            id: Uuid::from_u128(id),
            in_ledger,
            confirmed: false,
            several_transactions: false,
            leaves_movement_unrecorded: false,
            account_id: Some(account()),
            asset_id: EUR,
            quantity,
            date: time::macros::date!(2026 - 11 - 17),
            description: None,
        }
    }

    fn manual(quantity: Decimal, fee: Option<Decimal>) -> TransactionDto {
        TransactionDto {
            transaction_id: Some(Uuid::from_u128(99)),
            date: datetime!(2026-11-17 13:45 UTC),
            visibility: TransactionVisibilityDto::Default,
            fee_entries: fee
                .map(|fee| FeeEntryDto {
                    entry: EntryDto::new(EUR, account(), fee),
                    entry_type: FeeEntryTypesDto::Transaction,
                })
                .into_iter()
                .collect(),
            transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: Some("Lunch with Sam".to_string()),
                entry: EntryDto::new(EUR, account(), quantity),
                category_id: GROCERIES,
            }),
        }
    }

    fn regular(dto: &TransactionDto) -> (&RegularTransactionMetadataDto, OffsetDateTime) {
        match &dto.transaction_type {
            TransactionTypeDto::Regular(regular) => (regular, dto.date),
            other => panic!("expected a regular transaction, got {other:?}"),
        }
    }

    #[test]
    fn matches_with_nothing_in_the_ledger_are_written() {
        let same = MatchOutcomeDto::Same {
            targets: vec![candidate(2, dec!(-1), false)],
        };
        let ambiguous = MatchOutcomeDto::Ambiguous {
            ranked: vec![candidate(2, dec!(-1), false)],
        };

        assert_eq!(BankRowAction::from(same), BankRowAction::Write);
        assert_eq!(BankRowAction::from(ambiguous), BankRowAction::Write);
        assert_eq!(
            BankRowAction::from(MatchOutcomeDto::New),
            BankRowAction::Write
        );
    }

    fn ledger_movement(group_id: Option<Uuid>, linked: bool) -> LedgerMovementDto {
        LedgerMovementDto {
            transaction_id: Uuid::from_u128(99),
            group_id,
            date: time::macros::date!(2026 - 11 - 17),
            account_id: account(),
            asset_id: EUR,
            quantity: dec!(-12),
            description: Some("Lunch with Sam".to_string()),
            connector_binding_ids: if linked {
                vec![Uuid::from_u128(2)]
            } else {
                Vec::new()
            },
        }
    }

    fn typed_by_hand() -> LedgerActivityDto {
        LedgerActivityDto {
            movements: vec![ledger_movement(None, false)],
            sources: Vec::new(),
        }
    }

    fn attaches_only(decision: SameDecision) -> bool {
        matches!(decision, SameDecision::Attach)
    }

    #[test]
    fn same_takes_the_banks_amount_and_date_and_keeps_the_users_description_and_category() {
        load_dynamic_enums();
        let bank = row("tx", dec!(-12.34));

        let SameDecision::Amend(amended) = bank
            .same_as(manual(dec!(-12), None), typed_by_hand(), Uuid::nil())
            .unwrap()
        else {
            panic!("a single movement in the same account and asset is amended");
        };

        let (regular, date) = regular(&amended);
        assert_eq!(regular.entry.quantity, dec!(-12.34));
        assert_eq!(date, bank.date_transacted);
        assert_eq!(regular.description.as_deref(), Some("Lunch with Sam"));
        assert_eq!(regular.category_id, GROCERIES);
        assert_eq!(amended.transaction_id, Some(Uuid::from_u128(99)));
    }

    #[test]
    fn same_only_attaches_to_a_target_a_bank_already_confirms_or_one_of_several_grouped_transactions(
    ) {
        load_dynamic_enums();
        let bank = row("tx", dec!(-12.34));
        let on_a_statement = LedgerActivityDto {
            movements: vec![ledger_movement(None, false)],
            sources: vec![TransactionSourceDto {
                transaction_id: Uuid::from_u128(99),
                account_id: account(),
                asset_id: EUR,
                date: time::macros::date!(2026 - 11 - 17),
                description: None,
                kind: SourceKindDto::Statement,
                stream: "statement:1".to_string(),
                external_ref: Some("2026-11-17|-12|1".to_string()),
                itemised_fee: None,
            }],
        };
        let synced = LedgerActivityDto {
            movements: vec![ledger_movement(None, true)],
            sources: Vec::new(),
        };
        let grouped = LedgerActivityDto {
            movements: vec![
                ledger_movement(Some(Uuid::from_u128(5)), false),
                LedgerMovementDto {
                    transaction_id: Uuid::from_u128(98),
                    ..ledger_movement(Some(Uuid::from_u128(5)), false)
                },
            ],
            sources: Vec::new(),
        };

        for ledger in [on_a_statement, synced, grouped] {
            assert!(attaches_only(
                bank.same_as(manual(dec!(-12), None), ledger, Uuid::nil())
                    .unwrap()
            ));
        }
    }

    #[test]
    fn same_refuses_a_target_that_already_holds_a_row_of_this_bank_account() {
        load_dynamic_enums();
        let bank = row("second coffee", dec!(-12));
        let mut holds_the_first_coffee = ledger_movement(None, false);
        holds_the_first_coffee.connector_binding_ids = vec![bank.binding_id];
        let ledger = LedgerActivityDto {
            movements: vec![holds_the_first_coffee],
            sources: Vec::new(),
        };

        assert!(matches!(
            bank.same_as(manual(dec!(-12), None), ledger, Uuid::nil())
                .unwrap(),
            SameDecision::Refused
        ));
        assert!(!bank.can_be_recorded_on(&[Uuid::from_u128(2), bank.binding_id]));
        assert!(bank.can_be_recorded_on(&[Uuid::from_u128(2)]));
        assert!(bank.can_be_recorded_on(&[]));
    }

    const CURRENT_IBAN: &str = "LT067300010164068367";
    const SAVINGS_IBAN: &str = "LT121000011101001000";

    fn current() -> Uuid {
        Uuid::from_u128(10)
    }

    fn savings() -> Uuid {
        Uuid::from_u128(11)
    }

    fn own_accounts() -> HashMap<String, Uuid> {
        HashMap::from([
            (CURRENT_IBAN.to_string(), current()),
            (SAVINGS_IBAN.to_string(), savings()),
        ])
    }

    fn sent_to_savings(external_id: &str, quantity: Decimal) -> BankRow {
        BankRow {
            counterparty_iban: Some(SAVINGS_IBAN.to_string()),
            ..row(external_id, quantity)
        }
    }

    fn new_rows(rows: Vec<BankRow>) -> BankIngestionWrites {
        load_dynamic_enums();
        let mut ingestion = BankIngestion::default();
        let outcomes = rows.iter().map(|_| MatchOutcomeDto::New).collect();
        for row in rows {
            let transaction = row
                .transaction(Uuid::nil(), TransactionVisibilityDto::Ghost)
                .unwrap();
            ingestion.add_row(row, transaction);
        }
        ingestion.writes(Uuid::nil(), outcomes).unwrap()
    }

    fn synced_leg(id: u128, account_id: Uuid, quantity: Decimal, day: u8) -> TransactionDto {
        TransactionDto {
            transaction_id: Some(Uuid::from_u128(id)),
            date: datetime!(2026-11-01 0:00 UTC).replace_day(day).unwrap(),
            visibility: TransactionVisibilityDto::Ghost,
            fee_entries: Vec::new(),
            transaction_type: TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                description: Some("Transfer".to_string()),
                entry: EntryDto {
                    entry_id: Some(id as i32),
                    ..EntryDto::new(EUR, account_id, quantity)
                },
                category_id: IMPORT_CATEGORY_ID,
            }),
        }
    }

    fn transfer_legs(transfer: &TransactionDto) -> (&EntryDto, &EntryDto) {
        match &transfer.transaction_type {
            TransactionTypeDto::CashBalanceTransfer(legs) => {
                (&legs.outgoing_change, &legs.incoming_change)
            }
            other => panic!("expected a transfer, got {other:?}"),
        }
    }

    fn synced(ids: &[u128]) -> Vec<ConnectorTransactionRow> {
        ids.iter()
            .map(|id| ConnectorTransactionRow {
                binding_id: Uuid::from_u128(3),
                transaction_id: Some(Uuid::from_u128(*id)),
                external_id: format!("synced-{id}"),
                external_hash: format!("hash-{id}"),
                edited_by_user: false,
            })
            .collect()
    }

    fn paired(writes: &BankIngestionWrites) -> Vec<(&str, Option<Uuid>)> {
        writes
            .links
            .iter()
            .map(|link| (link.external_id.as_str(), link.transaction_id))
            .collect()
    }

    #[test]
    fn a_new_row_turns_its_other_leg_into_a_transfer_and_is_attached_to_it() {
        let mut writes = new_rows(vec![sent_to_savings("out", dec!(-100))]);

        writes.pair_transfers(
            &own_accounts(),
            vec![synced_leg(7, savings(), dec!(100), 19)],
            &synced(&[7]),
        );

        assert!(writes.rows.is_empty());
        assert_eq!(paired(&writes), vec![("out", Some(Uuid::from_u128(7)))]);
        let sources: Vec<(Uuid, Uuid)> = writes
            .sources(&[])
            .into_iter()
            .map(|source| (source.transaction_id, source.account_id))
            .collect();
        assert_eq!(sources, vec![(Uuid::from_u128(7), current())]);
        let [transfer] = writes.transfers.as_slice() else {
            panic!("expected one transfer");
        };
        assert_eq!(transfer.transaction_id, Some(Uuid::from_u128(7)));
        assert_eq!(transfer.date, datetime!(2026-11-18 0:00 UTC));
        assert_eq!(transfer.visibility, TransactionVisibilityDto::Ghost);
        let (outgoing, incoming) = transfer_legs(transfer);
        assert_eq!(
            (outgoing.entry_id, outgoing.account_id, outgoing.quantity),
            (None, current(), dec!(-100))
        );
        assert_eq!(
            (incoming.entry_id, incoming.account_id, incoming.quantity),
            (Some(7), savings(), dec!(100))
        );
    }

    #[test]
    fn an_edited_leg_a_leg_the_bank_did_not_write_one_with_fees_or_one_that_is_not_regular_stays_alone(
    ) {
        let mut with_fee = synced_leg(8, savings(), dec!(100), 18);
        with_fee.fee_entries.push(FeeEntryDto {
            entry: EntryDto::new(EUR, savings(), dec!(-1)),
            entry_type: FeeEntryTypesDto::Transaction,
        });
        let transfer_in = TransactionDto {
            transaction_type: TransactionTypeDto::CashTransferIn(
                crate::dtos::transaction_dto::CashTransferInMetadataDto {
                    entry: EntryDto::new(EUR, savings(), dec!(100)),
                },
            ),
            ..synced_leg(9, savings(), dec!(100), 18)
        };

        let mut edited = synced(&[7]);
        edited[0].edited_by_user = true;

        for (candidate, links) in [
            (synced_leg(7, savings(), dec!(100), 18), edited),
            (synced_leg(7, savings(), dec!(100), 18), Vec::new()),
            (with_fee, synced(&[8])),
            (transfer_in, synced(&[9])),
        ] {
            let mut writes = new_rows(vec![sent_to_savings("out", dec!(-100))]);

            writes.pair_transfers(&own_accounts(), vec![candidate], &links);

            assert!(writes.transfers.is_empty());
            assert!(writes.links.is_empty());
            assert_eq!(writes.rows.len(), 1);
        }
    }

    #[test]
    fn a_leg_two_rows_could_take_pairs_neither_but_others_still_pair() {
        let mut writes = new_rows(vec![
            sent_to_savings("first", dec!(-100)),
            sent_to_savings("second", dec!(-100)),
            sent_to_savings("lunch money", dec!(-20)),
        ]);

        writes.pair_transfers(
            &own_accounts(),
            vec![
                synced_leg(7, savings(), dec!(100), 18),
                synced_leg(8, savings(), dec!(20), 17),
            ],
            &synced(&[7, 8]),
        );

        assert_eq!(
            paired(&writes),
            vec![("lunch money", Some(Uuid::from_u128(8)))]
        );
        let unpaired: Vec<&str> = writes
            .rows
            .iter()
            .map(|(row, _)| row.external_id.as_str())
            .collect();
        assert_eq!(unpaired, vec!["first", "second"]);
    }
}
