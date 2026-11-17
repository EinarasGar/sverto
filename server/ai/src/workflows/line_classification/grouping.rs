use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::models::classification::{
    ClassificationGroup, ClassificationLine, GroupClassification, LineClassification,
    MoneyDirection,
};

pub(super) fn group(
    lines: &[ClassificationLine],
) -> (Vec<ClassificationGroup>, Vec<Option<usize>>) {
    let mut members: Vec<Vec<&ClassificationLine>> = Vec::new();
    let mut by_key: HashMap<(String, MoneyDirection), usize> = HashMap::new();
    let mut line_groups = vec![None; lines.len()];
    for (at, line) in lines.iter().enumerate() {
        let Some(description) = line
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
        else {
            continue;
        };
        let index = *by_key
            .entry((description.to_lowercase(), direction(line.amount)))
            .or_insert_with(|| {
                members.push(Vec::new());
                members.len() - 1
            });
        members[index].push(line);
        line_groups[at] = Some(index);
    }
    let mut groups: Vec<ClassificationGroup> = members
        .iter()
        .enumerate()
        .map(|(index, lines)| summary(index as u32, lines))
        .collect();
    groups.sort_by_cached_key(|group| without_digits(&group.description));
    (groups, line_groups)
}

pub(super) fn per_line(
    line_groups: &[Option<usize>],
    answers: Vec<GroupClassification>,
) -> Vec<Option<LineClassification>> {
    let answers: HashMap<usize, LineClassification> = answers
        .into_iter()
        .map(|answer| (answer.index as usize, answer.classification))
        .collect();
    line_groups
        .iter()
        .map(|group| group.and_then(|group| answers.get(&group).cloned()))
        .collect()
}

fn without_digits(description: &str) -> String {
    let lowercase = description.to_lowercase();
    let words: Vec<&str> = lowercase
        .split(|character: char| character.is_numeric() || character.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect();
    if words
        .iter()
        .any(|word| word.chars().any(char::is_alphabetic))
    {
        words.join(" ")
    } else {
        lowercase
    }
}

fn summary(index: u32, lines: &[&ClassificationLine]) -> ClassificationGroup {
    let mut amounts: Vec<(Decimal, &str)> = lines
        .iter()
        .map(|line| (line.amount.abs(), line.currency.as_str()))
        .collect();
    amounts.sort();
    let (amount, currency) = amounts[(amounts.len() - 1) / 2];
    ClassificationGroup {
        index,
        description: lines[0]
            .description
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
        direction: direction(lines[0].amount),
        typical_amount: format!("{} {currency}", amount.normalize()),
        times_seen: lines.len() as u32,
        account: lines[0].account.clone(),
        includes_a_fee: lines.iter().any(|line| line.has_fee),
    }
}

fn direction(amount: Decimal) -> MoneyDirection {
    if amount < Decimal::ZERO {
        MoneyDirection::Out
    } else {
        MoneyDirection::In
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(description: Option<&str>, amount: &str, has_fee: bool) -> ClassificationLine {
        ClassificationLine {
            description: description.map(str::to_string),
            amount: amount.parse().unwrap(),
            currency: "GBP".to_string(),
            account: "Current".to_string(),
            has_fee,
        }
    }

    #[test]
    fn lines_with_the_same_description_and_direction_form_one_group() {
        let lines = [
            line(Some("TESCO STORES 3297"), "-12.40", false),
            line(Some(" tesco stores 3297"), "-3.10", true),
            line(Some("TESCO STORES 3297"), "-30.00", false),
            line(Some("TESCO STORES 3297"), "5.00", false),
            line(Some("TESCO STORES 1120"), "-8.00", false),
        ];

        let (groups, line_groups) = group(&lines);

        assert_eq!(
            groups,
            vec![
                ClassificationGroup {
                    index: 0,
                    description: "TESCO STORES 3297".to_string(),
                    direction: MoneyDirection::Out,
                    typical_amount: "12.4 GBP".to_string(),
                    times_seen: 3,
                    account: "Current".to_string(),
                    includes_a_fee: true,
                },
                ClassificationGroup {
                    index: 1,
                    description: "TESCO STORES 3297".to_string(),
                    direction: MoneyDirection::In,
                    typical_amount: "5 GBP".to_string(),
                    times_seen: 1,
                    account: "Current".to_string(),
                    includes_a_fee: false,
                },
                ClassificationGroup {
                    index: 2,
                    description: "TESCO STORES 1120".to_string(),
                    direction: MoneyDirection::Out,
                    typical_amount: "8 GBP".to_string(),
                    times_seen: 1,
                    account: "Current".to_string(),
                    includes_a_fee: false,
                },
            ]
        );
        assert_eq!(
            line_groups,
            vec![Some(0), Some(0), Some(0), Some(1), Some(2)]
        );
    }

    #[test]
    fn lines_without_a_description_are_not_sent() {
        let lines = [
            line(Some("12345678"), "-10", false),
            line(Some("87654321"), "-20", false),
            line(None, "-30", false),
            line(Some("  "), "-40", false),
        ];

        let (groups, line_groups) = group(&lines);

        assert_eq!(
            groups
                .iter()
                .map(|group| group.description.as_str())
                .collect::<Vec<_>>(),
            vec!["12345678", "87654321"]
        );
        assert_eq!(line_groups, vec![Some(0), Some(1), None, None]);
    }

    #[test]
    fn spellings_of_one_payee_are_sent_side_by_side() {
        let lines = [
            line(Some("TESCO STORES 3297"), "-12.40", false),
            line(Some("SHELL 0412"), "-40", false),
            line(Some("Tesco  Stores 1120 03AUG"), "-8", false),
            line(Some("100045"), "-75", false),
        ];

        let (groups, line_groups) = group(&lines);

        assert_eq!(
            groups
                .iter()
                .map(|group| (group.index, group.description.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (3, "100045"),
                (1, "SHELL 0412"),
                (0, "TESCO STORES 3297"),
                (2, "Tesco  Stores 1120 03AUG"),
            ]
        );
        assert_eq!(line_groups, vec![Some(0), Some(1), Some(2), Some(3)]);
    }
}
