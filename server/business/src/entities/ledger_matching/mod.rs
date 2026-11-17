mod scoring;
mod text;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};

use dal::enums::transaction_types::DatabaseTransactionTypes;
use pathfinding::prelude::{kuhn_munkres, Matrix};
use rust_decimal::Decimal;
use time::{Date, Duration};
use uuid::Uuid;

use crate::dtos::ledger_matching_dto::{
    IncomingMovementDto, LedgerMatchBatchDto, LedgerMovementDto, MatchCandidateDto,
    MatchOutcomeDto, SettledMovementDto, SourceKindDto, TransactionSourceDto,
};
use scoring::{
    amount_level, is_digit_dropped_or_added, is_transposition_of, probability, AccountLevel,
    AmountLevel, DateLevel, Features, PairKind, WordingLevel,
};

pub const WINDOW_DAYS: i64 = 16;
const MIN_LAG_DAYS: i64 = -3;
const COMPOSITE_DAYS: i64 = 1;
const COVERAGE_MARGIN_DAYS: i64 = 3;
const SHORT_LAG_DAYS: i64 = 3;
const MIN_OFFSET_SAMPLES: usize = 20;
const CLEAR_MARGIN: f64 = 2.5;
const UNMATCHED_SCORE: f64 = -2.0;
const SCORE_SCALE: f64 = 1024.0;
const FORBIDDEN: i64 = -1_000_000_000_000;
const MAX_RANKED: usize = 5;
const MAX_PARTS: usize = 8;

#[derive(Clone)]
struct Source {
    kind: SourceKindDto,
    stream: String,
    date: Date,
}

struct Target {
    id: Uuid,
    in_ledger: bool,
    account_id: Option<Uuid>,
    asset_id: i32,
    quantity: Decimal,
    date: Date,
    description: Option<String>,
    wordings: Vec<String>,
    sources: Vec<Source>,
    itemised_fee: Option<Decimal>,
    several_transactions: bool,
    grouped: bool,
    is_fee: bool,
    takes_either_sign: bool,
}

impl Target {
    fn is_confirmed(&self) -> bool {
        self.sources
            .iter()
            .any(|source| source.kind.traits().proves_movement)
    }

    fn has_stream(&self, stream: &str) -> bool {
        self.sources.iter().any(|source| source.stream == stream)
    }

    fn may_be_rounded(&self) -> bool {
        !self.is_confirmed()
            && (self.sources.is_empty()
                || self
                    .sources
                    .iter()
                    .any(|source| source.kind.traits().may_be_rounded))
    }

    fn interchangeable_with(&self, other: &Target) -> bool {
        self.account_id == other.account_id
            && self.asset_id == other.asset_id
            && self.quantity == other.quantity
            && self.date == other.date
            && self.is_confirmed() == other.is_confirmed()
            && self.wordings == other.wordings
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    target: usize,
    score: f64,
    features: Features,
    at_edge: bool,
    asks_only: bool,
}

enum Decision {
    Recorded(Uuid),
    Link(Vec<usize>),
    Ask(Vec<usize>),
    Parts(Vec<usize>),
    Create,
}

struct Pairing<'a> {
    batch: &'a [IncomingMovementDto],
    open: &'a [usize],
    chosen: &'a [Option<usize>],
    claimed_by: HashMap<usize, usize>,
    wanted_by: HashMap<usize, Vec<(usize, Candidate)>>,
    cluster: Vec<usize>,
    surplus: Vec<bool>,
}

impl Pairing<'_> {
    fn movement(&self, row: usize) -> &IncomingMovementDto {
        &self.batch[self.open[row]]
    }
}

enum Coverage {
    Inside,
    NeverAutomatic,
    Outside,
}

type SourceKey = (String, String, Uuid, i32);
type ContentKey = (SourceKindDto, String, Uuid, i32);
pub type LedgerKey = (Uuid, Uuid, i32);
type OffsetKey = (Uuid, SourceKindDto, SourceKindDto);

pub fn ledger_roots(
    movements: &[LedgerMovementDto],
    sources: &[TransactionSourceDto],
) -> HashMap<LedgerKey, Uuid> {
    let member = |movement: &LedgerMovementDto| {
        (
            movement.transaction_id,
            movement.account_id,
            movement.asset_id,
        )
    };
    let mut described: HashSet<LedgerKey> = sources
        .iter()
        .filter(|source| source.kind.traits().proves_movement)
        .map(|source| (source.transaction_id, source.account_id, source.asset_id))
        .collect();
    described.extend(
        movements
            .iter()
            .filter(|movement| !movement.connector_binding_ids.is_empty())
            .map(member),
    );
    let split: HashSet<LedgerKey> = movements
        .iter()
        .filter(|movement| !described.contains(&member(movement)))
        .filter_map(|movement| Some((movement.group_id?, movement.account_id, movement.asset_id)))
        .collect();
    movements
        .iter()
        .map(|movement| {
            let root = match movement.group_id {
                Some(group_id)
                    if split.contains(&(group_id, movement.account_id, movement.asset_id)) =>
                {
                    group_id
                }
                _ => movement.transaction_id,
            };
            (member(movement), root)
        })
        .collect()
}

pub struct LedgerMatching {
    targets: Vec<Target>,
    by_day: BTreeMap<Date, Vec<usize>>,
    recorded: HashMap<SourceKey, Uuid>,
    described: HashMap<ContentKey, Uuid>,
    ledger_targets: HashMap<LedgerKey, usize>,
    moved_by: HashMap<Uuid, Vec<(Uuid, i32)>>,
    kept_for_own_account: HashSet<usize>,
    coverage: HashMap<String, (Date, Date)>,
    offset_samples: HashMap<OffsetKey, Vec<i64>>,
    offsets: HashMap<OffsetKey, i64>,
}

impl LedgerMatching {
    pub fn new(movements: Vec<LedgerMovementDto>, sources: Vec<TransactionSourceDto>) -> Self {
        let mut matching = Self {
            targets: Vec::new(),
            by_day: BTreeMap::new(),
            recorded: HashMap::new(),
            described: HashMap::new(),
            ledger_targets: HashMap::new(),
            moved_by: HashMap::new(),
            kept_for_own_account: HashSet::new(),
            coverage: HashMap::new(),
            offset_samples: HashMap::new(),
            offsets: HashMap::new(),
        };
        let roots = ledger_roots(&movements, &sources);
        let mut groups: BTreeMap<LedgerKey, Vec<LedgerMovementDto>> = BTreeMap::new();
        let mut group_of: HashMap<LedgerKey, LedgerKey> = HashMap::new();
        for movement in movements {
            let member = (
                movement.transaction_id,
                movement.account_id,
                movement.asset_id,
            );
            let key = (roots[&member], movement.account_id, movement.asset_id);
            group_of.insert(member, key);
            groups.entry(key).or_default().push(movement);
        }

        let mut records: HashMap<LedgerKey, Vec<TransactionSourceDto>> = HashMap::new();
        for source in sources {
            if let Some(external_ref) = &source.external_ref {
                matching.recorded.insert(
                    (
                        source.stream.clone(),
                        external_ref.clone(),
                        source.account_id,
                        source.asset_id,
                    ),
                    source.transaction_id,
                );
                if source.kind.traits().reference_from_content {
                    matching
                        .described
                        .entry((
                            source.kind,
                            external_ref.clone(),
                            source.account_id,
                            source.asset_id,
                        ))
                        .or_insert(source.transaction_id);
                }
            }
            if source.kind.traits().proves_movement {
                matching.extend_coverage(&source.stream, source.date);
            }
            let group = group_of
                .get(&(source.transaction_id, source.account_id, source.asset_id))
                .copied();
            if let Some(group) = group {
                records.entry(group).or_default().push(source);
            }
        }

        let mut targets: Vec<(Target, Vec<LedgerKey>)> = Vec::new();
        for (key, mut members) in groups {
            let quantity: Decimal = members.iter().map(|member| member.quantity).sum();
            if quantity.is_zero() {
                continue;
            }
            let ledger_date = members
                .iter()
                .map(|member| member.date)
                .min()
                .expect("a group has members");
            let group_records = records.remove(&key).unwrap_or_default();
            members.sort_by_key(|member| {
                let described = !member.connector_binding_ids.is_empty()
                    || group_records.iter().any(|record| {
                        record.transaction_id == member.transaction_id
                            && record.kind.traits().proves_movement
                    });
                (!described, member.transaction_id)
            });
            if group_records
                .iter()
                .any(|record| record.kind.traits().states_position)
            {
                continue;
            }
            let mut target_sources: Vec<Source> = group_records
                .iter()
                .map(|record| Source {
                    kind: record.kind,
                    stream: record.stream.clone(),
                    date: record.date,
                })
                .collect();
            for binding_id in members
                .iter()
                .flat_map(|member| member.connector_binding_ids.iter())
            {
                target_sources.push(Source {
                    kind: SourceKindDto::Connector,
                    stream: format!("connector:{binding_id}"),
                    date: ledger_date,
                });
            }
            target_sources.sort_by(|a, b| (a.date, &a.stream).cmp(&(b.date, &b.stream)));
            for source in target_sources
                .iter()
                .filter(|source| source.kind.traits().proves_movement)
            {
                matching.extend_coverage(&source.stream, source.date);
            }
            for a in &target_sources {
                for b in &target_sources {
                    let bank = a.kind.traits().proves_movement && b.kind.traits().proves_movement;
                    if bank && a.kind != b.kind {
                        matching
                            .offset_samples
                            .entry((key.1, a.kind, b.kind))
                            .or_default()
                            .push((a.date - b.date).whole_days());
                    }
                }
            }

            let description = members
                .iter()
                .find_map(|member| member.description.clone())
                .or_else(|| {
                    group_records
                        .iter()
                        .find_map(|record| record.description.clone())
                });
            let wordings = group_records
                .iter()
                .filter_map(|record| record.description.as_deref())
                .chain(description.as_deref())
                .map(text::normalise)
                .filter(|wording| !wording.is_empty())
                .collect();
            let date = target_sources
                .iter()
                .find(|source| source.kind.traits().proves_movement)
                .map_or(ledger_date, |source| source.date);
            targets.push((
                Target {
                    id: members[0].transaction_id,
                    in_ledger: true,
                    account_id: Some(key.1),
                    asset_id: key.2,
                    quantity,
                    date,
                    description,
                    wordings,
                    sources: target_sources,
                    itemised_fee: group_records.iter().find_map(|record| record.itemised_fee),
                    several_transactions: members.len() > 1,
                    grouped: members.iter().any(|member| member.group_id.is_some()),
                    is_fee: false,
                    takes_either_sign: false,
                },
                members
                    .iter()
                    .map(|member| (member.transaction_id, key.1, key.2))
                    .collect(),
            ));
        }
        targets.sort_by_key(|(target, _)| (target.date, target.id));
        for (target, members) in targets {
            matching.moved_by.entry(target.id).or_default().extend(
                target
                    .account_id
                    .map(|account_id| (account_id, target.asset_id)),
            );
            for member in members {
                matching
                    .ledger_targets
                    .insert(member, matching.targets.len());
            }
            matching.push_target(target);
        }
        matching
    }

    pub fn add_transaction_types(&mut self, types: &HashMap<Uuid, DatabaseTransactionTypes>) {
        for target in self
            .targets
            .iter_mut()
            .filter(|target| target.in_ledger && !target.several_transactions)
        {
            match types.get(&target.id) {
                Some(DatabaseTransactionTypes::AccountFees) => target.is_fee = true,
                Some(DatabaseTransactionTypes::RegularTransaction) => {
                    target.takes_either_sign = true
                }
                _ => {}
            }
        }
    }

    fn extend_coverage(&mut self, stream: &str, date: Date) {
        let range = self
            .coverage
            .entry(stream.to_string())
            .or_insert((date, date));
        *range = (range.0.min(date), range.1.max(date));
    }

    fn push_target(&mut self, target: Target) {
        self.by_day
            .entry(target.date)
            .or_default()
            .push(self.targets.len());
        self.targets.push(target);
    }

    fn add_settled(&mut self, settled: &[SettledMovementDto]) {
        for SettledMovementDto {
            movement,
            transaction_id,
        } in settled
        {
            if movement.kind.traits().proves_movement {
                self.extend_coverage(&movement.stream, movement.date);
            }
            match transaction_id {
                None => self.create_target(movement),
                Some(transaction_id) => {
                    let of_transaction = |target: &Target| {
                        target.in_ledger
                            && target.id == *transaction_id
                            && target.asset_id == movement.asset_id
                    };
                    let index = self
                        .targets
                        .iter()
                        .position(|target| {
                            of_transaction(target) && target.account_id == movement.account_id
                        })
                        .or_else(|| self.targets.iter().position(of_transaction));
                    if let Some(index) = index {
                        self.link(index, movement);
                    }
                }
            }
        }
    }

    pub fn match_batches(&mut self, batches: &[LedgerMatchBatchDto]) -> Vec<Vec<MatchOutcomeDto>> {
        for batch in batches {
            for movement in &batch.incoming {
                let own: Vec<usize> = self
                    .near(movement.date, WINDOW_DAYS)
                    .filter(|&index| {
                        let target = &self.targets[index];
                        !target.is_confirmed()
                            && self.features(movement, target, "").is_some_and(|features| {
                                features.account == AccountLevel::Same
                                    && Self::pair_kind(movement, target).may_link(features)
                            })
                    })
                    .collect();
                self.kept_for_own_account.extend(own);
            }
            for settled in &batch.settled {
                let pinned = settled
                    .transaction_id
                    .zip(settled.movement.account_id)
                    .and_then(|(transaction_id, account_id)| {
                        self.ledger_targets.get(&(
                            transaction_id,
                            account_id,
                            settled.movement.asset_id,
                        ))
                    });
                self.kept_for_own_account.extend(pinned);
            }
        }
        let mut outcomes: Vec<Vec<MatchOutcomeDto>> = batches
            .iter()
            .map(|batch| {
                self.add_settled(&batch.settled);
                self.match_batch(&batch.incoming)
            })
            .collect();

        let rows = || batches.iter().flat_map(|batch| &batch.incoming);
        let mut unmatched: HashMap<Uuid, Vec<(Uuid, i32)>> = HashMap::new();
        for (movement, outcome) in rows().zip(outcomes.iter().flatten()) {
            if let (MatchOutcomeDto::New, Some(account_id)) = (outcome, movement.account_id) {
                unmatched
                    .entry(movement.id)
                    .or_default()
                    .push((account_id, movement.asset_id));
            }
        }
        for (movement, outcome) in rows().zip(outcomes.iter_mut().flatten()) {
            let (MatchOutcomeDto::Same {
                targets: candidates,
            }
            | MatchOutcomeDto::Ambiguous { ranked: candidates }) = outcome
            else {
                continue;
            };
            let Some(unmatched) = unmatched.get(&movement.id) else {
                continue;
            };
            for candidate in candidates
                .iter_mut()
                .filter(|candidate| candidate.in_ledger && !candidate.several_transactions)
            {
                let moves = &self.moved_by[&candidate.id];
                candidate.leaves_movement_unrecorded =
                    unmatched.iter().any(|entry| !moves.contains(entry));
            }
        }
        outcomes
    }

    fn match_batch(&mut self, batch: &[IncomingMovementDto]) -> Vec<MatchOutcomeDto> {
        let mut taken: HashSet<usize> = HashSet::new();
        let mut decisions: Vec<Option<Decision>> = batch
            .iter()
            .map(|movement| self.already_known(movement, &mut taken))
            .collect();
        self.learn_offsets(batch, &decisions, &taken);
        self.match_composites(batch, &mut decisions, &mut taken);

        let mut batch_range: HashMap<&str, (Date, Date)> = HashMap::new();
        for movement in batch {
            let range = batch_range
                .entry(movement.stream.as_str())
                .or_insert_with(|| {
                    self.coverage
                        .get(&movement.stream)
                        .copied()
                        .unwrap_or((movement.date, movement.date))
                });
            *range = (range.0.min(movement.date), range.1.max(movement.date));
        }
        self.assign(batch, &mut decisions, &taken, &batch_range);
        self.match_unkeyed_fees(batch, &mut decisions, &taken, &batch_range);
        self.ask_about_slips(batch, &mut decisions, &taken, &batch_range);
        self.ask_about_parts(batch, &mut decisions, &taken);

        let decisions: Vec<Decision> = decisions
            .into_iter()
            .map(|decision| decision.unwrap_or(Decision::Create))
            .collect();
        let outcomes = decisions
            .iter()
            .map(|decision| self.outcome(decision))
            .collect();
        self.record(batch, &decisions);
        outcomes
    }

    fn already_known(
        &self,
        movement: &IncomingMovementDto,
        taken: &mut HashSet<usize>,
    ) -> Option<Decision> {
        let external_ref = movement.external_ref.clone()?;
        let account_id = movement.account_id?;
        let same_stream = (
            movement.stream.clone(),
            external_ref.clone(),
            account_id,
            movement.asset_id,
        );
        if let Some(transaction_id) = self.recorded.get(&same_stream) {
            return Some(Decision::Recorded(*transaction_id));
        }
        if !movement.kind.traits().reference_from_content {
            return None;
        }
        let said_before = (movement.kind, external_ref, account_id, movement.asset_id);
        let transaction_id = *self.described.get(&said_before)?;
        match self
            .ledger_targets
            .get(&(transaction_id, account_id, movement.asset_id))
        {
            Some(&target) => taken.insert(target).then(|| Decision::Link(vec![target])),
            None => Some(Decision::Recorded(transaction_id)),
        }
    }

    fn near(&self, date: Date, days: i64) -> impl Iterator<Item = usize> + '_ {
        self.by_day
            .range(date - Duration::days(days)..=date + Duration::days(days))
            .flat_map(|(_, targets)| targets.iter().copied())
    }

    fn match_composites(
        &self,
        batch: &[IncomingMovementDto],
        decisions: &mut [Option<Decision>],
        taken: &mut HashSet<usize>,
    ) {
        for (index, movement) in batch.iter().enumerate() {
            let Some(fee) = movement.itemised_fee else {
                continue;
            };
            if decisions[index].is_some() || !movement.kind.traits().proves_movement {
                continue;
            }
            let near: Vec<usize> = self
                .near(movement.date, COMPOSITE_DAYS)
                .filter(|target| !taken.contains(target))
                .filter(|&target| {
                    let target = &self.targets[target];
                    target.account_id == movement.account_id
                        && target.asset_id == movement.asset_id
                        && !target.has_stream(&movement.stream)
                })
                .collect();
            if near
                .iter()
                .any(|&target| self.targets[target].quantity == movement.quantity)
            {
                continue;
            }
            let pairs = unique_pair(&near, movement.quantity, fee, |&target| {
                self.targets[target].quantity
            });
            if let Some((a, b)) = pairs {
                decisions[index] = Some(Decision::Link(vec![a, b]));
                taken.extend([a, b]);
            }
        }

        let dates: HashSet<Date> = batch.iter().map(|movement| movement.date).collect();
        let grouped: HashSet<usize> = dates
            .iter()
            .flat_map(|&date| self.near(date, COMPOSITE_DAYS))
            .filter(|&target| self.targets[target].itemised_fee.is_some())
            .collect();
        let mut grouped: Vec<usize> = grouped.into_iter().collect();
        grouped.sort_unstable();
        for target_index in grouped {
            if taken.contains(&target_index) {
                continue;
            }
            let target = &self.targets[target_index];
            let Some(fee) = target.itemised_fee else {
                continue;
            };
            let rows: Vec<usize> = (0..batch.len())
                .filter(|&row| {
                    let movement = &batch[row];
                    decisions[row].is_none()
                        && movement.kind.traits().proves_movement
                        && movement.account_id == target.account_id
                        && movement.asset_id == target.asset_id
                        && (movement.date - target.date).whole_days().abs() <= COMPOSITE_DAYS
                        && !target.has_stream(&movement.stream)
                })
                .collect();
            if rows
                .iter()
                .any(|&row| batch[row].quantity == target.quantity)
            {
                continue;
            }
            if let Some((a, b)) =
                unique_pair(&rows, target.quantity, fee, |&row| batch[row].quantity)
            {
                decisions[a] = Some(Decision::Link(vec![target_index]));
                decisions[b] = Some(Decision::Link(vec![target_index]));
                taken.insert(target_index);
            }
        }
    }

    fn learn_offsets(
        &mut self,
        batch: &[IncomingMovementDto],
        decisions: &[Option<Decision>],
        taken: &HashSet<usize>,
    ) {
        let mut wanted_by: HashMap<usize, usize> = HashMap::new();
        let mut only_candidate: Vec<(usize, usize)> = Vec::new();
        for (row, movement) in batch.iter().enumerate() {
            if decisions[row].is_some() || !movement.kind.traits().proves_movement {
                continue;
            }
            let exact: Vec<usize> = self
                .near(movement.date, WINDOW_DAYS)
                .filter(|index| {
                    let target = &self.targets[*index];
                    !taken.contains(index)
                        && target.is_confirmed()
                        && target.account_id == movement.account_id
                        && target.asset_id == movement.asset_id
                        && target.quantity == movement.quantity
                        && !target.has_stream(&movement.stream)
                })
                .collect();
            for &target in &exact {
                *wanted_by.entry(target).or_default() += 1;
            }
            if let [target] = exact[..] {
                only_candidate.push((row, target));
            }
        }
        for (row, target) in only_candidate {
            let movement = &batch[row];
            let Some(account_id) = movement.account_id else {
                continue;
            };
            if wanted_by[&target] != 1 {
                continue;
            }
            for source in &self.targets[target].sources {
                if source.kind.traits().proves_movement && source.kind != movement.kind {
                    let days = (movement.date - source.date).whole_days();
                    self.offset_samples
                        .entry((account_id, movement.kind, source.kind))
                        .or_default()
                        .push(days);
                    self.offset_samples
                        .entry((account_id, source.kind, movement.kind))
                        .or_default()
                        .push(-days);
                }
            }
        }
        self.offsets = self
            .offset_samples
            .iter()
            .filter(|(_, samples)| samples.len() >= MIN_OFFSET_SAMPLES)
            .map(|(key, samples)| {
                let mut sorted = samples.clone();
                sorted.sort_unstable();
                (*key, sorted[sorted.len() / 2])
            })
            .collect();
    }

    fn date_gap(&self, movement: &IncomingMovementDto, target: &Target) -> i64 {
        target
            .sources
            .iter()
            .filter(|source| source.kind.traits().proves_movement)
            .map(|source| {
                let offset = movement
                    .account_id
                    .and_then(|account_id| {
                        self.offsets.get(&(account_id, movement.kind, source.kind))
                    })
                    .copied()
                    .unwrap_or(0);
                ((movement.date - source.date).whole_days() - offset).abs()
            })
            .min()
            .unwrap_or_else(|| (movement.date - target.date).whole_days().abs())
    }

    fn coverage_of(
        &self,
        movement: &IncomingMovementDto,
        target: &Target,
        batch_range: &HashMap<&str, (Date, Date)>,
    ) -> Coverage {
        if !movement.kind.traits().proves_movement {
            return Coverage::Inside;
        }
        let lists_whole_period = movement.kind.traits().lists_whole_period;
        let (mine_from, mine_to) = batch_range[movement.stream.as_str()];
        let margin = Duration::days(COVERAGE_MARGIN_DAYS);
        let within = |date: Date, from: Date, to: Date| from <= date && date <= to;
        let mut never_automatic = false;
        let mut dated_in_mine = None;
        for source in target.sources.iter().filter(|source| {
            source.kind.traits().proves_movement && source.stream != movement.stream
        }) {
            let Some(&(from, to)) = self.coverage.get(&source.stream) else {
                continue;
            };
            let lag = Duration::days(
                movement
                    .account_id
                    .and_then(|account_id| {
                        self.offsets.get(&(account_id, movement.kind, source.kind))
                    })
                    .copied()
                    .unwrap_or(0),
            );
            let mine_within =
                |from, to| within(movement.date, from, to) || within(movement.date - lag, from, to);
            let theirs_within =
                |from, to| within(source.date, from, to) || within(source.date + lag, from, to);
            let inside = mine_within(from, to);
            let near = mine_within(from - margin, to + margin);
            let source_inside = theirs_within(mine_from, mine_to);
            if source.kind.traits().lists_whole_period && lists_whole_period {
                let source_near = theirs_within(mine_from - margin, mine_to + margin);
                if !((inside && source_near) || (source_inside && near)) {
                    return Coverage::Outside;
                }
                never_automatic |= !(inside && source_inside);
            } else {
                if !near {
                    return Coverage::Outside;
                }
                never_automatic |= !inside;
            }
            dated_in_mine = Some(dated_in_mine.unwrap_or(false) || source_inside);
        }
        if never_automatic || (lists_whole_period && dated_in_mine == Some(false)) {
            Coverage::NeverAutomatic
        } else {
            Coverage::Inside
        }
    }

    fn pair_kind(movement: &IncomingMovementDto, target: &Target) -> PairKind {
        match (
            movement.kind.traits().proves_movement,
            target.is_confirmed(),
        ) {
            (true, true) => PairKind::BankBank,
            (false, false) => PairKind::UserUser,
            _ => PairKind::BankUser,
        }
    }

    fn features(
        &self,
        movement: &IncomingMovementDto,
        target: &Target,
        wording: &str,
    ) -> Option<Features> {
        let kind = Self::pair_kind(movement, target);
        if target.asset_id != movement.asset_id {
            return None;
        }
        let account = match (movement.account_id, target.account_id) {
            (Some(a), Some(b)) if a == b => AccountLevel::Same,
            (Some(_), Some(_)) if kind == PairKind::BankBank => return None,
            (Some(_), Some(_)) => AccountLevel::Mismatch,
            _ => AccountLevel::Unknown,
        };
        let amount = amount_level(
            kind,
            movement.quantity,
            target.quantity,
            movement.kind.traits().may_be_rounded,
            target.may_be_rounded(),
        )?;
        let date = if kind == PairKind::BankUser {
            let lag = if movement.kind.traits().proves_movement {
                (movement.date - target.date).whole_days()
            } else {
                let booked = target
                    .sources
                    .iter()
                    .find(|source| source.kind.traits().proves_movement)
                    .map_or(target.date, |source| source.date);
                (booked - movement.date).whole_days()
            };
            if !(MIN_LAG_DAYS..=WINDOW_DAYS).contains(&lag) {
                return None;
            }
            if lag < 0 {
                DateLevel::BeforePurchase
            } else {
                DateLevel::of_gap(lag)
            }
        } else if kind == PairKind::BankBank {
            DateLevel::of_gap(self.date_gap(movement, target))
        } else {
            DateLevel::of_gap((movement.date - target.date).whole_days().abs())
        };
        let wording = match kind {
            PairKind::BankUser => WordingLevel::Unknown,
            _ => WordingLevel::of(
                target
                    .wordings
                    .iter()
                    .filter_map(|other| text::similarity(wording, other))
                    .reduce(f64::max),
            ),
        };
        Some(Features {
            amount,
            date,
            wording,
            account,
        })
    }

    fn candidates(
        &self,
        movement: &IncomingMovementDto,
        taken: &HashSet<usize>,
        batch_range: &HashMap<&str, (Date, Date)>,
    ) -> Vec<Candidate> {
        let wording = text::normalise(movement.description.as_deref().unwrap_or_default());
        let mut candidates = Vec::new();
        for index in self.near(movement.date, WINDOW_DAYS) {
            let target = &self.targets[index];
            if taken.contains(&index)
                || (movement.kind.traits().proves_movement && target.has_stream(&movement.stream))
                || (movement.kind.traits().one_per_transaction
                    && target
                        .sources
                        .iter()
                        .any(|source| source.kind == movement.kind))
            {
                continue;
            }
            let at_edge = match self.coverage_of(movement, target, batch_range) {
                Coverage::Outside => continue,
                Coverage::NeverAutomatic => true,
                Coverage::Inside => false,
            };
            let Some(features) = self.features(movement, target, &wording) else {
                continue;
            };
            if features.account == AccountLevel::Mismatch
                && self.kept_for_own_account.contains(&index)
            {
                continue;
            }
            let kind = Self::pair_kind(movement, target);
            candidates.push(Candidate {
                target: index,
                score: kind.score(features),
                features,
                at_edge,
                asks_only: !kind.may_link(features),
            });
        }
        candidates
    }

    fn assign(
        &self,
        batch: &[IncomingMovementDto],
        decisions: &mut [Option<Decision>],
        taken: &HashSet<usize>,
        batch_range: &HashMap<&str, (Date, Date)>,
    ) {
        let mut open: Vec<usize> = (0..batch.len())
            .filter(|&row| decisions[row].is_none())
            .collect();
        open.sort_by_cached_key(|&row| {
            let movement = &batch[row];
            (
                movement.date,
                movement.quantity,
                movement.account_id,
                movement.asset_id,
                text::normalise(movement.description.as_deref().unwrap_or_default()),
            )
        });
        let mut rows: Vec<Vec<Candidate>> = open
            .iter()
            .map(|&row| self.candidates(&batch[row], taken, batch_range))
            .collect();

        let may_link = |candidate: &&Candidate| !candidate.at_edge && !candidate.asks_only;
        let linkable: Vec<Vec<Candidate>> = rows
            .iter()
            .map(|candidates| candidates.iter().filter(may_link).copied().collect())
            .collect();
        let mut chosen = pair_up(&linkable);
        let paired: HashSet<usize> = chosen.iter().flatten().copied().collect();
        let rest: Vec<Vec<Candidate>> = rows
            .iter()
            .zip(&chosen)
            .map(|(candidates, chosen)| {
                candidates
                    .iter()
                    .filter(|candidate| {
                        chosen.is_none()
                            && !may_link(candidate)
                            && !paired.contains(&candidate.target)
                    })
                    .copied()
                    .collect()
            })
            .collect();
        for (chosen, asked) in chosen.iter_mut().zip(pair_up(&rest)) {
            *chosen = chosen.or(asked);
        }
        let mut claimed_by: HashMap<usize, usize> = HashMap::new();
        for (row, target) in chosen.iter().enumerate() {
            if let Some(target) = target {
                claimed_by.insert(*target, row);
            }
        }
        let mut wanted_by: HashMap<usize, Vec<(usize, Candidate)>> = HashMap::new();
        for (row, candidates) in rows.iter().enumerate() {
            for candidate in candidates {
                wanted_by
                    .entry(candidate.target)
                    .or_default()
                    .push((row, *candidate));
            }
        }
        let mut clusters = HashMap::new();
        let cluster: Vec<usize> = open
            .iter()
            .map(|&row| {
                let movement = &batch[row];
                let next = clusters.len();
                *clusters
                    .entry((
                        movement.account_id,
                        movement.asset_id,
                        movement.date,
                        movement.quantity,
                        text::normalise(movement.description.as_deref().unwrap_or_default()),
                    ))
                    .or_insert(next)
            })
            .collect();
        let paired_clusters: HashSet<usize> = (0..open.len())
            .filter(|&row| chosen[row].is_some())
            .map(|row| cluster[row])
            .collect();
        let surplus = (0..open.len())
            .map(|row| chosen[row].is_none() && paired_clusters.contains(&cluster[row]))
            .collect();
        for candidates in &mut rows {
            candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        }
        let pairing = Pairing {
            batch,
            open: &open,
            chosen: &chosen,
            claimed_by,
            wanted_by,
            cluster,
            surplus,
        };

        let mut linked: HashSet<usize> = HashSet::new();
        for (row, &index) in open.iter().enumerate() {
            let Some(target) = chosen[row] else {
                continue;
            };
            let me = rows[row]
                .iter()
                .find(|candidate| candidate.target == target)
                .expect("a chosen target is one of the row's candidates");
            let decision = self.decide_paired(row, me, &rows[row], &pairing);
            if matches!(decision, Decision::Link(_)) {
                linked.insert(target);
            }
            decisions[index] = Some(decision);
        }
        for (row, &index) in open.iter().enumerate() {
            let left: Vec<Candidate> = rows[row]
                .iter()
                .filter(|candidate| {
                    !linked.contains(&candidate.target)
                        || (may_link(candidate)
                            && Self::pair_kind(&batch[index], &self.targets[candidate.target])
                                .policy()
                                .link
                                .is_some_and(|bar| probability(candidate.score) >= bar))
                })
                .copied()
                .collect();
            match chosen[row] {
                Some(target) => {
                    if let Some(Decision::Ask(listed)) = &mut decisions[index] {
                        *listed = self.ranked(&left, target);
                    }
                }
                None if pairing.surplus[row] => decisions[index] = Some(Decision::Create),
                None => {
                    decisions[index] = Some(self.decide_unpaired(&batch[index], &left, &pairing))
                }
            }
        }
    }

    fn decide_unpaired(
        &self,
        movement: &IncomingMovementDto,
        candidates: &[Candidate],
        pairing: &Pairing,
    ) -> Decision {
        let Some(best) = candidates.first() else {
            return Decision::Create;
        };
        let kind = Self::pair_kind(movement, &self.targets[best.target]);
        let explained = kind == PairKind::BankBank
            && candidates.iter().all(|candidate| {
                let target = &self.targets[candidate.target];
                pairing
                    .claimed_by
                    .get(&candidate.target)
                    .is_some_and(|&row| {
                        self.date_gap(pairing.movement(row), target)
                            <= self.date_gap(movement, target)
                    })
            });
        let only_pair = kind == PairKind::BankUser
            && candidates.len() == 1
            && pairing.wanted_by[&best.target].len() == 1
            && best.features.amount == AmountLevel::Exact
            && best.features.account == AccountLevel::Same;
        if !explained && (only_pair || probability(best.score) >= kind.policy().ask) {
            Decision::Ask(self.ranked(candidates, best.target))
        } else {
            Decision::Create
        }
    }

    fn best_rival(
        &self,
        row: usize,
        me: &Candidate,
        candidates: &[Candidate],
        pairing: &Pairing,
    ) -> Option<f64> {
        let target = me.target;
        let movement = pairing.movement(row);
        let kind = Self::pair_kind(movement, &self.targets[target]);
        let unclaimed = candidates.iter().filter(|candidate| {
            !candidate.asks_only
                && !pairing.claimed_by.contains_key(&candidate.target)
                && !self.targets[candidate.target].interchangeable_with(&self.targets[target])
        });
        let competitors = pairing.wanted_by[&target].iter().filter(|(other, theirs)| {
            !theirs.asks_only
                && pairing.chosen[*other].is_none()
                && pairing.cluster[*other] != pairing.cluster[row]
                && !(pairing.surplus[*other] && theirs.score < me.score)
        });
        let (free, competing): (Vec<f64>, Vec<f64>) = if kind == PairKind::BankBank {
            let gap = self.date_gap(movement, &self.targets[target]);
            (
                unclaimed
                    .filter(|candidate| {
                        self.date_gap(movement, &self.targets[candidate.target]) <= gap
                    })
                    .map(|candidate| candidate.score)
                    .collect(),
                competitors
                    .filter(|(other, _)| {
                        self.date_gap(pairing.movement(*other), &self.targets[target]) <= gap
                    })
                    .map(|(_, theirs)| theirs.score)
                    .collect(),
            )
        } else {
            (
                unclaimed.map(|candidate| candidate.score).collect(),
                competitors.map(|(_, theirs)| theirs.score).collect(),
            )
        };
        free.into_iter().chain(competing).reduce(f64::max)
    }

    fn decide_paired(
        &self,
        row: usize,
        me: &Candidate,
        candidates: &[Candidate],
        pairing: &Pairing,
    ) -> Decision {
        let movement = pairing.movement(row);
        let target = &self.targets[me.target];
        let kind = Self::pair_kind(movement, target);
        let policy = kind.policy();
        let allowed = !me.at_edge && !me.asks_only;
        let probability = probability(me.score);
        let rival = self.best_rival(row, me, candidates, pairing);
        let clear_winner = rival.is_none_or(|rival| me.score - rival >= CLEAR_MARGIN);
        let close = |other: &IncomingMovementDto, target: &Target| {
            other.quantity == target.quantity && self.date_gap(other, target) <= SHORT_LAG_DAYS
        };
        let lag_seen_elsewhere = target.sources.iter().any(|source| {
            let lag = (movement.date - source.date).whole_days();
            movement
                .account_id
                .and_then(|account_id| {
                    self.offset_samples
                        .get(&(account_id, movement.kind, source.kind))
                })
                .is_some_and(|lags| lags.iter().filter(|&&other| other == lag).count() > 1)
        });
        let only_pair = kind == PairKind::BankBank
            && lag_seen_elsewhere
            && me.features.account == AccountLevel::Same
            && candidates
                .iter()
                .filter(|other| close(movement, &self.targets[other.target]))
                .count()
                == 1
            && pairing.wanted_by[&me.target]
                .iter()
                .filter(|(other, _)| close(pairing.movement(*other), target))
                .count()
                == 1
            && close(movement, target);
        if allowed
            && clear_winner
            && (only_pair || policy.link.is_some_and(|bar| probability >= bar))
        {
            Decision::Link(vec![me.target])
        } else if probability >= policy.ask {
            Decision::Ask(vec![me.target])
        } else {
            Decision::Create
        }
    }

    fn ranked(&self, candidates: &[Candidate], first: usize) -> Vec<usize> {
        let mut listed = vec![first];
        for candidate in candidates {
            let target = &self.targets[candidate.target];
            let new = listed.iter().all(|&other| {
                other != candidate.target && !self.targets[other].interchangeable_with(target)
            });
            if new && listed.len() < MAX_RANKED {
                listed.push(candidate.target);
            }
        }
        listed
    }

    fn match_unkeyed_fees(
        &self,
        batch: &[IncomingMovementDto],
        decisions: &mut [Option<Decision>],
        taken: &HashSet<usize>,
        batch_range: &HashMap<&str, (Date, Date)>,
    ) {
        let mut claimed = taken.clone();
        for decision in decisions.iter().flatten() {
            if let Decision::Link(targets) | Decision::Ask(targets) = decision {
                claimed.extend(targets);
            }
        }
        let leftover: Vec<usize> = (0..batch.len())
            .filter(|&row| {
                matches!(decisions[row], Some(Decision::Create))
                    && batch[row].kind.traits().proves_movement
                    && batch[row].account_id.is_some()
            })
            .collect();
        let free = |row: usize| -> Vec<usize> {
            let movement = &batch[row];
            self.near(movement.date, COMPOSITE_DAYS)
                .filter(|index| {
                    let target = &self.targets[*index];
                    !claimed.contains(index)
                        && target.is_confirmed()
                        && target.account_id == movement.account_id
                        && target.asset_id == movement.asset_id
                        && !target.has_stream(&movement.stream)
                        && matches!(
                            self.coverage_of(movement, target, batch_range),
                            Coverage::Inside
                        )
                })
                .collect()
        };

        let mut sums: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();
        for &row in &leftover {
            let movement = &batch[row];
            let near = free(row);
            if near.is_empty() {
                continue;
            }
            if !movement.is_fee {
                for (position, &a) in near.iter().enumerate() {
                    for &b in &near[position + 1..] {
                        let (first, second) = (&self.targets[a], &self.targets[b]);
                        if first.is_fee != second.is_fee
                            && first.quantity + second.quantity == movement.quantity
                        {
                            sums.push((vec![row], vec![a, b]));
                        }
                    }
                }
                continue;
            }
            for &other in &leftover {
                let payment = &batch[other];
                if payment.is_fee
                    || payment.stream != movement.stream
                    || payment.account_id != movement.account_id
                    || payment.asset_id != movement.asset_id
                {
                    continue;
                }
                let net = movement.quantity + payment.quantity;
                let theirs = free(other);
                sums.extend(
                    near.iter()
                        .filter(|index| {
                            let target = &self.targets[**index];
                            !target.is_fee && target.quantity == net && theirs.contains(index)
                        })
                        .map(|&target| (vec![row, other], vec![target])),
                );
            }
        }

        let mut row_uses: HashMap<usize, usize> = HashMap::new();
        let mut target_uses: HashMap<usize, usize> = HashMap::new();
        for (rows, targets) in &sums {
            for &row in rows {
                *row_uses.entry(row).or_default() += 1;
            }
            for &target in targets {
                *target_uses.entry(target).or_default() += 1;
            }
        }
        for (rows, targets) in sums {
            if rows.iter().all(|row| row_uses[row] == 1)
                && targets.iter().all(|target| target_uses[target] == 1)
            {
                for row in rows {
                    decisions[row] = Some(Decision::Link(targets.clone()));
                }
            }
        }
    }

    fn ask_about_slips(
        &self,
        batch: &[IncomingMovementDto],
        decisions: &mut [Option<Decision>],
        taken: &HashSet<usize>,
        batch_range: &HashMap<&str, (Date, Date)>,
    ) {
        let mut claimed = taken.clone();
        for decision in decisions.iter().flatten() {
            match decision {
                Decision::Link(targets) => claimed.extend(targets),
                Decision::Ask(targets) => claimed.extend(targets.first()),
                Decision::Recorded(_) | Decision::Parts(_) | Decision::Create => {}
            }
        }
        let mut slips: Vec<(usize, usize)> = Vec::new();
        for (row, movement) in batch.iter().enumerate() {
            if !matches!(decisions[row], Some(Decision::Create))
                || !movement.kind.traits().proves_movement
                || movement.account_id.is_none()
            {
                continue;
            }
            let (first, last) = batch_range[movement.stream.as_str()];
            for index in self.near(movement.date, SHORT_LAG_DAYS) {
                let target = &self.targets[index];
                if claimed.contains(&index)
                    || !target.in_ledger
                    || target.several_transactions
                    || !target.may_be_rounded()
                    || target.account_id != movement.account_id
                    || target.asset_id != movement.asset_id
                {
                    continue;
                }
                let wrong_sign = target.takes_either_sign
                    && target.quantity == -movement.quantity
                    && movement.kind.traits().lists_whole_period
                    && (first..=last).contains(&target.date);
                if wrong_sign
                    || is_transposition_of(target.quantity, movement.quantity)
                    || is_digit_dropped_or_added(target.quantity, movement.quantity)
                {
                    slips.push((row, index));
                }
            }
        }
        for &(row, target) in &slips {
            let related = slips
                .iter()
                .filter(|(other_row, other_target)| *other_row == row || *other_target == target)
                .count();
            if related == 1 {
                decisions[row] = Some(Decision::Ask(vec![target]));
            }
        }
    }

    fn ask_about_parts(
        &self,
        batch: &[IncomingMovementDto],
        decisions: &mut [Option<Decision>],
        taken: &HashSet<usize>,
    ) {
        let mut claimed = taken.clone();
        for decision in decisions.iter().flatten() {
            match decision {
                Decision::Link(targets) | Decision::Parts(targets) => claimed.extend(targets),
                Decision::Ask(targets) => claimed.extend(targets.first()),
                Decision::Recorded(_) | Decision::Create => {}
            }
        }
        let mut found: Vec<(usize, Vec<usize>)> = Vec::new();
        for (row, movement) in batch.iter().enumerate() {
            if !matches!(decisions[row], Some(Decision::Create))
                || !movement.kind.traits().proves_movement
                || movement.account_id.is_none()
            {
                continue;
            }
            let mut by_day: BTreeMap<Date, Vec<usize>> = BTreeMap::new();
            for index in self.near(movement.date, SHORT_LAG_DAYS) {
                let target = &self.targets[index];
                if claimed.contains(&index)
                    || !target.in_ledger
                    || target.grouped
                    || target.is_fee
                    || target.is_confirmed()
                    || target.account_id != movement.account_id
                    || target.asset_id != movement.asset_id
                    || target.quantity.is_sign_negative() != movement.quantity.is_sign_negative()
                {
                    continue;
                }
                by_day.entry(target.date).or_default().push(index);
            }
            let sets: Vec<Vec<usize>> = by_day
                .values()
                .flat_map(|day| {
                    sets_adding_up(day, movement.quantity, |&index| {
                        self.targets[index].quantity
                    })
                })
                .collect();
            if let [parts] = &sets[..] {
                found.push((row, parts.clone()));
            }
        }
        for (row, parts) in &found {
            let shared = found.iter().any(|(other_row, other)| {
                other_row != row && other.iter().any(|target| parts.contains(target))
            });
            if !shared {
                decisions[*row] = Some(Decision::Parts(parts.clone()));
            }
        }
    }

    fn outcome(&self, decision: &Decision) -> MatchOutcomeDto {
        let candidates = |targets: &[usize]| -> Vec<MatchCandidateDto> {
            targets
                .iter()
                .map(|&index| {
                    let target = &self.targets[index];
                    MatchCandidateDto {
                        id: target.id,
                        in_ledger: target.in_ledger,
                        confirmed: target.is_confirmed(),
                        several_transactions: target.several_transactions,
                        leaves_movement_unrecorded: false,
                        account_id: target.account_id,
                        asset_id: target.asset_id,
                        quantity: target.quantity,
                        date: target.date,
                        description: target.description.clone(),
                    }
                })
                .collect()
        };
        match decision {
            Decision::Recorded(transaction_id) => MatchOutcomeDto::Recorded {
                transaction_id: *transaction_id,
            },
            Decision::Link(targets) => MatchOutcomeDto::Same {
                targets: candidates(targets),
            },
            Decision::Ask(targets) => MatchOutcomeDto::Ambiguous {
                ranked: candidates(targets),
            },
            Decision::Parts(targets) => MatchOutcomeDto::Parts {
                parts: candidates(targets),
            },
            Decision::Create => MatchOutcomeDto::New,
        }
    }

    fn record(&mut self, batch: &[IncomingMovementDto], decisions: &[Decision]) {
        for (movement, decision) in batch.iter().zip(decisions) {
            if movement.kind.traits().proves_movement {
                self.extend_coverage(&movement.stream, movement.date);
            }
            match decision {
                Decision::Recorded(_) => {}
                Decision::Link(targets) => {
                    for &target in targets {
                        self.link(target, movement);
                    }
                }
                Decision::Ask(_) | Decision::Parts(_) | Decision::Create => {
                    self.create_target(movement)
                }
            }
        }
    }

    fn link(&mut self, index: usize, movement: &IncomingMovementDto) {
        let target = &mut self.targets[index];
        if movement.kind.traits().proves_movement && !target.is_confirmed() {
            target.account_id = movement.account_id;
        }
        target.sources.push(Source {
            kind: movement.kind,
            stream: movement.stream.clone(),
            date: movement.date,
        });
        let wording = text::normalise(movement.description.as_deref().unwrap_or_default());
        if !wording.is_empty() {
            target.wordings.push(wording);
        }
    }

    fn create_target(&mut self, movement: &IncomingMovementDto) {
        let wording = text::normalise(movement.description.as_deref().unwrap_or_default());
        self.push_target(Target {
            id: movement.id,
            in_ledger: false,
            account_id: movement.account_id,
            asset_id: movement.asset_id,
            quantity: movement.quantity,
            date: movement.date,
            description: movement.description.clone(),
            wordings: if wording.is_empty() {
                Vec::new()
            } else {
                vec![wording]
            },
            sources: vec![Source {
                kind: movement.kind,
                stream: movement.stream.clone(),
                date: movement.date,
            }],
            itemised_fee: movement.itemised_fee,
            several_transactions: false,
            grouped: false,
            is_fee: movement.is_fee,
            takes_either_sign: false,
        });
    }
}

fn sets_adding_up<T: Copy>(
    items: &[T],
    total: Decimal,
    quantity: impl Fn(&T) -> Decimal,
) -> Vec<Vec<T>> {
    if items.len() > MAX_PARTS {
        return Vec::new();
    }
    (0u32..1 << items.len())
        .filter(|mask| mask.count_ones() >= 2)
        .map(|mask| {
            items
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, item)| *item)
                .collect::<Vec<T>>()
        })
        .filter(|set| set.iter().map(&quantity).sum::<Decimal>() == total)
        .collect()
}

fn unique_pair<T: Copy>(
    items: &[T],
    total: Decimal,
    fee: Decimal,
    quantity: impl Fn(&T) -> Decimal,
) -> Option<(T, T)> {
    let mut found = None;
    for (index, a) in items.iter().enumerate() {
        for b in &items[index + 1..] {
            let (x, y) = (quantity(a), quantity(b));
            if x + y == total && (x == fee || y == fee) {
                if found.is_some() {
                    return None;
                }
                found = Some((*a, *b));
            }
        }
    }
    found
}

fn pair_up(rows: &[Vec<Candidate>]) -> Vec<Option<usize>> {
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    fn root(parent: &mut [usize], mut row: usize) -> usize {
        while parent[row] != row {
            parent[row] = parent[parent[row]];
            row = parent[row];
        }
        row
    }
    let mut first_row_of: HashMap<usize, usize> = HashMap::new();
    for (row, candidates) in rows.iter().enumerate() {
        for candidate in candidates {
            let first = *first_row_of.entry(candidate.target).or_insert(row);
            let (a, b) = (root(&mut parent, row), root(&mut parent, first));
            parent[a] = b;
        }
    }
    let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for row in 0..rows.len() {
        if !rows[row].is_empty() {
            let component = root(&mut parent, row);
            components.entry(component).or_default().push(row);
        }
    }

    let mut chosen = vec![None; rows.len()];
    for members in components.values() {
        let mut targets: Vec<usize> = members
            .iter()
            .flat_map(|&row| rows[row].iter().map(|candidate| candidate.target))
            .collect();
        targets.sort_unstable();
        targets.dedup();
        let column: HashMap<usize, usize> = targets
            .iter()
            .enumerate()
            .map(|(column, &target)| (target, column))
            .collect();

        let width = targets.len() + members.len();
        let mut matrix = Matrix::new(members.len(), width, FORBIDDEN);
        for (row, &member) in members.iter().enumerate() {
            for candidate in &rows[member] {
                matrix[(row, column[&candidate.target])] = scaled(candidate.score);
            }
            matrix[(row, targets.len() + row)] = scaled(UNMATCHED_SCORE);
        }
        let (_, assigned) = kuhn_munkres(&matrix);
        for (row, &member) in members.iter().enumerate() {
            chosen[member] = targets.get(assigned[row]).copied();
        }
    }
    chosen
}

fn scaled(score: f64) -> i64 {
    (score * SCORE_SCALE).round() as i64
}
