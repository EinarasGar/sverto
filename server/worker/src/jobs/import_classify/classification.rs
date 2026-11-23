use std::collections::{BTreeMap, BTreeSet, HashMap};

use ai::models::classification::{ClassificationCategory, ClassificationLine, LineClassification};
use business::dtos::categories::category_dto::CategoryDto;
use business::dtos::imports::import_classification_dto::{
    ImportLineTypeDto, ImportLineTypingDto, ImportParsedLegDto, ImportParsedLineDto,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::jobs::line_classification::{offered_categories, typing};

pub struct FileClassification {
    lines: Vec<ImportParsedLineDto>,
    categories: Vec<CategoryDto>,
    classifications: HashMap<Uuid, LineClassification>,
}

impl FileClassification {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            categories: Vec::new(),
            classifications: HashMap::new(),
        }
    }

    pub fn add_lines(&mut self, lines: Vec<ImportParsedLineDto>) {
        self.lines.extend(lines);
    }

    pub fn add_categories(&mut self, categories: Vec<CategoryDto>) {
        self.categories.extend(categories);
    }

    pub fn add_classifications(
        &mut self,
        line_ids: &[Uuid],
        classifications: Vec<Option<LineClassification>>,
    ) {
        self.classifications.extend(
            line_ids
                .iter()
                .zip(classifications)
                .filter_map(|(line_id, classification)| Some((*line_id, classification?))),
        );
    }

    pub fn offered_categories(&self) -> Vec<ClassificationCategory> {
        offered_categories(&self.categories)
    }

    pub fn classification_lines(&self) -> Vec<(Uuid, ClassificationLine)> {
        let mut sides: HashMap<(Uuid, &str, &str), Vec<&ImportParsedLineDto>> = HashMap::new();
        for line in &self.lines {
            if let Some(key) = line.transaction_key.as_deref() {
                sides
                    .entry((line.source_id, key, line.account_ref.as_str()))
                    .or_default()
                    .push(line);
            }
        }
        self.lines
            .iter()
            .filter_map(|line| {
                let legs = main_legs(line);
                let asset_ref = legs.first()?.asset_ref.as_str();
                let amount = total(&legs);
                if legs.iter().any(|leg| leg.asset_ref != asset_ref)
                    || (line.related_asset_ref.is_some() && amount > Decimal::ZERO)
                {
                    return None;
                }
                if let Some(key) = line.transaction_key.as_deref() {
                    let others: BTreeSet<&str> = sides
                        [&(line.source_id, key, line.account_ref.as_str())]
                        .iter()
                        .filter(|other| other.id != line.id)
                        .flat_map(|other| main_legs(other))
                        .map(|leg| leg.asset_ref.as_str())
                        .collect();
                    if others.len() == 1 && !others.contains(asset_ref) {
                        return None;
                    }
                }
                Some((
                    line.id,
                    ClassificationLine {
                        description: line.description.clone(),
                        amount,
                        currency: asset_ref.to_string(),
                        account: line.account_ref.clone(),
                        has_fee: line.legs.iter().any(|leg| leg.fee_type.is_some()),
                    },
                ))
            })
            .collect()
    }

    pub fn typings(&self) -> Vec<ImportLineTypingDto> {
        let mut grouped: BTreeMap<(ImportLineTypeDto, Option<i32>), ImportLineTypingDto> =
            BTreeMap::new();
        for line in &self.lines {
            let Some(classification) = self.classifications.get(&line.id) else {
                continue;
            };
            let (line_type, category_id) =
                typing(classification, total(&main_legs(line)) > Decimal::ZERO);
            grouped
                .entry((line_type, category_id))
                .or_insert_with(|| ImportLineTypingDto {
                    line_type,
                    category_id,
                    line_ids: Vec::new(),
                })
                .line_ids
                .push(line.id);
        }
        grouped.into_values().collect()
    }

    pub fn uncategorized_typings(&self) -> Vec<ImportLineTypingDto> {
        vec![ImportLineTypingDto {
            line_type: ImportLineTypeDto::Regular,
            category_id: None,
            line_ids: self
                .classification_lines()
                .into_iter()
                .map(|(line_id, _)| line_id)
                .collect(),
        }]
    }
}

fn main_legs(line: &ImportParsedLineDto) -> Vec<&ImportParsedLegDto> {
    line.legs
        .iter()
        .filter(|leg| leg.fee_type.is_none())
        .collect()
}

fn total(legs: &[&ImportParsedLegDto]) -> Decimal {
    legs.iter().map(|leg| leg.quantity).sum()
}

#[cfg(test)]
mod tests {
    use ai::models::classification::ClassifiedKind;
    use business::dtos::fee_entry_types_dto::FeeEntryTypesDto;

    use super::*;

    const GROCERIES: i32 = 25;

    fn line(description: &str, key: Option<&str>) -> ImportParsedLineDto {
        ImportParsedLineDto {
            id: Uuid::new_v4(),
            source_id: Uuid::nil(),
            transaction_key: key.map(str::to_string),
            account_ref: "Current".to_string(),
            date: time::OffsetDateTime::UNIX_EPOCH,
            description: Some(description.to_string()),
            related_asset_ref: None,
            external_id: None,
            legs: Vec::new(),
        }
    }

    fn leg(
        line: &ImportParsedLineDto,
        asset_ref: &str,
        quantity: &str,
    ) -> (Uuid, ImportParsedLegDto) {
        (
            line.id,
            ImportParsedLegDto {
                asset_ref: asset_ref.to_string(),
                quantity: quantity.parse().unwrap(),
                fee_type: None,
            },
        )
    }

    fn fee(line: &ImportParsedLineDto, quantity: &str) -> (Uuid, ImportParsedLegDto) {
        let (line_id, leg) = leg(line, "GBP", quantity);
        (
            line_id,
            ImportParsedLegDto {
                fee_type: Some(FeeEntryTypesDto::Transaction),
                ..leg
            },
        )
    }

    fn classified(kind: ClassifiedKind, confident: bool) -> Option<LineClassification> {
        Some(LineClassification {
            kind,
            category_id: Some(GROCERIES),
            confidence: if confident { 1.0 } else { 0.3 },
            confident,
        })
    }

    fn answer_lines(
        classification: &mut FileClassification,
        answers: Vec<Option<LineClassification>>,
    ) {
        let line_ids = asked(classification);
        classification.add_classifications(&line_ids, answers);
    }

    fn asked(classification: &FileClassification) -> Vec<Uuid> {
        classification
            .classification_lines()
            .into_iter()
            .map(|(line_id, _)| line_id)
            .collect()
    }

    fn classification(
        lines: &[ImportParsedLineDto],
        legs: Vec<(Uuid, ImportParsedLegDto)>,
    ) -> FileClassification {
        let mut lines = lines.to_vec();
        for (line_id, leg) in legs {
            if let Some(line) = lines.iter_mut().find(|line| line.id == line_id) {
                line.legs.push(leg);
            }
        }
        let mut classification = FileClassification::new();
        classification.add_lines(lines);
        classification
    }

    #[test]
    fn single_asset_lines_are_handed_over_as_plain_facts() {
        let lines = [line("TESCO STORES 3297", None), line("Buy VUSA", None)];
        let legs = vec![
            leg(&lines[0], "GBP", "-12.40"),
            fee(&lines[0], "-0.50"),
            leg(&lines[1], "GBP", "-550"),
            leg(&lines[1], "VUSA", "3"),
        ];

        assert_eq!(
            classification(&lines, legs).classification_lines(),
            vec![(
                lines[0].id,
                ClassificationLine {
                    description: Some("TESCO STORES 3297".to_string()),
                    amount: "-12.40".parse().unwrap(),
                    currency: "GBP".to_string(),
                    account: "Current".to_string(),
                    has_fee: true,
                }
            )]
        );
    }

    #[test]
    fn a_fee_in_another_asset_does_not_stop_a_line_being_asked_about() {
        let lines = [line("Card payment", None)];
        let mut eur_fee = fee(&lines[0], "-1");
        eur_fee.1.asset_ref = "EUR".to_string();
        let legs = vec![leg(&lines[0], "GBP", "-12"), eur_fee];

        assert_eq!(asked(&classification(&lines, legs)), vec![lines[0].id]);
    }

    #[test]
    fn lines_the_legs_decide_are_not_asked_about() {
        let mut lines = [
            line("Swap", None),
            line("Exchanged to EUR", None),
            line("Dividend", None),
            line("Fee", None),
            line("Dividend reversal", None),
        ];
        lines[2].related_asset_ref = Some("VUSA".to_string());
        lines[4].related_asset_ref = Some("VUSA".to_string());
        let legs = vec![
            leg(&lines[0], "VUSA", "-1"),
            leg(&lines[0], "VWRL", "2"),
            leg(&lines[1], "GBP", "-100"),
            leg(&lines[1], "EUR", "115"),
            leg(&lines[2], "GBP", "4.20"),
            fee(&lines[3], "-2"),
            leg(&lines[4], "GBP", "-4.20"),
        ];

        assert_eq!(asked(&classification(&lines, legs)), vec![lines[4].id]);
    }

    #[test]
    fn inside_a_trades_key_only_a_line_that_may_be_a_fee_is_asked_about() {
        let mut lines = [
            line("Buy", Some("t1")),
            line("Transaction Related", Some("t1")),
            line("Buy VUSA", Some("t2")),
            line("Commission", Some("t2")),
            line("Shares to the other account", Some("t3")),
            line("Paid from the other account", Some("t3")),
        ];
        lines[5].account_ref = "Savings".to_string();
        let legs = vec![
            leg(&lines[0], "BTC", "0.01"),
            leg(&lines[1], "GBP", "-550"),
            leg(&lines[2], "GBP", "-550"),
            leg(&lines[2], "VUSA", "3"),
            leg(&lines[3], "GBP", "-5"),
            leg(&lines[4], "VUSA", "3"),
            leg(&lines[5], "GBP", "-550"),
        ];

        assert_eq!(
            asked(&classification(&lines, legs)),
            vec![lines[3].id, lines[4].id, lines[5].id]
        );
    }

    #[test]
    fn each_line_keeps_its_own_answer_and_only_a_confident_payment_keeps_its_category() {
        let lines = [
            line("Tesco", Some("t1")),
            line("Bank charge", Some("t1")),
            line("Corner shop", None),
            line("Salary", None),
            line("Fee refund", None),
        ];
        let legs = vec![
            leg(&lines[0], "GBP", "-12"),
            leg(&lines[1], "GBP", "-0.41"),
            leg(&lines[2], "GBP", "-3"),
            leg(&lines[3], "GBP", "2000"),
            leg(&lines[4], "GBP", "5"),
        ];
        let mut classification = classification(&lines, legs);
        answer_lines(
            &mut classification,
            vec![
                classified(ClassifiedKind::Expense, true),
                classified(ClassifiedKind::BankFee, true),
                classified(ClassifiedKind::Expense, false),
                classified(ClassifiedKind::TransferIn, true),
                classified(ClassifiedKind::BankFee, true),
            ],
        );

        assert_eq!(
            classification.typings(),
            vec![
                ImportLineTypingDto {
                    line_type: ImportLineTypeDto::Regular,
                    category_id: None,
                    line_ids: vec![lines[2].id],
                },
                ImportLineTypingDto {
                    line_type: ImportLineTypeDto::Regular,
                    category_id: Some(GROCERIES),
                    line_ids: vec![lines[0].id, lines[4].id],
                },
                ImportLineTypingDto {
                    line_type: ImportLineTypeDto::CashTransferIn,
                    category_id: None,
                    line_ids: vec![lines[3].id],
                },
                ImportLineTypingDto {
                    line_type: ImportLineTypeDto::AccountFees,
                    category_id: None,
                    line_ids: vec![lines[1].id],
                },
            ]
        );
    }

    #[test]
    fn without_ai_every_line_asked_about_is_a_payment_without_a_category() {
        let lines = [
            line("TESCO STORES 3297", None),
            line("Salary", None),
            line("Buy VUSA", None),
        ];
        let legs = vec![
            leg(&lines[0], "GBP", "-12.40"),
            leg(&lines[1], "GBP", "2000"),
            leg(&lines[2], "GBP", "-550"),
            leg(&lines[2], "VUSA", "3"),
        ];

        assert_eq!(
            classification(&lines, legs).uncategorized_typings(),
            vec![ImportLineTypingDto {
                line_type: ImportLineTypeDto::Regular,
                category_id: None,
                line_ids: vec![lines[0].id, lines[1].id],
            }]
        );
    }

    #[test]
    fn the_same_transaction_key_in_two_files_is_two_transactions() {
        let mut other_source = line("Coffee", Some("t1"));
        other_source.source_id = Uuid::new_v4();
        let lines = [line("Buy", Some("t1")), other_source];
        let legs = vec![leg(&lines[0], "VUSA", "3"), leg(&lines[1], "GBP", "-3")];

        assert_eq!(
            asked(&classification(&lines, legs)),
            vec![lines[0].id, lines[1].id]
        );
    }
}
