use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;
use time::{Date, Duration};

use crate::models::accepted::ExtractedAssertion;
use crate::models::finding::{BalancePosition, Finding};
use crate::models::output::{BalanceKind, ExtractedLine, ExtractionOutput};

pub type BalanceGroup = (String, String);

pub struct BalanceCheck {
    pub findings: Vec<Finding>,
    pub derived: Vec<ExtractedAssertion>,
    pub end_of_day: Vec<ExtractedAssertion>,
    pub read_mid_day: BTreeSet<usize>,
    pub redated: BTreeMap<usize, Date>,
    pub evidenced: BTreeSet<BalanceGroup>,
}

#[derive(Clone, Copy)]
struct Step {
    line: usize,
    date: Date,
    net: Decimal,
    printed: Option<Decimal>,
    before: Option<Decimal>,
}

struct Walk {
    opening: Decimal,
    closing: Decimal,
    days: Vec<(Date, Decimal)>,
}

struct Break {
    line: usize,
    expected: Decimal,
    printed: Decimal,
    progress: usize,
}

pub fn check(output: &ExtractionOutput) -> BalanceCheck {
    let mut findings = Vec::new();
    let mut derived = Vec::new();
    let mut end_of_day_balances = Vec::new();
    let mut evidenced = BTreeSet::new();
    let mut intraday: BTreeMap<BalanceGroup, BTreeSet<(Date, Decimal)>> = BTreeMap::new();

    let chain_groups: BTreeSet<BalanceGroup> = output
        .lines
        .iter()
        .flat_map(|line| {
            line.balances
                .iter()
                .map(|b| (line.account_ref.clone(), b.asset_ref.clone()))
        })
        .collect();
    for group in chain_groups {
        let steps = steps(&output.lines, &group);
        let walked = walk_in_any_order(&steps).or_else(|broken| {
            let printed_before: Vec<Step> = steps
                .iter()
                .map(|step| Step {
                    before: step.printed,
                    ..*step
                })
                .collect();
            match walk_in_any_order(&printed_before) {
                Ok(Some(walk)) if walk.days.len() > 1 => Ok(Some(walk)),
                _ => Err(broken),
            }
        });
        match walked {
            Ok(Some(walk)) => {
                let first = steps.iter().map(|s| s.date).min();
                let last = steps.iter().map(|s| s.date).max();
                if let (Some(first), Some(last)) = (first, last) {
                    derived.push(assertion(&group, BalanceKind::Opening, first, walk.opening));
                    derived.push(assertion(&group, BalanceKind::Closing, last, walk.closing));
                }
                end_of_day_balances.extend(walk.days.iter().map(|(date, quantity)| {
                    assertion(&group, BalanceKind::Snapshot, *date, *quantity)
                }));
                intraday.insert(
                    group.clone(),
                    steps
                        .iter()
                        .filter_map(|s| s.printed.map(|p| (s.date, p)))
                        .collect(),
                );
                evidenced.insert(group);
            }
            Ok(None) => {}
            Err(broken) => findings.push(Finding::BalanceChainBroken {
                account_ref: group.0.clone(),
                asset_ref: group.1.clone(),
                line: broken.line,
                expected: broken.expected,
                printed: broken.printed,
            }),
        }
    }

    let mut positions: BTreeMap<BalanceGroup, Vec<(Date, BalancePosition)>> = BTreeMap::new();
    let mut read_mid_day: BTreeSet<usize> = output
        .balances
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            let group = (b.account_ref.clone(), b.asset_ref.clone());
            b.kind == BalanceKind::Snapshot
                && intraday
                    .get(&group)
                    .is_some_and(|printed| printed.contains(&(b.as_of_date, b.quantity)))
        })
        .map(|(index, _)| index)
        .collect();
    let stated = output
        .balances
        .iter()
        .enumerate()
        .filter(|(index, b)| b.kind != BalanceKind::ValueTotal && !read_mid_day.contains(index))
        .map(|(index, b)| {
            let group = (b.account_ref.clone(), b.asset_ref.clone());
            (group, Some(index), b.kind, b.as_of_date, b.quantity)
        });
    let inferred = derived.iter().map(|a| {
        let group = (a.account_ref.clone(), a.asset_ref.clone());
        (group, None, a.kind, a.as_of_date, a.quantity)
    });
    for (group, balance, kind, as_of_date, quantity) in stated.chain(inferred) {
        positions.entry(group).or_default().push((
            end_of_day(kind, as_of_date),
            BalancePosition {
                balance,
                kind,
                as_of_date,
                quantity,
            },
        ));
    }

    let mut redated = BTreeMap::new();
    for (group, mut list) in positions {
        list.sort_by_key(|(end_of_day, _)| *end_of_day);
        let (mut disagreeing, mut where_printed) = disagreements(output, &group, &list);
        let moved: Vec<Date> = output
            .lines
            .iter()
            .filter(|line| line.account_ref == group.0)
            .filter(|line| line.legs.iter().any(|leg| leg.asset_ref == group.1))
            .map(|line| line.date)
            .collect();
        if let (false, Some(first), Some(last)) = (
            disagreeing.is_empty(),
            moved.iter().min(),
            moved.iter().max(),
        ) {
            let mut bracketed = list.clone();
            let mut brackets = BTreeMap::new();
            let stated = |(_, position): &&mut (Date, BalancePosition)| position.balance.is_some();
            if let Some((end_of_day, position)) =
                bracketed
                    .iter_mut()
                    .find(stated)
                    .filter(|(end_of_day, position)| {
                        position.kind == BalanceKind::Opening && end_of_day >= first
                    })
            {
                *end_of_day = *first - Duration::days(1);
                brackets.extend(position.balance.map(|balance| (balance, Some(*end_of_day))));
            }
            if let Some((end_of_day, position)) =
                bracketed
                    .iter_mut()
                    .rev()
                    .find(stated)
                    .filter(|(end_of_day, position)| {
                        position.kind == BalanceKind::Closing && end_of_day < last
                    })
            {
                *end_of_day = *last;
                brackets.extend(position.balance.map(|balance| (balance, Some(*end_of_day))));
            }
            bracketed.sort_by_key(|(end_of_day, _)| *end_of_day);
            let (still_disagreeing, bracketed_where_printed) =
                disagreements(output, &group, &bracketed);
            if still_disagreeing.is_empty() {
                disagreeing.clear();
                where_printed = bracketed_where_printed;
                where_printed.extend(brackets);
            }
        }
        findings.extend(disagreeing);
        for (balance, end_of_day) in where_printed {
            match end_of_day {
                Some(end_of_day) => {
                    let kind = output.balances[balance].kind;
                    let opens_next_day = Duration::days(i64::from(kind == BalanceKind::Opening));
                    redated.insert(balance, end_of_day + opens_next_day);
                }
                None => {
                    read_mid_day.insert(balance);
                }
            }
        }
        if list.len() > 1 {
            evidenced.insert(group);
        }
    }

    BalanceCheck {
        findings,
        derived,
        end_of_day: end_of_day_balances,
        read_mid_day,
        redated,
        evidenced,
    }
}

pub fn end_of_day(kind: BalanceKind, as_of_date: Date) -> Date {
    if kind == BalanceKind::Opening {
        as_of_date - Duration::days(1)
    } else {
        as_of_date
    }
}

pub fn net(line: &ExtractedLine, asset_ref: &str) -> Decimal {
    line.legs
        .iter()
        .filter(|leg| leg.asset_ref == asset_ref)
        .map(|leg| leg.quantity)
        .sum()
}

fn disagreements(
    output: &ExtractionOutput,
    group: &BalanceGroup,
    positions: &[(Date, BalancePosition)],
) -> (Vec<Finding>, BTreeMap<usize, Option<Date>>) {
    let lines: Vec<&ExtractedLine> = output
        .lines
        .iter()
        .filter(|line| line.account_ref == group.0)
        .filter(|line| line.legs.iter().any(|leg| leg.asset_ref == group.1))
        .collect();
    let moved = |comes_after: &dyn Fn(&ExtractedLine) -> bool| -> Decimal {
        lines
            .iter()
            .filter(|line| comes_after(line))
            .map(|line| net(line, &group.1))
            .sum()
    };
    let stated = |(_, position): &(Date, BalancePosition)| position.balance.is_some();
    let first_stated = positions.iter().position(stated);
    let last_stated = positions.iter().rposition(stated);
    let mut findings = Vec::new();
    let mut where_printed = BTreeMap::new();
    let mut previous: Option<(&BalancePosition, Decimal)> = None;
    for (index, (end_of_day, position)) in positions.iter().enumerate() {
        let mut moved_before = moved(&|line| line.date <= *end_of_day);
        if let Some((earlier, moved_before_earlier)) = previous {
            let expected =
                |moved_before: Decimal| earlier.quantity + moved_before - moved_before_earlier;
            if expected(moved_before) != position.quantity {
                let inside = match position.kind {
                    BalanceKind::Opening => Some(index) != first_stated,
                    BalanceKind::Closing => Some(index) != last_stated,
                    BalanceKind::Snapshot | BalanceKind::ValueTotal => true,
                };
                let printed_at = position.balance.filter(|_| inside).and_then(|balance| {
                    Some((balance, *output.balances[balance].refs.iter().min()?))
                });
                let read_there = printed_at.and_then(|(balance, record)| {
                    [true, false].into_iter().find_map(|printed_before| {
                        let counted = &|line: &ExtractedLine| {
                            let first_record = line.refs.iter().min().copied();
                            if printed_before {
                                first_record < Some(record)
                            } else {
                                first_record > Some(record)
                            }
                        };
                        let moved_before = moved(counted);
                        let count = lines.iter().filter(|line| counted(line)).count();
                        let dates = lines.iter().map(|line| line.date);
                        let end_of_day = if count == lines.len() {
                            dates.max()
                        } else if count == 0 {
                            dates.min().map(|first| first - Duration::days(1))
                        } else {
                            None
                        };
                        (expected(moved_before) == position.quantity).then_some((
                            balance,
                            moved_before,
                            end_of_day,
                        ))
                    })
                });
                match read_there {
                    Some((balance, moved_before_record, end_of_day)) => {
                        moved_before = moved_before_record;
                        where_printed.insert(balance, end_of_day);
                    }
                    None => findings.push(Finding::BalancesDisagree {
                        account_ref: group.0.clone(),
                        asset_ref: group.1.clone(),
                        earlier: earlier.clone(),
                        later: position.clone(),
                        expected: expected(moved_before),
                    }),
                }
            }
        }
        previous = Some((position, moved_before));
    }
    (findings, where_printed)
}

fn assertion(
    group: &BalanceGroup,
    kind: BalanceKind,
    as_of_date: Date,
    quantity: Decimal,
) -> ExtractedAssertion {
    ExtractedAssertion {
        kind,
        account_ref: group.0.clone(),
        asset_ref: group.1.clone(),
        as_of_date,
        quantity,
    }
}

fn steps(lines: &[ExtractedLine], (account_ref, asset_ref): &BalanceGroup) -> Vec<Step> {
    let mut members: Vec<(usize, &ExtractedLine)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| &line.account_ref == account_ref)
        .filter(|(_, line)| {
            line.legs.iter().any(|leg| &leg.asset_ref == asset_ref)
                || line.balances.iter().any(|b| &b.asset_ref == asset_ref)
        })
        .collect();
    members.sort_by_key(|(_, line)| line.refs.iter().min().copied());
    members
        .into_iter()
        .map(|(index, line)| {
            let net = net(line, asset_ref);
            let printed = line
                .balances
                .iter()
                .find(|b| &b.asset_ref == asset_ref)
                .map(|b| b.quantity);
            Step {
                line: index,
                date: line.date,
                net,
                printed,
                before: printed.map(|printed| printed - net),
            }
        })
        .collect()
}

fn walk_in_any_order(steps: &[Step]) -> Result<Option<Walk>, Break> {
    let forward: Vec<&Step> = steps.iter().collect();
    let backward: Vec<&Step> = steps.iter().rev().collect();
    let mut by_date = forward.clone();
    by_date.sort_by_key(|step| step.date);
    let mut by_date_from_the_end = backward.clone();
    by_date_from_the_end.sort_by_key(|step| step.date);

    let mut furthest = match walk(&forward) {
        Ok(walk) => return Ok(walk),
        Err(broken) => broken,
    };
    for order in [backward, by_date, by_date_from_the_end] {
        match walk(&order) {
            Ok(walk) => return Ok(walk),
            Err(broken) if broken.progress > furthest.progress => furthest = broken,
            Err(_) => {}
        }
    }
    Err(furthest)
}

fn walk(steps: &[&Step]) -> Result<Option<Walk>, Break> {
    let mut balance: Option<Decimal> = None;
    let mut opening = None;
    let mut unanchored = Decimal::ZERO;
    let mut progress = 0;
    let mut days = Vec::new();

    for day in steps.chunk_by(|a, b| a.date == b.date) {
        let mut remaining: Vec<&Step> = day.to_vec();
        while !remaining.is_empty() {
            let next = match balance {
                None if remaining[0].before.is_none() => 0,
                None => chain_start(&remaining),
                Some(current) => remaining
                    .iter()
                    .position(|s| s.before == Some(current))
                    .or_else(|| remaining.iter().position(|s| s.before.is_none()))
                    .ok_or_else(|| Break {
                        line: remaining[0].line,
                        expected: current + remaining[0].net,
                        printed: remaining[0].printed.unwrap_or_default(),
                        progress,
                    })?,
            };
            let step = remaining.remove(next);
            balance = match (balance, step.before) {
                (None, Some(before)) => {
                    opening = Some(before - unanchored);
                    Some(before + step.net)
                }
                (None, None) => {
                    unanchored += step.net;
                    None
                }
                (Some(current), _) => Some(current + step.net),
            };
            progress += 1;
        }
        if let Some(balance) = balance {
            days.push((day[0].date, balance));
        }
    }

    if !days.windows(2).all(|pair| pair[0].0 < pair[1].0) {
        days.clear();
    }
    Ok(opening.zip(balance).map(|(opening, closing)| Walk {
        opening,
        closing,
        days,
    }))
}

fn chain_start(remaining: &[&Step]) -> usize {
    let starts_chain = |index: usize, step: &Step| {
        step.before.is_some_and(|before| {
            !remaining.iter().enumerate().any(|(other, s)| {
                other != index && s.before.map(|other_before| other_before + s.net) == Some(before)
            })
        })
    };
    remaining
        .iter()
        .enumerate()
        .position(|(index, step)| starts_chain(index, step))
        .or_else(|| remaining.iter().position(|s| s.before.is_some()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::date;

    use super::*;
    use crate::models::output::{ExtractedLeg, PrintedBalance, StatedBalance};

    fn line(refs: usize, day: Date, amount: Decimal, balance: Option<Decimal>) -> ExtractedLine {
        ExtractedLine {
            refs: vec![refs],
            date_refs: Vec::new(),
            date: day,
            date_text: day.to_string(),
            time: None,
            description: None,
            account_ref: "Current".to_string(),
            transaction_key: None,
            external_id: None,
            related_asset_ref: None,
            legs: vec![ExtractedLeg {
                asset_ref: "GBP".to_string(),
                quantity: amount,
                amount_text: amount.abs().to_string(),
                fee_type: None,
            }],
            balances: balance
                .map(|quantity| PrintedBalance {
                    asset_ref: "GBP".to_string(),
                    quantity,
                    amount_text: quantity.to_string(),
                })
                .into_iter()
                .collect(),
        }
    }

    fn stated(kind: BalanceKind, day: Date, quantity: Decimal) -> StatedBalance {
        StatedBalance {
            refs: vec![0],
            kind,
            account_ref: "Current".to_string(),
            asset_ref: "GBP".to_string(),
            as_of_date: day,
            date_text: day.to_string(),
            quantity,
            amount_text: quantity.to_string(),
        }
    }

    fn derived(result: &BalanceCheck) -> Vec<(BalanceKind, Date, Decimal)> {
        result
            .derived
            .iter()
            .map(|a| (a.kind, a.as_of_date, a.quantity))
            .collect()
    }

    #[test]
    fn a_newest_first_chain_runs_backwards() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 03 - 18), dec!(-16), Some(dec!(1278.42))),
                line(2, date!(2024 - 03 - 17), dec!(-7), Some(dec!(1294.42))),
                line(3, date!(2024 - 03 - 17), dec!(-13), Some(dec!(1301.42))),
                line(4, date!(2024 - 03 - 15), dec!(1000), Some(dec!(1314.42))),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            derived(&result),
            vec![
                (BalanceKind::Opening, date!(2024 - 03 - 15), dec!(314.42)),
                (BalanceKind::Closing, date!(2024 - 03 - 18), dec!(1278.42)),
            ]
        );
    }

    #[test]
    fn rows_within_a_day_are_reordered_by_their_balances() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-30), Some(dec!(40))),
                line(2, date!(2024 - 01 - 02), dec!(-10), Some(dec!(90))),
                line(3, date!(2024 - 01 - 02), dec!(-20), Some(dec!(70))),
                line(4, date!(2024 - 01 - 03), dec!(5), Some(dec!(45))),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            derived(&result),
            vec![
                (BalanceKind::Opening, date!(2024 - 01 - 02), dec!(100)),
                (BalanceKind::Closing, date!(2024 - 01 - 03), dec!(45)),
            ]
        );
    }

    #[test]
    fn a_balance_printed_once_per_day_anchors_the_rows_before_it() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-10), None),
                line(2, date!(2024 - 01 - 02), dec!(-20), None),
                line(3, date!(2024 - 01 - 02), dec!(5), Some(dec!(75))),
                line(4, date!(2024 - 01 - 03), dec!(-5), None),
                line(5, date!(2024 - 01 - 03), dec!(-45), Some(dec!(25))),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            derived(&result),
            vec![
                (BalanceKind::Opening, date!(2024 - 01 - 02), dec!(100)),
                (BalanceKind::Closing, date!(2024 - 01 - 03), dec!(25)),
            ]
        );
    }

    #[test]
    fn a_wrong_sign_breaks_the_chain_where_it_happens() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-10), Some(dec!(90))),
                line(2, date!(2024 - 01 - 03), dec!(-20), Some(dec!(70))),
                line(3, date!(2024 - 01 - 04), dec!(-7), Some(dec!(77))),
                line(4, date!(2024 - 01 - 05), dec!(-2), Some(dec!(75))),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert_eq!(
            result.findings,
            vec![Finding::BalanceChainBroken {
                account_ref: "Current".to_string(),
                asset_ref: "GBP".to_string(),
                line: 2,
                expected: dec!(63),
                printed: dec!(77),
            }]
        );
        assert!(result.derived.is_empty());
        assert!(result.evidenced.is_empty());
    }

    #[test]
    fn stated_balances_must_add_up_with_the_lines_between_them() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 08 - 03), dec!(-1), None),
                line(2, date!(2024 - 08 - 18), dec!(-1), None),
                line(3, date!(2024 - 08 - 20), dec!(30), None),
            ],
            balances: vec![
                stated(BalanceKind::Opening, date!(2024 - 08 - 01), dec!(6.80)),
                stated(BalanceKind::Closing, date!(2024 - 08 - 31), dec!(34.80)),
            ],
            ..Default::default()
        };
        let result = check(&output);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert!(result.derived.is_empty());
        assert_eq!(result.evidenced.len(), 1);

        let mut dropped_row = output.clone();
        dropped_row.lines.remove(1);
        assert_eq!(
            check(&dropped_row).findings,
            vec![Finding::BalancesDisagree {
                account_ref: "Current".to_string(),
                asset_ref: "GBP".to_string(),
                earlier: BalancePosition {
                    balance: Some(0),
                    kind: BalanceKind::Opening,
                    as_of_date: date!(2024 - 08 - 01),
                    quantity: dec!(6.80),
                },
                later: BalancePosition {
                    balance: Some(1),
                    kind: BalanceKind::Closing,
                    as_of_date: date!(2024 - 08 - 31),
                    quantity: dec!(34.80),
                },
                expected: dec!(35.80),
            }]
        );
    }

    #[test]
    fn a_stated_closing_must_agree_with_the_chain() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-10), Some(dec!(90))),
                line(2, date!(2024 - 01 - 03), dec!(-20), Some(dec!(70))),
            ],
            balances: vec![stated(
                BalanceKind::Closing,
                date!(2024 - 01 - 03),
                dec!(75),
            )],
            ..Default::default()
        };
        let findings = check(&output).findings;

        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(matches!(
            &findings[0],
            Finding::BalancesDisagree { expected, later, .. }
                if *expected == dec!(70) && later.balance == Some(0)
        ));
    }

    #[test]
    fn the_last_running_balance_of_each_day_is_kept_in_either_direction() {
        let oldest_first = vec![
            line(0, date!(2026 - 03 - 01), dec!(-10), Some(dec!(90))),
            line(1, date!(2026 - 03 - 01), dec!(-5), Some(dec!(85))),
            line(2, date!(2026 - 03 - 03), dec!(-20), Some(dec!(65))),
        ];
        let mut newest_first: Vec<ExtractedLine> = oldest_first.iter().rev().cloned().collect();
        for (index, line) in newest_first.iter_mut().enumerate() {
            line.refs = vec![index];
        }
        for lines in [oldest_first, newest_first] {
            let output = ExtractionOutput {
                lines,
                ..ExtractionOutput::default()
            };
            let days: Vec<(Date, Decimal)> = check(&output)
                .end_of_day
                .iter()
                .map(|a| (a.as_of_date, a.quantity))
                .collect();
            assert_eq!(
                days,
                vec![
                    (date!(2026 - 03 - 01), dec!(85)),
                    (date!(2026 - 03 - 03), dec!(65))
                ]
            );
        }
    }

    #[test]
    fn a_chain_walked_against_the_calendar_keeps_no_daily_balances() {
        let output = ExtractionOutput {
            lines: vec![
                line(0, date!(2026 - 03 - 03), dec!(-20), Some(dec!(65))),
                line(1, date!(2026 - 03 - 01), dec!(-10), None),
            ],
            ..ExtractionOutput::default()
        };
        assert!(check(&output).end_of_day.is_empty());
    }

    #[test]
    fn a_balance_carried_forward_mid_day_matches_that_days_running_balance() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-10), Some(dec!(90))),
                line(3, date!(2024 - 01 - 02), dec!(-20), Some(dec!(70))),
            ],
            balances: vec![stated(
                BalanceKind::Snapshot,
                date!(2024 - 01 - 02),
                dec!(90),
            )],
            ..Default::default()
        };

        let result = check(&output);
        assert!(result.findings.is_empty());
        assert_eq!(result.read_mid_day, BTreeSet::from([0]));
    }

    #[test]
    fn a_chain_that_follows_the_lines_dates_holds_when_the_document_is_sorted_otherwise() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2026 - 10 - 02), dec!(-1400), Some(dec!(2962.19))),
                line(2, date!(2026 - 10 - 03), dec!(-7.65), Some(dec!(2602.95))),
                line(3, date!(2026 - 10 - 03), dec!(-300), Some(dec!(2662.19))),
                line(4, date!(2026 - 10 - 03), dec!(-43.70), Some(dec!(2618.49))),
                line(5, date!(2026 - 10 - 05), dec!(-17.90), Some(dec!(2493.41))),
                line(6, date!(2026 - 10 - 03), dec!(-7.89), Some(dec!(2610.60))),
                line(7, date!(2026 - 10 - 04), dec!(-22.25), Some(dec!(2580.70))),
                line(8, date!(2026 - 10 - 05), dec!(-9.99), Some(dec!(2570.71))),
                line(9, date!(2026 - 10 - 05), dec!(-21.94), Some(dec!(2548.77))),
                line(10, date!(2026 - 10 - 05), dec!(-37.46), Some(dec!(2511.31))),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            derived(&result),
            vec![
                (BalanceKind::Opening, date!(2026 - 10 - 02), dec!(4362.19)),
                (BalanceKind::Closing, date!(2026 - 10 - 05), dec!(2493.41)),
            ]
        );
        let days: Vec<(Date, Decimal)> = result
            .end_of_day
            .iter()
            .map(|a| (a.as_of_date, a.quantity))
            .collect();
        assert_eq!(
            days,
            vec![
                (date!(2026 - 10 - 02), dec!(2962.19)),
                (date!(2026 - 10 - 03), dec!(2602.95)),
                (date!(2026 - 10 - 04), dec!(2580.70)),
                (date!(2026 - 10 - 05), dec!(2493.41)),
            ]
        );

        let mut dropped_row = output.clone();
        dropped_row.lines.remove(6);
        assert!(matches!(
            check(&dropped_row).findings.as_slice(),
            [Finding::BalanceChainBroken { .. }]
        ));
    }

    #[test]
    fn a_balance_printed_before_each_row_is_read_forward_through_the_calendar() {
        let oldest_first = vec![
            line(1, date!(2024 - 01 - 02), dec!(-10), Some(dec!(100))),
            line(2, date!(2024 - 01 - 03), dec!(50), Some(dec!(90))),
            line(3, date!(2024 - 01 - 05), dec!(-40), Some(dec!(140))),
            line(4, date!(2024 - 01 - 06), dec!(-2), Some(dec!(100))),
        ];
        let mut newest_first: Vec<ExtractedLine> = oldest_first.iter().rev().cloned().collect();
        for (index, line) in newest_first.iter_mut().enumerate() {
            line.refs = vec![index];
        }
        for lines in [oldest_first, newest_first] {
            let result = check(&ExtractionOutput {
                lines,
                ..Default::default()
            });

            assert!(result.findings.is_empty(), "{:?}", result.findings);
            assert_eq!(
                derived(&result),
                vec![
                    (BalanceKind::Opening, date!(2024 - 01 - 02), dec!(100)),
                    (BalanceKind::Closing, date!(2024 - 01 - 06), dec!(98)),
                ]
            );
            assert_eq!(
                result.end_of_day.last().map(|a| (a.as_of_date, a.quantity)),
                Some((date!(2024 - 01 - 06), dec!(98)))
            );
        }
    }

    #[test]
    fn flipping_every_sign_does_not_pass_as_a_balance_printed_before_each_row() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(10), Some(dec!(90))),
                line(2, date!(2024 - 01 - 03), dec!(20), Some(dec!(70))),
                line(3, date!(2024 - 01 - 04), dec!(7), Some(dec!(63))),
                line(4, date!(2024 - 01 - 05), dec!(2), Some(dec!(61))),
            ],
            ..Default::default()
        };

        assert!(matches!(
            check(&output).findings.as_slice(),
            [Finding::BalanceChainBroken { .. }]
        ));
    }

    #[test]
    fn a_statements_opening_and_closing_bracket_lines_dated_outside_them() {
        let output = ExtractionOutput {
            lines: vec![
                line(2, date!(2024 - 01 - 03), dec!(-25), None),
                line(3, date!(2024 - 01 - 13), dec!(-40), None),
            ],
            balances: vec![
                stated(BalanceKind::Opening, date!(2024 - 01 - 05), dec!(-100)),
                stated(BalanceKind::Closing, date!(2024 - 02 - 04), dec!(-165)),
            ],
            ..Default::default()
        };
        let result = check(&output);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(result.evidenced.len(), 1);
        assert_eq!(result.redated, BTreeMap::from([(0, date!(2024 - 01 - 03))]));

        let mut closing_dated_too_early = output.clone();
        closing_dated_too_early.balances[1].as_of_date = date!(2024 - 01 - 05);
        let result = check(&closing_dated_too_early);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            result.redated,
            BTreeMap::from([(0, date!(2024 - 01 - 03)), (1, date!(2024 - 01 - 13))])
        );

        let mut dropped_row = output.clone();
        dropped_row.lines.remove(1);
        assert!(matches!(
            check(&dropped_row).findings.as_slice(),
            [Finding::BalancesDisagree { expected, .. }] if *expected == dec!(-100)
        ));
    }

    #[test]
    fn a_stated_opening_agrees_with_a_chain_that_starts_before_its_date() {
        let output = ExtractionOutput {
            lines: vec![
                line(2, date!(2024 - 01 - 03), dec!(-25), Some(dec!(-125))),
                line(3, date!(2024 - 01 - 13), dec!(-40), Some(dec!(-165))),
            ],
            balances: vec![
                stated(BalanceKind::Opening, date!(2024 - 01 - 05), dec!(-100)),
                stated(BalanceKind::Closing, date!(2024 - 02 - 04), dec!(-165)),
            ],
            ..Default::default()
        };
        assert!(check(&output).findings.is_empty());

        let mut wrong_opening = output.clone();
        wrong_opening.balances[0].quantity = dec!(-110);
        assert!(!check(&wrong_opening).findings.is_empty());
    }

    fn printed_at(record: usize, balance: StatedBalance) -> StatedBalance {
        StatedBalance {
            refs: vec![record],
            ..balance
        }
    }

    #[test]
    fn a_balance_brought_or_carried_forward_is_read_between_the_rows_where_it_is_printed() {
        let page_break = date!(2024 - 01 - 03);
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 02), dec!(-10), None),
                line(2, page_break, dec!(-20), None),
                line(5, page_break, dec!(-5), None),
                line(6, date!(2024 - 01 - 04), dec!(-15), None),
            ],
            balances: vec![
                stated(BalanceKind::Opening, date!(2024 - 01 - 02), dec!(100)),
                printed_at(3, stated(BalanceKind::Snapshot, page_break, dec!(70))),
                printed_at(4, stated(BalanceKind::Snapshot, page_break, dec!(70))),
                printed_at(
                    7,
                    stated(BalanceKind::Closing, date!(2024 - 01 - 04), dec!(50)),
                ),
            ],
            ..Default::default()
        };
        let result = check(&output);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(result.read_mid_day, BTreeSet::from([1, 2]));

        let mut newest_first = output.clone();
        for (line, record) in newest_first.lines.iter_mut().zip([6, 5, 2, 1]) {
            line.refs = vec![record];
        }
        let result = check(&newest_first);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(result.read_mid_day, BTreeSet::from([1, 2]));

        let mut called_closing_and_opening = output.clone();
        called_closing_and_opening.balances[1].kind = BalanceKind::Closing;
        called_closing_and_opening.balances[2].kind = BalanceKind::Opening;
        let result = check(&called_closing_and_opening);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(result.read_mid_day, BTreeSet::from([1, 2]));

        let mut row_dropped = output.clone();
        row_dropped.lines.remove(2);
        assert!(!check(&row_dropped).findings.is_empty());

        let mut row_misread = output;
        row_misread.lines[1].legs[0].quantity = dec!(-25);
        assert!(!check(&row_misread).findings.is_empty());
    }

    #[test]
    fn a_balance_printed_after_every_line_holds_on_the_last_lines_day_whatever_its_date() {
        let output = ExtractionOutput {
            lines: vec![
                line(1, date!(2024 - 01 - 05), dec!(-10), None),
                line(2, date!(2024 - 01 - 20), dec!(-20), None),
            ],
            balances: vec![
                stated(BalanceKind::Opening, date!(2024 - 01 - 01), dec!(100)),
                printed_at(
                    3,
                    stated(BalanceKind::Snapshot, date!(2024 - 01 - 05), dec!(70)),
                ),
            ],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert!(result.read_mid_day.is_empty());
        assert_eq!(result.redated, BTreeMap::from([(1, date!(2024 - 01 - 20))]));

        let mut before_every_line = output;
        before_every_line.balances[1] = printed_at(
            0,
            stated(BalanceKind::Snapshot, date!(2024 - 01 - 05), dec!(100)),
        );
        assert_eq!(
            check(&before_every_line).redated,
            BTreeMap::from([(1, date!(2024 - 01 - 04))])
        );
    }

    #[test]
    fn a_lone_balance_is_not_evidence() {
        let output = ExtractionOutput {
            lines: vec![line(1, date!(2024 - 01 - 02), dec!(-10), None)],
            balances: vec![stated(
                BalanceKind::Closing,
                date!(2024 - 01 - 31),
                dec!(90),
            )],
            ..Default::default()
        };
        let result = check(&output);

        assert!(result.findings.is_empty());
        assert!(result.evidenced.is_empty());
    }
}
