use std::collections::{HashMap, HashSet};

use dal::enums::account_types::DatabaseAccountTypes;
use dal::models::import_models::ImportAccountMappingModel;
use uuid::Uuid;

use crate::dtos::imports::import_balance_dto::{ImportAssertionDto, ImportAssertionKindDto};
use crate::dtos::imports::import_classification_dto::{ImportLineTypeDto, ImportParsedLineDto};
use crate::dtos::imports::import_staging_dto::ImportLineClassificationDto;
use crate::dynamic_enums::account_types::AccountTypes;
use crate::dynamic_enums::DynamicEnum;

pub type Statement = (Uuid, String);

pub fn read_as_owed(
    lines: &[ImportParsedLineDto],
    assertions: &[ImportAssertionDto],
    mappings: &[ImportAccountMappingModel],
    account_types: &HashMap<Uuid, i32>,
) -> HashSet<Statement> {
    mappings
        .iter()
        .filter_map(|mapping| {
            let account_type = account_types.get(&mapping.account_id?)?;
            let rows_count = match AccountTypes::try_from_dynamic_enum(*account_type).ok()? {
                DatabaseAccountTypes::Credit => true,
                DatabaseAccountTypes::Mortgage | DatabaseAccountTypes::Loan => false,
                _ => return None,
            };
            let of_statement = |file_id: Uuid, account_ref: &str| {
                file_id == mapping.file_id && account_ref == mapping.account_ref
            };
            let balances: Vec<bool> = assertions
                .iter()
                .filter(|assertion| of_statement(assertion.file_id, &assertion.account_ref))
                .filter(|assertion| assertion.kind != ImportAssertionKindDto::ValueTotal)
                .filter(|assertion| !assertion.quantity.is_zero())
                .map(|assertion| assertion.quantity.is_sign_positive())
                .collect();
            let by_balances = (!balances.is_empty()).then(|| balances.iter().all(|above| *above));
            let (money_in, money_out) = lines
                .iter()
                .filter(|line| rows_count && of_statement(line.file_id, &line.account_ref))
                .flat_map(|line| &line.legs)
                .filter(|leg| leg.fee_type.is_none() && !leg.quantity.is_zero())
                .fold((0, 0), |(money_in, money_out), leg| {
                    if leg.quantity.is_sign_positive() {
                        (money_in + 1, money_out)
                    } else {
                        (money_in, money_out + 1)
                    }
                });
            let by_rows = (money_in != money_out).then_some(money_in > money_out);
            let evidence = [by_balances, by_rows];
            (evidence.contains(&Some(true)) && !evidence.contains(&Some(false)))
                .then(|| (mapping.file_id, mapping.account_ref.clone()))
        })
        .collect()
}

pub fn read_lines_as_owed(
    lines: &mut [ImportParsedLineDto],
    classifications: &mut [ImportLineClassificationDto],
    owed: &HashSet<Statement>,
) {
    let mut negated: HashSet<Uuid> = HashSet::new();
    for line in lines
        .iter_mut()
        .filter(|line| owed.contains(&(line.file_id, line.account_ref.clone())))
    {
        for leg in &mut line.legs {
            leg.quantity = -leg.quantity;
        }
        negated.insert(line.id);
    }
    for classification in classifications
        .iter_mut()
        .filter(|classification| negated.contains(&classification.parsed_line_id))
    {
        classification.line_type = match classification.line_type {
            ImportLineTypeDto::CashTransferIn => ImportLineTypeDto::CashTransferOut,
            ImportLineTypeDto::CashTransferOut => ImportLineTypeDto::CashTransferIn,
            ImportLineTypeDto::AssetTransferIn => ImportLineTypeDto::AssetTransferOut,
            ImportLineTypeDto::AssetTransferOut => ImportLineTypeDto::AssetTransferIn,
            other => other,
        };
    }
}

pub fn read_assertions_as_owed(assertions: &mut [ImportAssertionDto], owed: &HashSet<Statement>) {
    for assertion in assertions
        .iter_mut()
        .filter(|assertion| owed.contains(&(assertion.file_id, assertion.account_ref.clone())))
    {
        assertion.quantity = -assertion.quantity;
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use time::macros::{date, datetime};

    use super::*;
    use crate::dtos::imports::import_classification_dto::ImportParsedLegDto;

    const FILE: Uuid = Uuid::from_u128(1);
    const CARD: Uuid = Uuid::from_u128(2);
    const LOAN: Uuid = Uuid::from_u128(3);
    const CURRENT: Uuid = Uuid::from_u128(4);

    fn line(id: u128, account_ref: &str, quantity: Decimal) -> ImportParsedLineDto {
        ImportParsedLineDto {
            id: Uuid::from_u128(id),
            file_id: FILE,
            transaction_key: None,
            account_ref: account_ref.to_string(),
            date: datetime!(2026-03-02 0:00 UTC),
            description: None,
            related_asset_ref: None,
            legs: vec![ImportParsedLegDto {
                asset_ref: "GBP".to_string(),
                quantity,
                fee_type: None,
            }],
        }
    }

    fn balance(account_ref: &str, quantity: Decimal) -> ImportAssertionDto {
        ImportAssertionDto {
            file_id: FILE,
            account_ref: account_ref.to_string(),
            asset_ref: Some("GBP".to_string()),
            kind: ImportAssertionKindDto::Closing,
            date: date!(2026 - 03 - 31),
            quantity,
        }
    }

    fn owed(lines: &[ImportParsedLineDto], balances: &[ImportAssertionDto]) -> Vec<String> {
        crate::test_support::load_dynamic_enums();
        let mapping = |account_ref: &str, account_id: Uuid| ImportAccountMappingModel {
            file_id: FILE,
            account_ref: account_ref.to_string(),
            account_id: Some(account_id),
        };
        let mut owed: Vec<String> = read_as_owed(
            lines,
            balances,
            &[
                mapping("Card", CARD),
                mapping("Loan", LOAN),
                mapping("Current", CURRENT),
            ],
            &HashMap::from([(CARD, 4), (LOAN, 8), (CURRENT, 1)]),
        )
        .into_iter()
        .map(|(_, account_ref)| account_ref)
        .collect();
        owed.sort();
        owed
    }

    #[test]
    fn a_card_statement_printing_what_is_owed_as_positive_is_read_the_other_way() {
        let purchases = || {
            vec![
                line(1, "Card", dec!(12)),
                line(2, "Card", dec!(30)),
                line(3, "Card", dec!(-50)),
            ]
        };
        assert_eq!(owed(&purchases(), &[balance("Card", dec!(842))]), ["Card"]);
        assert_eq!(owed(&purchases(), &[]), ["Card"]);
        assert_eq!(owed(&[], &[balance("Card", dec!(842))]), ["Card"]);
    }

    #[test]
    fn a_card_in_credit_or_printed_as_owed_stays_as_read() {
        let purchases = vec![
            line(1, "Card", dec!(-12)),
            line(2, "Card", dec!(-30)),
            line(3, "Card", dec!(50)),
        ];
        assert!(owed(&purchases, &[balance("Card", dec!(8))]).is_empty());
        assert!(owed(&purchases, &[balance("Card", dec!(-842))]).is_empty());
        assert!(owed(&[], &[]).is_empty());
    }

    #[test]
    fn a_loan_is_read_by_its_balances_alone_and_any_other_account_never() {
        let payments = vec![line(1, "Loan", dec!(300)), line(2, "Loan", dec!(300))];
        assert!(owed(&payments, &[balance("Loan", dec!(-9000))]).is_empty());
        assert!(owed(&payments, &[]).is_empty());
        assert_eq!(owed(&payments, &[balance("Loan", dec!(9000))]), ["Loan"]);

        let income = vec![line(1, "Current", dec!(2000))];
        assert!(owed(&income, &[balance("Current", dec!(2400))]).is_empty());
    }

    #[test]
    fn reading_a_statement_as_owed_negates_it_and_turns_its_transfers_around() {
        let mut lines = vec![line(1, "Card", dec!(50)), line(2, "Current", dec!(50))];
        let classification = |id: u128| ImportLineClassificationDto {
            parsed_line_id: Uuid::from_u128(id),
            line_type: ImportLineTypeDto::CashTransferOut,
            category_id: None,
        };
        let mut classifications = vec![classification(1), classification(2)];
        let mut balances = vec![balance("Card", dec!(842)), balance("Current", dec!(10))];
        let owed = HashSet::from([(FILE, "Card".to_string())]);

        read_lines_as_owed(&mut lines, &mut classifications, &owed);
        read_assertions_as_owed(&mut balances, &owed);

        assert_eq!(
            lines
                .iter()
                .map(|line| line.legs[0].quantity)
                .collect::<Vec<_>>(),
            [dec!(-50), dec!(50)]
        );
        assert_eq!(
            classifications
                .iter()
                .map(|classification| classification.line_type)
                .collect::<Vec<_>>(),
            [
                ImportLineTypeDto::CashTransferIn,
                ImportLineTypeDto::CashTransferOut
            ]
        );
        assert_eq!(
            balances
                .iter()
                .map(|balance| balance.quantity)
                .collect::<Vec<_>>(),
            [dec!(-842), dec!(10)]
        );
    }
}
