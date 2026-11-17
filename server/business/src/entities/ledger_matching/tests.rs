use std::collections::HashMap;

use dal::enums::transaction_types::DatabaseTransactionTypes;
use rust_decimal::Decimal;
use time::macros::date;
use time::{Date, Duration};
use uuid::Uuid;

use super::LedgerMatching;
use crate::dtos::ledger_matching_dto::{
    IncomingMovementDto, LedgerMatchBatchDto, LedgerMovementDto, MatchOutcomeDto,
    SettledMovementDto, SourceKindDto, TransactionSourceDto,
};

const ACCOUNT: Uuid = Uuid::from_u128(1);
const CASH_ACCOUNT: Uuid = Uuid::from_u128(2);
const GBP: i32 = 45;
const STATEMENT: &str = "statement:march";
const CONNECTOR: &str = "connector:northwind";

fn day(n: i64) -> Date {
    date!(2026 - 03 - 01) + Duration::days(n)
}

fn amount(text: &str) -> Decimal {
    text.parse().unwrap()
}

struct Rows {
    next: u128,
}

impl Rows {
    fn new() -> Self {
        Self { next: 100 }
    }

    fn row(
        &mut self,
        kind: SourceKindDto,
        stream: &str,
        when: i64,
        account_id: Option<Uuid>,
    ) -> IncomingMovementDto {
        self.next += 1;
        IncomingMovementDto {
            id: Uuid::from_u128(self.next),
            kind,
            stream: stream.to_string(),
            external_ref: None,
            account_id,
            asset_id: GBP,
            quantity: amount("-3.50"),
            date: day(when),
            description: Some("CORNER SHOP".to_string()),
            itemised_fee: None,
            is_fee: false,
        }
    }

    fn statement(&mut self, stream: &str, when: i64) -> IncomingMovementDto {
        self.row(SourceKindDto::Statement, stream, when, Some(ACCOUNT))
    }

    fn connector(&mut self, when: i64) -> IncomingMovementDto {
        self.row(SourceKindDto::Connector, CONNECTOR, when, Some(ACCOUNT))
    }

    fn receipt(&mut self, when: i64) -> IncomingMovementDto {
        let mut row = self.row(SourceKindDto::Receipt, "receipt", when, None);
        row.description = Some("Corner Shop".to_string());
        row
    }
}

fn empty_ledger() -> LedgerMatching {
    LedgerMatching::new(Vec::new(), Vec::new())
}

fn verbs(outcomes: Vec<MatchOutcomeDto>) -> Vec<&'static str> {
    outcomes
        .into_iter()
        .map(|outcome| match outcome {
            MatchOutcomeDto::Recorded { .. } => "recorded",
            MatchOutcomeDto::Same { .. } => "same",
            MatchOutcomeDto::Ambiguous { .. } => "ambiguous",
            MatchOutcomeDto::Parts { .. } => "parts",
            MatchOutcomeDto::New => "new",
        })
        .collect()
}

fn replay(batches: Vec<Vec<IncomingMovementDto>>) -> Vec<&'static str> {
    let mut matching = empty_ledger();
    batches
        .iter()
        .flat_map(|batch| verbs(matching.match_batch(batch)))
        .collect()
}

fn one_by_one(rows: Vec<IncomingMovementDto>) -> Vec<Vec<IncomingMovementDto>> {
    rows.into_iter().map(|row| vec![row]).collect()
}

fn assert_repeated(expected: &[(&'static str, usize)], actual: Vec<&'static str>) {
    let expected: Vec<&'static str> = expected
        .iter()
        .flat_map(|(verb, count)| std::iter::repeat_n(*verb, *count))
        .collect();
    assert_eq!(expected, actual);
}

#[test]
fn a_connector_one_day_early_flags_only_its_first_row() {
    let mut rows = Rows::new();
    let mut batches = vec![(3..=7)
        .map(|d| rows.statement(STATEMENT, d))
        .collect::<Vec<_>>()];
    batches.extend(one_by_one((2..=6).map(|d| rows.connector(d)).collect()));
    assert_repeated(
        &[("new", 5), ("ambiguous", 1), ("same", 4)],
        replay(batches),
    );
}

#[test]
fn overlapping_statements_link_only_the_days_both_cover() {
    let mut rows = Rows::new();
    let batches = vec![
        (2..=5).map(|d| rows.statement("statement:f1", d)).collect(),
        (4..=7).map(|d| rows.statement("statement:f2", d)).collect(),
    ];
    assert_eq!(
        vec!["new", "new", "new", "new", "same", "same", "new", "new"],
        replay(batches)
    );
}

#[test]
fn consecutive_statements_keep_a_purchase_either_side_of_the_boundary() {
    let mut rows = Rows::new();
    let batches = vec![
        vec![rows.statement("statement:feb", 4)],
        vec![rows.statement("statement:mar", 5)],
    ];
    assert_eq!(vec!["new", "new"], replay(batches));
}

#[test]
fn a_receipt_each_evening_pairs_with_the_next_mornings_bank_row() {
    let mut rows = Rows::new();
    let mut batches = Vec::new();
    for offset in 0..5 {
        batches.push(vec![rows.receipt(2 + offset)]);
        batches.push(vec![rows.connector(3 + offset)]);
    }
    assert_eq!(["new", "same"].repeat(5), replay(batches));
}

#[test]
fn a_month_of_repeats_adds_only_the_days_the_connector_missed() {
    let mut rows = Rows::new();
    let missing = [4, 13, 26];
    let mut batches = one_by_one(
        (0..30)
            .filter(|d| !missing.contains(d))
            .map(|d| rows.connector(d))
            .collect(),
    );
    batches.push((0..30).map(|d| rows.statement(STATEMENT, d)).collect());
    let verbs = replay(batches);
    assert_eq!(27, verbs.iter().filter(|verb| **verb == "same").count());
    assert_eq!(30, verbs.iter().filter(|verb| **verb == "new").count());
    assert!(!verbs.contains(&"ambiguous"));
}

fn ledger_movement(transaction: u128, when: i64, quantity: &str) -> LedgerMovementDto {
    LedgerMovementDto {
        transaction_id: Uuid::from_u128(transaction),
        group_id: None,
        date: day(when),
        account_id: ACCOUNT,
        asset_id: GBP,
        quantity: amount(quantity),
        description: Some("CORNER SHOP".to_string()),
        connector_binding_ids: Vec::new(),
    }
}

fn statement_source(transaction: u128, when: i64, reference: &str) -> TransactionSourceDto {
    TransactionSourceDto {
        transaction_id: Uuid::from_u128(transaction),
        account_id: ACCOUNT,
        asset_id: GBP,
        date: day(when),
        description: Some("CORNER SHOP".to_string()),
        kind: SourceKindDto::Statement,
        stream: STATEMENT.to_string(),
        external_ref: Some(reference.to_string()),
        itemised_fee: None,
    }
}

#[test]
fn a_row_its_feed_already_sent_is_recorded() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![ledger_movement(7, 3, "-3.50")],
        vec![statement_source(7, 3, "a")],
    );
    let known = referenced(rows.statement(STATEMENT, 3), "a");
    assert_eq!(
        vec![MatchOutcomeDto::Recorded {
            transaction_id: Uuid::from_u128(7)
        }],
        matching.match_batch(&[known])
    );
}

#[test]
fn a_starting_balance_is_never_a_candidate_for_a_bank_row() {
    let mut rows = Rows::new();
    let mut evidence = statement_source(7, 3, "starting-balance");
    evidence.kind = SourceKindDto::Balance;
    evidence.stream = "statement:february".to_string();
    let mut matching = LedgerMatching::new(vec![ledger_movement(7, 3, "-3.50")], vec![evidence]);
    assert_eq!(
        vec!["new", "new"],
        verbs(matching.match_batch(&[rows.statement(STATEMENT, 3), rows.connector(3)]))
    );
}

fn settled(movement: IncomingMovementDto, transaction: Option<u128>) -> SettledMovementDto {
    SettledMovementDto {
        movement,
        transaction_id: transaction.map(Uuid::from_u128),
    }
}

#[test]
fn a_row_the_user_linked_takes_the_transaction_for_its_own_file_only() {
    let mut rows = Rows::new();
    let linked = |rows: &mut Rows| {
        let mut matching = LedgerMatching::new(vec![ledger_movement(7, 3, "-3.50")], Vec::new());
        matching.add_settled(&[settled(rows.statement("statement:f1", 9), Some(7))]);
        matching
    };

    let same_file = rows.statement("statement:f1", 9);
    assert_eq!(
        vec!["new"],
        verbs(linked(&mut rows).match_batch(&[same_file]))
    );

    let other_file = rows.statement("statement:f2", 9);
    let outcome = linked(&mut rows).match_batch(&[other_file]);
    let MatchOutcomeDto::Same { targets } = &outcome[0] else {
        panic!("expected a link, got {outcome:?}");
    };
    assert_eq!(Uuid::from_u128(7), targets[0].id);
}

#[test]
fn different_shops_at_one_price_on_one_day_ask_with_the_closest_wording_first() {
    let mut rows = Rows::new();
    let mut tesco = rows.connector(3);
    tesco.description = Some("TESCO STORES".to_string());
    let mut shell = rows.connector(3);
    shell.description = Some("SHELL GARAGE".to_string());
    let mut matching = empty_ledger();
    matching.match_batch(&[tesco, shell]);

    let mut statement_shell = rows.statement(STATEMENT, 3);
    statement_shell.description = Some("SHELL GARAGE 123".to_string());
    let outcome = matching.match_batch(&[statement_shell]);
    let MatchOutcomeDto::Ambiguous { ranked } = &outcome[0] else {
        panic!("expected a question, got {outcome:?}");
    };
    assert_eq!(Some("SHELL GARAGE"), ranked[0].description.as_deref());
}

fn itemised(mut row: IncomingMovementDto, total: &str, fee: &str) -> IncomingMovementDto {
    row.quantity = amount(total);
    row.itemised_fee = Some(amount(fee));
    row
}

fn priced(mut row: IncomingMovementDto, quantity: &str) -> IncomingMovementDto {
    row.quantity = amount(quantity);
    row
}

#[test]
fn an_itemised_payment_equals_its_payment_and_fee_rows() {
    let mut rows = Rows::new();
    let parts = vec![
        priced(rows.statement("statement:pdf", 3), "-35.00"),
        priced(rows.statement("statement:pdf", 3), "-0.20"),
    ];
    let joined = itemised(rows.statement("statement:csv", 3), "-35.20", "-0.20");
    let mut matching = empty_ledger();
    matching.match_batch(&parts);
    let outcome = matching.match_batch(&[joined]);
    let MatchOutcomeDto::Same { targets } = &outcome[0] else {
        panic!("expected a link, got {outcome:?}");
    };
    assert_eq!(2, targets.len());
}

#[test]
fn payment_and_fee_rows_equal_an_itemised_payment_already_recorded() {
    let mut rows = Rows::new();
    let joined = itemised(rows.statement("statement:csv", 3), "-35.20", "-0.20");
    let parts = vec![
        priced(rows.statement("statement:pdf", 3), "-35.00"),
        priced(rows.statement("statement:pdf", 3), "-0.20"),
    ];
    assert_eq!(
        vec!["new", "same", "same"],
        replay(vec![vec![joined], parts])
    );
}

#[test]
fn two_purchases_that_merely_add_up_are_not_a_payment_and_fee() {
    let mut rows = Rows::new();
    let parts = vec![
        priced(rows.statement("statement:pdf", 3), "-12.99"),
        priced(rows.statement("statement:pdf", 3), "-7.00"),
    ];
    let joined = itemised(rows.statement("statement:csv", 3), "-19.99", "-0.50");
    assert_eq!(vec!["new", "new", "new"], replay(vec![parts, vec![joined]]));
}

#[test]
fn a_settled_row_can_still_be_matched_by_a_later_file() {
    let mut rows = Rows::new();
    let mut matching = empty_ledger();
    matching.add_settled(&[settled(rows.statement("statement:f1", 3), None)]);
    let outcome = matching.match_batch(&[rows.statement("statement:f2", 3)]);
    let MatchOutcomeDto::Same { targets } = &outcome[0] else {
        panic!("expected a link, got {outcome:?}");
    };
    assert!(!targets[0].in_ledger);
}

fn synced(transaction: u128, when: i64, quantity: &str) -> LedgerMovementDto {
    LedgerMovementDto {
        connector_binding_ids: vec![Uuid::from_u128(9)],
        ..ledger_movement(transaction, when, quantity)
    }
}

fn linked_to(outcome: &MatchOutcomeDto) -> Vec<Uuid> {
    match outcome {
        MatchOutcomeDto::Same { targets } => targets.iter().map(|target| target.id).collect(),
        other => panic!("expected a link, got {other:?}"),
    }
}

#[test]
fn a_bank_row_dated_before_the_statement_starts_is_offered_not_linked() {
    let mut rows = Rows::new();
    let ledger = || {
        vec![
            synced(1, 0, "-9.00"),
            synced(7, 9, "-3.50"),
            synced(2, 20, "-8.00"),
        ]
    };

    let mut matching = LedgerMatching::new(ledger(), Vec::new());
    let first_day = rows.statement(STATEMENT, 10);
    let later = priced(rows.statement(STATEMENT, 14), "-5.00");
    assert_eq!(
        vec!["ambiguous", "new"],
        verbs(matching.match_batch(&[first_day, later]))
    );

    let mut matching = LedgerMatching::new(ledger(), Vec::new());
    let earlier = priced(rows.statement(STATEMENT, 8), "-5.00");
    let inside_its_dates = rows.statement(STATEMENT, 10);
    assert_eq!(
        vec!["new", "same"],
        verbs(matching.match_batch(&[earlier, inside_its_dates]))
    );
}

#[test]
fn two_formats_of_one_statement_dated_a_day_apart_ask_at_both_edges() {
    let mut rows = Rows::new();
    let prices = ["-11.00", "-12.00", "-13.00", "-14.00"];
    let batches = vec![
        (2..)
            .zip(prices)
            .map(|(d, price)| priced(rows.statement("statement:pdf", d), price))
            .collect(),
        (3..)
            .zip(prices)
            .map(|(d, price)| priced(rows.statement("statement:csv", d), price))
            .collect(),
    ];
    assert_eq!(
        vec![
            "new",
            "new",
            "new",
            "new",
            "ambiguous",
            "same",
            "same",
            "ambiguous"
        ],
        replay(batches)
    );
}

#[test]
fn identical_typed_entries_are_not_rivals() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            ledger_movement(6, 3, "-3.50"),
            ledger_movement(7, 3, "-3.50"),
            ledger_movement(8, 3, "-3.50"),
        ],
        Vec::new(),
    );
    assert_eq!(
        vec!["same", "same"],
        verbs(matching.match_batch(&[rows.statement(STATEMENT, 4), rows.statement(STATEMENT, 4)]))
    );
}

#[test]
fn typed_entries_worded_differently_are_rivals() {
    let mut rows = Rows::new();
    let mut lunch = ledger_movement(7, 3, "-3.50");
    lunch.description = Some("lunch".to_string());
    let mut matching = LedgerMatching::new(vec![ledger_movement(6, 3, "-3.50"), lunch], Vec::new());
    assert_eq!(
        vec!["ambiguous"],
        verbs(matching.match_batch(&[rows.statement(STATEMENT, 4)]))
    );
}

#[test]
fn a_typed_amount_is_asked_about_when_it_is_the_bank_amount_without_its_cents_or_as_a_round_figure()
{
    let mut rows = Rows::new();
    let mut read = |entry: LedgerMovementDto, bank: &str| {
        let mut matching = LedgerMatching::new(vec![entry], Vec::new());
        verbs(matching.match_batch(&[priced(rows.statement(STATEMENT, 4), bank)]))[0]
    };
    for (typed, bank) in [
        ("-7", "-6.75"),
        ("-6", "-6.75"),
        ("-7", "-6.20"),
        ("2", "1.50"),
        ("-52", "-52.40"),
        ("-70", "-67.80"),
        ("-65", "-67.20"),
        ("-30", "-27.90"),
        ("-300", "-284.00"),
        ("-250", "-262.50"),
        ("-3000", "-2760.00"),
        ("-2000", "-2400.00"),
    ] {
        assert_eq!(
            "ambiguous",
            read(ledger_movement(7, 3, typed), bank),
            "{typed} for {bank}"
        );
    }
    for (typed, bank) in [
        ("-1", "-0.50"),
        ("1", "0.6"),
        ("0.0031", "0.00312"),
        ("-8", "-6.75"),
        ("-6.7", "-6.75"),
        ("-90", "-67.80"),
        ("-225", "-223.10"),
        ("-6", "6.75"),
    ] {
        assert_eq!(
            "new",
            read(ledger_movement(7, 3, typed), bank),
            "{typed} for {bank}"
        );
    }
    assert_eq!("same", read(ledger_movement(7, 3, "-6.5"), "-6.50"));
    assert_eq!("new", read(synced(7, 3, "-70"), "-67.80"));

    let typed_later = priced(
        rows.row(SourceKindDto::Manual, "manual", 3, Some(ACCOUNT)),
        "-70",
    );
    let mut booked = LedgerMatching::new(vec![synced(7, 4, "-67.80")], Vec::new());
    assert_eq!(vec!["ambiguous"], verbs(booked.match_batch(&[typed_later])));
}

#[test]
fn a_typed_entry_booked_much_later_asks_when_its_amount_occurs_once_on_each_side() {
    let mut rows = Rows::new();
    let typed = || LedgerMatching::new(vec![ledger_movement(7, 3, "-3.50")], Vec::new());
    assert_eq!(
        vec!["ambiguous"],
        verbs(typed().match_batch(&[rows.statement(STATEMENT, 15)]))
    );
    assert_eq!(
        vec!["new", "new"],
        verbs(typed().match_batch(&[rows.statement(STATEMENT, 15), rows.statement(STATEMENT, 16)]))
    );
    assert_eq!(
        vec!["new"],
        verbs(typed().match_batch(&[rows.statement(STATEMENT, 20)]))
    );

    let mut other_account = ledger_movement(7, 3, "-3.50");
    other_account.account_id = CASH_ACCOUNT;
    let mut matching = LedgerMatching::new(vec![other_account], Vec::new());
    assert_eq!(
        vec!["new"],
        verbs(matching.match_batch(&[rows.statement(STATEMENT, 15)]))
    );
}

#[test]
fn a_bank_row_is_dated_against_the_nearest_bank_source_of_a_transaction() {
    let mut rows = Rows::new();
    let mut purchase = synced(7, 10, "-3.50");
    purchase.description = Some("CARD PAYMENT 4471".to_string());
    let mut booked = statement_source(7, 12, "a");
    booked.description = Some("Tesco Stores Leith".to_string());
    let mut matching = LedgerMatching::new(
        vec![synced(1, 0, "-9.00"), purchase, synced(2, 20, "-8.00")],
        vec![booked],
    );
    let mut row = rows.statement("statement:pdf", 12);
    row.description = Some("POS 6279 LEEDS GB".to_string());
    assert_eq!(
        vec![Uuid::from_u128(7)],
        linked_to(&matching.match_batch(&[row])[0])
    );
}

fn referenced(mut row: IncomingMovementDto, reference: &str) -> IncomingMovementDto {
    row.external_ref = Some(reference.to_string());
    row
}

#[test]
fn a_row_another_upload_recorded_is_the_same_transaction_even_after_an_edit() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![ledger_movement(7, 3, "-2.00")],
        vec![statement_source(7, 3, "a")],
    );
    let again = referenced(rows.statement("statement:again", 3), "a");
    let outcome = matching.match_batch(&[again]);
    assert_eq!(vec![Uuid::from_u128(7)], linked_to(&outcome[0]));

    let later_file = matching.match_batch(&[rows.statement("statement:again", 3)]);
    assert_eq!(vec!["new"], verbs(later_file));
}

#[test]
fn a_file_holding_both_of_two_identical_rows_adds_the_one_an_earlier_file_lacked() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![ledger_movement(7, 3, "-3.50")],
        vec![statement_source(7, 3, "x|1")],
    );
    let outcome = matching.match_batch(&[
        referenced(rows.statement("statement:both", 3), "x|1"),
        referenced(rows.statement("statement:both", 3), "x|2"),
    ]);
    assert_eq!(vec![Uuid::from_u128(7)], linked_to(&outcome[0]));
    assert_eq!(MatchOutcomeDto::New, outcome[1]);
}

#[test]
fn a_reference_a_feed_made_up_is_known_to_that_feed_only() {
    let mut rows = Rows::new();
    let mut elsewhere = statement_source(7, 3, "a");
    elsewhere.kind = SourceKindDto::Connector;
    elsewhere.stream = "connector:other".to_string();
    let mut matching = LedgerMatching::new(Vec::new(), vec![elsewhere]);
    let row = referenced(rows.connector(3), "a");
    assert_eq!(vec!["new"], verbs(matching.match_batch(&[row])));
}

#[test]
fn a_group_member_a_bank_feed_created_is_matched_on_its_own() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            grouped(synced(6, 2, "-120.00")),
            grouped(synced(7, 5, "-300.00")),
            grouped(synced(8, 5, "300.00")),
        ],
        Vec::new(),
    );
    let outcome = matching.match_batch(&[
        priced(rows.statement(STATEMENT, 2), "-120.00"),
        priced(rows.statement(STATEMENT, 5), "-300.00"),
        priced(rows.statement(STATEMENT, 5), "300.00"),
    ]);
    let linked: Vec<Uuid> = outcome.iter().flat_map(linked_to).collect();
    assert_eq!([6, 7, 8].map(Uuid::from_u128).to_vec(), linked);
}

fn grouped(mut movement: LedgerMovementDto) -> LedgerMovementDto {
    movement.group_id = Some(Uuid::from_u128(50));
    movement
}

#[test]
fn a_payment_split_into_parts_is_one_movement_whichever_part_a_bank_source_describes() {
    let mut rows = Rows::new();
    let whole = |rows: &mut Rows, stream: &str| priced(rows.statement(stream, 3), "-50.00");

    let mut synced_then_split = LedgerMatching::new(
        vec![
            grouped(ledger_movement(7, 3, "-20.00")),
            grouped(synced(9, 3, "-30.00")),
        ],
        Vec::new(),
    );
    let outcome = synced_then_split.match_batch(&[whole(&mut rows, STATEMENT)]);
    let MatchOutcomeDto::Same { targets } = &outcome[0] else {
        panic!("expected a link, got {outcome:?}");
    };
    assert!(targets[0].several_transactions);
    assert_eq!(Uuid::from_u128(9), targets[0].id);

    let mut recorded_by_a_statement = LedgerMatching::new(
        vec![
            grouped(ledger_movement(7, 3, "-20.00")),
            grouped(ledger_movement(8, 3, "-30.00")),
        ],
        vec![statement_source(8, 3, "a")],
    );
    let outcome = recorded_by_a_statement.match_batch(&[whole(&mut rows, "statement:other")]);
    assert_eq!(vec![Uuid::from_u128(8)], linked_to(&outcome[0]));
}

#[test]
fn group_members_a_statement_recorded_one_by_one_are_matched_on_their_own() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            grouped(ledger_movement(7, 2, "-120.00")),
            grouped(ledger_movement(8, 5, "-300.00")),
        ],
        vec![statement_source(7, 2, "a"), statement_source(8, 5, "b")],
    );
    let outcome = matching.match_batch(&[
        priced(rows.statement("statement:other", 2), "-120.00"),
        priced(rows.statement("statement:other", 5), "-300.00"),
    ]);
    let linked: Vec<Uuid> = outcome.iter().flat_map(linked_to).collect();
    assert_eq!([7, 8].map(Uuid::from_u128).to_vec(), linked);
}

fn fee(mut row: IncomingMovementDto) -> IncomingMovementDto {
    row.is_fee = true;
    row
}

fn worded(mut movement: LedgerMovementDto, description: &str) -> LedgerMovementDto {
    movement.description = Some(description.to_string());
    movement
}

fn of_type(
    transaction: u128,
    kind: DatabaseTransactionTypes,
) -> HashMap<Uuid, DatabaseTransactionTypes> {
    HashMap::from([(Uuid::from_u128(transaction), kind)])
}

#[test]
fn a_payment_row_and_a_fee_row_nothing_keyed_together_are_one_net_bank_transaction() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(vec![synced(7, 3, "-101.50")], Vec::new());
    let outcome = matching.match_batch(&[
        priced(rows.statement(STATEMENT, 3), "-100.00"),
        fee(priced(rows.statement(STATEMENT, 3), "-1.50")),
    ]);
    assert_eq!(vec![Uuid::from_u128(7)], linked_to(&outcome[0]));
    assert_eq!(vec![Uuid::from_u128(7)], linked_to(&outcome[1]));
}

#[test]
fn a_net_row_is_a_bank_payment_and_the_fee_transaction_beside_it() {
    let mut rows = Rows::new();
    let ledger = || vec![synced(7, 3, "-200.00"), synced(8, 3, "-5.50")];
    let net = priced(rows.statement(STATEMENT, 3), "-205.50");

    let mut matching = LedgerMatching::new(ledger(), Vec::new());
    matching.add_transaction_types(&of_type(8, DatabaseTransactionTypes::AccountFees));
    let outcome = matching.match_batch(std::slice::from_ref(&net));
    assert_eq!([7, 8].map(Uuid::from_u128).to_vec(), linked_to(&outcome[0]));

    let mut two_purchases = LedgerMatching::new(ledger(), Vec::new());
    assert_eq!(vec!["new"], verbs(two_purchases.match_batch(&[net])));
}

#[test]
fn a_fee_two_payments_could_carry_and_rows_that_merely_add_up_are_not_summed() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![synced(7, 3, "-101.50"), synced(9, 10, "-19.99")],
        Vec::new(),
    );
    let outcome = matching.match_batch(&[
        priced(rows.statement(STATEMENT, 3), "-100.00"),
        priced(rows.statement(STATEMENT, 4), "-100.00"),
        fee(priced(rows.statement(STATEMENT, 3), "-1.50")),
        priced(rows.statement(STATEMENT, 10), "-12.99"),
        priced(rows.statement(STATEMENT, 10), "-7.00"),
    ]);
    assert_repeated(&[("new", 5)], verbs(outcome));
}

#[test]
fn a_bank_row_links_days_from_its_counterpart_at_a_lag_another_once_only_amount_shows() {
    let mut rows = Rows::new();
    let feed = |lag: i64, lag_of_last: i64| {
        let card = |transaction: u128, when: i64, quantity: &str| {
            worded(synced(transaction, when, quantity), "CARD PAYMENT 4471")
        };
        LedgerMatching::new(
            vec![
                synced(1, 0, "-9.00"),
                card(5, 1 + lag, "-6.10"),
                card(6, 2 + lag, "-7.20"),
                card(7, 3 + lag_of_last, "-3.50"),
                synced(2, 20, "-9.00"),
            ],
            Vec::new(),
        )
    };
    let mut statement = |last: i64| {
        vec![
            priced(rows.statement(STATEMENT, 1), "-6.10"),
            priced(rows.statement(STATEMENT, 2), "-7.20"),
            rows.statement(STATEMENT, 3),
            priced(rows.statement(STATEMENT, last), "-8.00"),
        ]
    };
    for lag in [2, 3] {
        assert_eq!(
            vec!["same", "same", "same", "new"],
            verbs(feed(lag, lag).match_batch(&statement(9)))
        );
    }
    assert_eq!("ambiguous", verbs(feed(4, 4).match_batch(&statement(9)))[2]);
    assert_eq!(
        vec!["same", "same", "ambiguous", "new"],
        verbs(feed(0, 2).match_batch(&statement(9)))
    );
    assert_eq!(
        vec!["same", "same", "ambiguous", "new"],
        verbs(feed(2, 2).match_batch(&statement(4)))
    );
}

#[test]
fn a_bank_row_days_from_two_like_amounts_asks() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            worded(synced(6, 1, "-3.50"), "CARD PAYMENT 4471"),
            worded(synced(7, 5, "-3.50"), "CARD PAYMENT 4471"),
        ],
        Vec::new(),
    );
    let outcome = matching.match_batch(&[
        priced(rows.statement(STATEMENT, 0), "-8.00"),
        rows.statement(STATEMENT, 3),
        priced(rows.statement(STATEMENT, 9), "-8.00"),
    ]);
    assert_eq!(vec!["new", "ambiguous", "new"], verbs(outcome));
}

fn shifted_feeds(once: i64) -> Vec<&'static str> {
    let mut rows = Rows::new();
    let mut ledger = vec![
        synced(3, 0, "-9.00"),
        worded(synced(1, 12, "-3.50"), "CARD PAYMENT 4471"),
        worded(synced(2, 14, "-3.50"), "CARD PAYMENT 4471"),
    ];
    let mut batch = vec![rows.statement(STATEMENT, 10), rows.statement(STATEMENT, 12)];
    for n in 0..once {
        let quantity = format!("-{}.00", 20 + n);
        ledger.push(worded(
            synced(10 + n as u128, n + 2, &quantity),
            "CARD PAYMENT 4471",
        ));
        batch.push(priced(rows.statement(STATEMENT, n), &quantity));
    }
    batch.push(priced(rows.statement(STATEMENT, 30), "-99.00"));
    let mut matching = LedgerMatching::new(ledger, Vec::new());
    verbs(matching.match_batch(&batch))
}

#[test]
fn a_steady_lag_between_two_feeds_is_read_from_the_amounts_that_occur_once() {
    assert_repeated(&[("same", 22), ("new", 1)], shifted_feeds(20));
    assert_repeated(
        &[("ambiguous", 2), ("same", 5), ("new", 1)],
        shifted_feeds(5),
    );
}

#[test]
fn a_steady_lag_keeps_a_statements_first_and_last_days_inside_the_feeds_dates() {
    let mut rows = Rows::new();
    let mut ledger = Vec::new();
    let mut batch = Vec::new();
    for n in 0..22 {
        let quantity = format!("-{}.00", 20 + n);
        ledger.push(worded(
            synced(10 + n as u128, n + 2, &quantity),
            "CARD PAYMENT 4471",
        ));
        batch.push(priced(rows.statement(STATEMENT, n), &quantity));
    }
    let mut matching = LedgerMatching::new(ledger, Vec::new());
    assert_repeated(&[("same", 22)], verbs(matching.match_batch(&batch)));
}

#[test]
fn a_typed_amount_is_asked_about_when_digits_of_the_bank_amount_are_swapped_left_out_or_added() {
    let mut rows = Rows::new();
    let mut read = |typed: &str, bank: &str| {
        let mut matching = LedgerMatching::new(vec![ledger_movement(7, 3, typed)], Vec::new());
        verbs(matching.match_batch(&[priced(rows.statement(STATEMENT, 4), bank)]))[0]
    };
    for (typed, bank) in [
        ("-72.18", "-27.18"),
        ("-40.36", "-40.63"),
        ("9.4", "9.04"),
        ("-680", "-860.00"),
        ("-20.0", "-2.00"),
        ("-10.50", "-1.50"),
        ("-15.5", "-155.00"),
        ("-12.34", "-123.4"),
    ] {
        assert_eq!("ambiguous", read(typed, bank), "{typed} for {bank}");
    }
    for (typed, bank) in [
        ("-72.18", "27.18"),
        ("-40.36", "-40.86"),
        ("-12.34", "-43.21"),
        ("-12.34", "-32.14"),
        ("-45.00", "-54.30"),
        ("-20.00", "-3.00"),
        ("-20.00", "2.00"),
    ] {
        assert_eq!("new", read(typed, bank), "{typed} for {bank}");
    }
}

#[test]
fn a_typed_amount_with_two_digits_swapped_is_offered_and_never_linked() {
    let mut rows = Rows::new();
    let typed = || LedgerMatching::new(vec![ledger_movement(7, 3, "-72.18")], Vec::new());
    let outcome = typed().match_batch(&[priced(rows.statement(STATEMENT, 4), "-27.18")]);
    let MatchOutcomeDto::Ambiguous { ranked } = &outcome[0] else {
        panic!("expected a question, got {outcome:?}");
    };
    assert_eq!(Uuid::from_u128(7), ranked[0].id);

    assert_eq!(
        vec!["same", "new"],
        verbs(typed().match_batch(&[
            priced(rows.statement(STATEMENT, 4), "-72.18"),
            priced(rows.statement(STATEMENT, 4), "-27.18"),
        ]))
    );
    assert_eq!(
        vec!["new", "new"],
        verbs(typed().match_batch(&[
            priced(rows.statement(STATEMENT, 3), "-27.18"),
            priced(rows.statement(STATEMENT, 4), "-27.18"),
        ]))
    );
    assert_eq!(
        vec!["new"],
        verbs(typed().match_batch(&[priced(rows.statement(STATEMENT, 8), "-27.18")]))
    );
    let mut synced = LedgerMatching::new(vec![synced(7, 3, "-72.18")], Vec::new());
    assert_eq!(
        vec!["new"],
        verbs(synced.match_batch(&[priced(rows.statement(STATEMENT, 4), "-27.18")]))
    );
}

#[test]
fn a_typed_amount_with_the_wrong_sign_is_offered_where_it_cannot_be_a_refund() {
    let mut rows = Rows::new();
    let typed = |when: i64, kind: DatabaseTransactionTypes| {
        let mut matching = LedgerMatching::new(vec![ledger_movement(7, when, "3.50")], Vec::new());
        matching.add_transaction_types(&of_type(7, kind));
        matching
    };
    let mut batch = || {
        vec![
            priced(rows.statement(STATEMENT, 3), "-8.00"),
            rows.statement(STATEMENT, 4),
        ]
    };
    let regular = DatabaseTransactionTypes::RegularTransaction;
    assert_eq!(
        vec!["new", "ambiguous"],
        verbs(typed(3, regular).match_batch(&batch()))
    );
    assert_eq!(
        vec!["new", "new"],
        verbs(typed(2, regular).match_batch(&batch()))
    );
    assert_eq!(
        vec!["new", "new"],
        verbs(typed(3, DatabaseTransactionTypes::CashTransferIn).match_batch(&batch()))
    );
}

fn worded_row(mut row: IncomingMovementDto, description: &str) -> IncomingMovementDto {
    row.description = Some(description.to_string());
    row
}

#[test]
fn a_rounded_candidate_never_moves_an_exact_match_onto_a_look_alike() {
    let mut rows = Rows::new();
    let typed = |entries: Vec<LedgerMovementDto>| LedgerMatching::new(entries, Vec::new());
    let fuel = || worded(ledger_movement(8, 11, "-20.00"), "fuel");

    let lunch = worded(ledger_movement(7, 10, "-20.00"), "lunch (cash)");
    let outcome = typed(vec![lunch, fuel()]).match_batch(&[
        worded_row(priced(rows.statement(STATEMENT, 12), "-20.00"), "SHELL"),
        worded_row(priced(rows.statement(STATEMENT, 14), "-19.71"), "ARGOS"),
    ]);
    let MatchOutcomeDto::Ambiguous { ranked } = &outcome[0] else {
        panic!("expected a question, got {outcome:?}");
    };
    assert_eq!(Uuid::from_u128(8), ranked[0].id);

    let outcome = typed(vec![fuel()]).match_batch(&[
        worded_row(priced(rows.statement(STATEMENT, 13), "-20.00"), "SHELL"),
        worded_row(priced(rows.statement(STATEMENT, 12), "-19.71"), "ARGOS"),
    ]);
    assert_eq!(vec![Uuid::from_u128(8)], linked_to(&outcome[0]));
    assert_eq!(MatchOutcomeDto::New, outcome[1]);
}

#[test]
fn a_typed_entry_one_row_links_is_offered_to_another_only_if_that_would_have_linked_too() {
    let mut rows = Rows::new();
    let typed = || LedgerMatching::new(vec![ledger_movement(7, 3, "-3.50")], Vec::new());
    assert_eq!(
        vec!["same", "new"],
        verbs(typed().match_batch(&[rows.statement(STATEMENT, 4), rows.statement(STATEMENT, 10)]))
    );
    assert_eq!(
        vec!["same", "ambiguous"],
        verbs(typed().match_batch(&[rows.statement(STATEMENT, 4), rows.statement(STATEMENT, 3)]))
    );
}

fn import(files: Vec<Vec<IncomingMovementDto>>) -> Vec<LedgerMatchBatchDto> {
    files
        .into_iter()
        .map(|incoming| LedgerMatchBatchDto {
            incoming,
            settled: Vec::new(),
        })
        .collect()
}

fn other_account(rows: &mut Rows, when: i64) -> IncomingMovementDto {
    rows.row(
        SourceKindDto::Statement,
        "statement:joint",
        when,
        Some(CASH_ACCOUNT),
    )
}

#[test]
fn a_typed_entry_its_own_account_states_is_not_offered_to_another_account_in_either_file_order() {
    let mut rows = Rows::new();
    let typed = || LedgerMatching::new(vec![ledger_movement(7, 3, "-3.50")], Vec::new());

    let files = import(vec![
        vec![other_account(&mut rows, 3)],
        vec![rows.statement(STATEMENT, 4)],
    ]);
    assert_eq!(
        vec![vec!["new"], vec!["same"]],
        typed()
            .match_batches(&files)
            .into_iter()
            .map(verbs)
            .collect::<Vec<_>>()
    );
    let reversed: Vec<LedgerMatchBatchDto> = files.into_iter().rev().collect();
    assert_eq!(
        vec![vec!["same"], vec!["new"]],
        typed()
            .match_batches(&reversed)
            .into_iter()
            .map(verbs)
            .collect::<Vec<_>>()
    );

    let mut pinned = import(vec![vec![other_account(&mut rows, 3)], Vec::new()]);
    pinned[1].settled = vec![settled(
        priced(rows.statement(STATEMENT, 4), "-3.62"),
        Some(7),
    )];
    assert_eq!(
        vec!["new"],
        verbs(typed().match_batches(&pinned).swap_remove(0))
    );

    let alone = import(vec![vec![other_account(&mut rows, 3)]]);
    assert_eq!(
        vec!["ambiguous"],
        verbs(typed().match_batches(&alone).swap_remove(0))
    );
}

#[test]
fn statements_of_two_accounts_never_describe_the_same_movement() {
    let mut rows = Rows::new();
    let files = import(vec![
        vec![other_account(&mut rows, 3), other_account(&mut rows, 4)],
        vec![rows.statement(STATEMENT, 3), rows.statement(STATEMENT, 4)],
    ]);
    assert_eq!(
        vec![vec!["new", "new"], vec!["new", "new"]],
        empty_ledger()
            .match_batches(&files)
            .into_iter()
            .map(verbs)
            .collect::<Vec<_>>()
    );
}

#[test]
fn identical_rows_on_two_days_each_take_the_typed_entry_nearest_them_and_add_the_rest() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            ledger_movement(7, 5, "-3.50"),
            ledger_movement(8, 8, "-3.50"),
        ],
        Vec::new(),
    );
    let batch: Vec<IncomingMovementDto> = [5, 5, 5, 5, 8, 8, 8, 8]
        .iter()
        .map(|&d| rows.statement(STATEMENT, d))
        .collect();
    let outcomes = matching.match_batch(&batch);
    for (cluster, entry) in outcomes.chunks(4).zip([7, 8]) {
        let links: Vec<&MatchOutcomeDto> = cluster
            .iter()
            .filter(|outcome| **outcome != MatchOutcomeDto::New)
            .collect();
        assert_eq!(1, links.len(), "{cluster:?}");
        assert_eq!(vec![Uuid::from_u128(entry)], linked_to(links[0]));
    }
}

#[test]
fn a_row_left_over_from_identical_ones_still_rivals_a_claim_no_better_than_its_own() {
    let mut rows = Rows::new();
    let mut matching = LedgerMatching::new(
        vec![
            worded(ledger_movement(7, 7, "-3.50"), "coffee"),
            worded(ledger_movement(8, 7, "-3.50"), "lunch"),
        ],
        Vec::new(),
    );
    let outcome = verbs(matching.match_batch(&[
        rows.statement(STATEMENT, 8),
        rows.statement(STATEMENT, 8),
        worded_row(rows.statement(STATEMENT, 8), "TESCO"),
    ]));
    assert_eq!("ambiguous", outcome[2], "{outcome:?}");
}

#[test]
fn a_transaction_with_no_entry_for_a_movement_of_the_row_that_matched_nothing_says_so() {
    let mut rows = Rows::new();
    let paid = priced(rows.statement(STATEMENT, 3), "-500.00");
    let received = IncomingMovementDto {
        account_id: Some(CASH_ACCOUNT),
        quantity: amount("500.00"),
        ..paid.clone()
    };
    let transfer = import(vec![vec![paid, received]]);
    let read = |ledger: Vec<LedgerMovementDto>| {
        let outcomes = LedgerMatching::new(ledger, Vec::new())
            .match_batches(&transfer)
            .swap_remove(0);
        let MatchOutcomeDto::Same { targets } = &outcomes[0] else {
            panic!("expected a link, got {outcomes:?}");
        };
        assert_eq!(MatchOutcomeDto::New, outcomes[1]);
        targets[0].leaves_movement_unrecorded
    };

    assert!(read(vec![synced(7, 3, "-500.00")]));

    let other_side = LedgerMovementDto {
        account_id: CASH_ACCOUNT,
        ..synced(7, 3, "480.00")
    };
    assert!(!read(vec![synced(7, 3, "-500.00"), other_side]));
}

#[test]
fn the_order_of_a_files_rows_does_not_change_what_each_row_reads() {
    let mut rows = Rows::new();
    let read = |ledger: Vec<LedgerMovementDto>, batch: Vec<IncomingMovementDto>| {
        let outcomes = verbs(LedgerMatching::new(ledger, Vec::new()).match_batch(&batch));
        let mut read: Vec<(Option<String>, Date, &str)> = batch
            .into_iter()
            .zip(outcomes)
            .map(|(row, verb)| (row.description, row.date, verb))
            .collect();
        read.sort();
        read
    };

    let feed = || {
        vec![
            synced(1, 0, "-9.00"),
            worded(synced(7, 5, "-3.50"), "CARD PAYMENT 4471"),
            synced(2, 20, "-9.00"),
        ]
    };
    let day_before = worded_row(rows.statement(STATEMENT, 4), "SHELL");
    let day_after = worded_row(rows.statement(STATEMENT, 6), "ESSO");
    let last = priced(rows.statement(STATEMENT, 9), "-8.00");
    assert_eq!(
        read(
            feed(),
            vec![day_before.clone(), day_after.clone(), last.clone()]
        ),
        read(feed(), vec![day_after, day_before, last])
    );

    let typed = || {
        vec![
            worded(ledger_movement(7, 5, "-3.50"), "coffee"),
            worded(ledger_movement(8, 5, "-3.50"), "sandwich"),
        ]
    };
    let costa = worded_row(rows.statement(STATEMENT, 5), "COSTA");
    let costa_again = worded_row(rows.statement(STATEMENT, 5), "COSTA");
    let greggs = worded_row(rows.statement(STATEMENT, 5), "GREGGS");
    assert_eq!(
        read(
            typed(),
            vec![costa.clone(), costa_again.clone(), greggs.clone()]
        ),
        read(typed(), vec![greggs, costa, costa_again])
    );
}

#[test]
fn a_purchase_typed_item_by_item_is_asked_about_as_its_parts() {
    let mut rows = Rows::new();
    let items = || {
        vec![
            ledger_movement(7, 3, "-20.00"),
            ledger_movement(8, 3, "-18.00"),
            ledger_movement(9, 3, "-12.00"),
        ]
    };
    let outcome = LedgerMatching::new(items(), Vec::new())
        .match_batch(&[priced(rows.statement(STATEMENT, 4), "-50.00")]);
    let MatchOutcomeDto::Parts { parts } = &outcome[0] else {
        panic!("expected parts, got {outcome:?}");
    };
    let ids: Vec<Uuid> = parts.iter().map(|part| part.id).collect();
    assert_eq!([7, 8, 9].map(Uuid::from_u128).to_vec(), ids);

    for (ledger, bank) in [
        (items(), "-49.00"),
        (
            vec![
                ledger_movement(7, 3, "-20.00"),
                ledger_movement(8, 2, "-30.00"),
            ],
            "-50.00",
        ),
        (items().into_iter().map(grouped).collect(), "-50.00"),
    ] {
        let mut matching = LedgerMatching::new(ledger, Vec::new());
        let verbs = verbs(matching.match_batch(&[priced(rows.statement(STATEMENT, 4), bank)]));
        assert_ne!(vec!["parts"], verbs, "{bank}");
    }
}
