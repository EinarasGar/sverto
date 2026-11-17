use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use dal::enums::transaction_types::DatabaseTransactionTypes;
use dal::models::import_models::{ImportAccountMappingModel, ImportAssetMappingModel};
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime, Time, UtcOffset};
use uuid::Uuid;

use crate::dtos::entry_dto::EntryDto;
use crate::dtos::fee_entry_dto::FeeEntryDto;
use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
use crate::dtos::imports::import_classification_dto::{ImportLineTypeDto, ImportParsedLineDto};
use crate::dtos::imports::import_staging_dto::{
    ImportLineClassificationDto, StagedMovementDto, StagedTransactionDto, UnstageableGroupDto,
};
use crate::dtos::transaction_dto::{
    AccountFeesMetadataDto, AssetPurchaseMetadataDto, AssetSaleMetadataDto, AssetTradeMetadataDto,
    AssetTransferInMetadataDto, AssetTransferOutMetadataDto, CashBalanceTransferMetadataDto,
    CashDividendMetadataDto, CashTransferInMetadataDto, CashTransferOutMetadataDto,
    RegularTransactionMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
};
use crate::dtos::validation_error_dto::BusinessValidationErrorDto;
use crate::entities::connectors::provider_transaction_import::IMPORT_CATEGORY_ID;
use crate::entities::transactions::transaction_types::create_transaction_from_dto;

pub struct SessionStaging {
    user_id: Uuid,
    lines: Vec<ImportParsedLineDto>,
    accounts: HashMap<(Uuid, String), Option<Uuid>>,
    assets: HashMap<String, i32>,
    left_out_assets: HashSet<String>,
    classifications: HashMap<Uuid, ImportLineClassificationDto>,
    currency_assets: HashSet<i32>,
}

pub struct SessionStage {
    pub staged: Vec<StagedTransactionDto>,
    pub unstageable: Vec<UnstageableGroupDto>,
    pub joined: HashMap<Uuid, [Uuid; 2]>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum GroupKey {
    Keyed(Uuid, String),
    Single(Uuid),
}

enum LineLink {
    Excluded,
    Blocked,
    Linked(Uuid),
}

type LinkedLine<'a> = (&'a ImportParsedLineDto, Uuid);

struct ExchangeSide {
    group: usize,
    account_id: Uuid,
    date: OffsetDateTime,
    asset_id: i32,
    incoming: bool,
}

#[derive(Default)]
struct KeyShare {
    lines: usize,
    own_line: bool,
    sum: Decimal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AssetKind {
    Currency,
    Security,
}

impl AssetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Currency => "currency",
            Self::Security => "security",
        }
    }
}

struct Leg {
    account_id: Uuid,
    asset_id: i32,
    kind: AssetKind,
    quantity: Decimal,
}

impl Leg {
    fn entry(&self) -> EntryDto {
        EntryDto::new(self.asset_id, self.account_id, self.quantity)
    }
}

impl SessionStaging {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            lines: Vec::new(),
            accounts: HashMap::new(),
            assets: HashMap::new(),
            left_out_assets: HashSet::new(),
            classifications: HashMap::new(),
            currency_assets: HashSet::new(),
        }
    }

    pub fn add_lines(&mut self, lines: Vec<ImportParsedLineDto>) {
        self.lines.extend(lines);
    }

    pub fn add_account_mappings(&mut self, mappings: &[ImportAccountMappingModel]) {
        self.accounts.extend(mappings.iter().map(|mapping| {
            (
                (mapping.file_id, mapping.account_ref.clone()),
                mapping.account_id,
            )
        }));
    }

    pub fn add_asset_mappings(&mut self, mappings: &[ImportAssetMappingModel]) {
        for mapping in mappings {
            match mapping.asset_id {
                Some(asset_id) => {
                    self.assets.insert(mapping.asset_ref.clone(), asset_id);
                }
                None => {
                    self.left_out_assets.insert(mapping.asset_ref.clone());
                }
            }
        }
    }

    pub fn add_classifications(&mut self, classifications: Vec<ImportLineClassificationDto>) {
        self.classifications.extend(
            classifications
                .into_iter()
                .map(|classification| (classification.parsed_line_id, classification)),
        );
    }

    pub fn add_currency_assets(&mut self, asset_ids: impl IntoIterator<Item = i32>) {
        self.currency_assets.extend(asset_ids);
    }

    pub fn stage(&self) -> anyhow::Result<SessionStage> {
        let left_out = |asset_ref: &String| self.left_out_assets.contains(asset_ref);
        let trimmed: HashMap<Uuid, ImportParsedLineDto> = self
            .lines
            .iter()
            .filter(|line| {
                line.legs.iter().any(|leg| left_out(&leg.asset_ref))
                    || line.related_asset_ref.as_ref().is_some_and(left_out)
            })
            .map(|line| {
                let mut line = line.clone();
                line.legs.retain(|leg| !left_out(&leg.asset_ref));
                if line.related_asset_ref.as_ref().is_some_and(left_out) && !has_main_leg(&line) {
                    for leg in &mut line.legs {
                        if leg.fee_type == Some(FeeEntryTypesDto::WithholdingTax) {
                            leg.fee_type = None;
                        }
                    }
                }
                line.related_asset_ref =
                    line.related_asset_ref.filter(|related| !left_out(related));
                (line.id, line)
            })
            .collect();
        let mut with_left_out_asset: HashSet<GroupKey> = trimmed.values().map(group_key).collect();
        let lines: Vec<&ImportParsedLineDto> = self
            .lines
            .iter()
            .map(|line| trimmed.get(&line.id).unwrap_or(line))
            .filter(|line| !(line.legs.is_empty() && trimmed.contains_key(&line.id)))
            .collect();

        let fee_rows = self.classified_fee_rows(&lines);
        let mut groups: BTreeMap<GroupKey, Vec<LinkedLine>> = BTreeMap::new();
        let mut blocked: HashSet<GroupKey> = HashSet::new();
        for line in lines {
            let line = fee_rows.get(&line.id).unwrap_or(line);
            let key = group_key(line);
            match self.link(line) {
                LineLink::Excluded => {}
                LineLink::Blocked => {
                    blocked.insert(key);
                }
                LineLink::Linked(account_id) => {
                    groups.entry(key).or_default().push((line, account_id))
                }
            }
        }
        groups.retain(|key, _| !blocked.contains(key));

        let groups = groups
            .into_iter()
            .flat_map(|(key, group)| {
                let transactions = self.transactions(group);
                if transactions.len() > 1 {
                    with_left_out_asset.remove(&key);
                }
                transactions
            })
            .collect();
        let mut groups = self.attach_withholding(self.pair_exchanges(groups));
        for group in &mut groups {
            group.sort_by_key(|(line, _)| line.id);
        }
        groups.sort_by_key(|group| (group[0].0.date, group[0].0.id));

        let mut stage = SessionStage {
            staged: Vec::new(),
            unstageable: Vec::new(),
            joined: HashMap::new(),
        };
        for group in groups {
            let involves_left_out_asset = group.iter().any(|(line, _)| {
                trimmed.contains_key(&line.id) || with_left_out_asset.contains(&group_key(line))
            });
            let reason = match self.build(&group, involves_left_out_asset) {
                Ok(None) => continue,
                Ok(Some(staged)) => match self.ledger_rejection(&staged.transaction)? {
                    None => {
                        stage.staged.push(staged);
                        continue;
                    }
                    Some(reason) => reason,
                },
                Err(reason) => reason,
            };
            stage.unstageable.push(unstageable(&group, reason));
        }
        Ok(stage)
    }

    fn ledger_rejection(&self, transaction: &TransactionDto) -> anyhow::Result<Option<String>> {
        let Err(error) = create_transaction_from_dto(transaction.clone(), self.user_id) else {
            return Ok(None);
        };
        let validation = error.downcast::<BusinessValidationErrorDto>()?;
        let mut reasons: Vec<String> = validation
            .errors
            .into_iter()
            .map(|error| {
                if error.message.contains(&error.field) {
                    error.message
                } else {
                    format!("{}: {}", error.field, error.message)
                }
            })
            .collect();
        reasons.dedup();
        Ok(Some(reasons.join(" ")))
    }

    fn link(&self, line: &ImportParsedLineDto) -> LineLink {
        match self.accounts.get(&(line.file_id, line.account_ref.clone())) {
            None => LineLink::Blocked,
            Some(None) => LineLink::Excluded,
            Some(Some(account_id)) => {
                let mut refs = line
                    .legs
                    .iter()
                    .map(|leg| leg.asset_ref.as_str())
                    .chain(line.related_asset_ref.as_deref());
                if refs.all(|asset_ref| self.assets.contains_key(asset_ref)) {
                    LineLink::Linked(*account_id)
                } else {
                    LineLink::Blocked
                }
            }
        }
    }

    fn line_type(&self, line: &ImportParsedLineDto) -> Option<ImportLineTypeDto> {
        self.classifications
            .get(&line.id)
            .map(|classification| classification.line_type)
    }

    fn classified_fee_rows(
        &self,
        lines: &[&ImportParsedLineDto],
    ) -> HashMap<Uuid, ImportParsedLineDto> {
        let mut keyed: HashMap<(Uuid, &str), Vec<&ImportParsedLineDto>> = HashMap::new();
        for line in lines.iter().copied() {
            if let Some(key) = line.transaction_key.as_deref() {
                keyed.entry((line.file_id, key)).or_default().push(line);
            }
        }
        let is_fee = |line: &ImportParsedLineDto| {
            self.line_type(line) == Some(ImportLineTypeDto::AccountFees)
        };
        let moved = |line: &ImportParsedLineDto| -> HashMap<(Option<Uuid>, Option<i32>), Decimal> {
            let account = self
                .accounts
                .get(&(line.file_id, line.account_ref.clone()))
                .copied()
                .flatten();
            let mut moved = HashMap::new();
            for leg in line.legs.iter().filter(|leg| leg.fee_type.is_none()) {
                *moved
                    .entry((account, self.assets.get(&leg.asset_ref).copied()))
                    .or_default() += leg.quantity;
            }
            moved
        };
        keyed
            .values()
            .flat_map(|lines| {
                let mut paid: HashMap<(Option<Uuid>, Option<i32>), Decimal> = HashMap::new();
                for line in lines.iter().filter(|line| !is_fee(line)) {
                    for (place, quantity) in moved(line) {
                        *paid.entry(place).or_default() += quantity;
                    }
                }
                lines.iter().filter(move |line| {
                    let moved = moved(line);
                    let beside_two = |(account, _): &(Option<Uuid>, Option<i32>)| {
                        paid.iter()
                            .filter(|((other, _), sum)| other == account && !sum.is_zero())
                            .count()
                            >= 2
                    };
                    is_fee(line)
                        && !moved.is_empty()
                        && moved.keys().all(|(_, asset)| {
                            asset.is_some_and(|asset| self.currency_assets.contains(&asset))
                        })
                        && (moved.keys().all(|place| paid.contains_key(place))
                            || moved.keys().all(beside_two))
                })
            })
            .map(|line| {
                let mut row = (*line).clone();
                for leg in &mut row.legs {
                    leg.fee_type.get_or_insert(FeeEntryTypesDto::Transaction);
                }
                (row.id, row)
            })
            .collect()
    }

    fn transactions<'a>(&self, group: Vec<LinkedLine<'a>>) -> Vec<Vec<LinkedLine<'a>>> {
        let mut shares: HashMap<(&str, i32), KeyShare> = HashMap::new();
        for (line, _) in &group {
            let mut moved: BTreeMap<i32, Decimal> = BTreeMap::new();
            for leg in line.legs.iter().filter(|leg| leg.fee_type.is_none()) {
                *moved.entry(self.assets[&leg.asset_ref]).or_default() += leg.quantity;
            }
            for (asset_id, quantity) in &moved {
                let share = shares
                    .entry((line.account_ref.as_str(), *asset_id))
                    .or_default();
                share.lines += 1;
                share.own_line |= moved.len() == 1;
                share.sum += quantity;
            }
        }
        let one_transaction = shares.iter().all(|(place, share)| match share.lines {
            1 => true,
            _ if share.sum.is_zero() => shares
                .iter()
                .any(|(other, rest)| other != place && !rest.sum.is_zero()),
            2 => share.own_line,
            _ => false,
        });
        if !one_transaction {
            return group.into_iter().map(|line| vec![line]).collect();
        }

        let one_transaction = match self.legs(&group, false).as_slice() {
            [a, b]
                if a.account_id != b.account_id
                    && a.kind == AssetKind::Currency
                    && b.kind == AssetKind::Currency =>
            {
                if a.asset_id == b.asset_id {
                    (a.quantity + b.quantity).is_zero()
                } else {
                    a.quantity.is_sign_negative() != b.quantity.is_sign_negative()
                        && group
                            .iter()
                            .filter(|(line, _)| has_main_leg(line))
                            .all(|(line, _)| {
                                self.line_type(line) == Some(ImportLineTypeDto::AssetTrade)
                            })
                }
            }
            legs => legs.iter().all(|leg| leg.account_id == legs[0].account_id),
        };
        if one_transaction {
            return vec![group];
        }
        let mut sides: BTreeMap<Uuid, Vec<LinkedLine>> = BTreeMap::new();
        for (line, account_id) in group {
            sides
                .entry(account_id)
                .or_default()
                .push((line, account_id));
        }
        sides.into_values().collect()
    }

    fn lone_exchange_side(&self, index: usize, group: &[LinkedLine]) -> Option<ExchangeSide> {
        let mut main_lines = group.iter().filter(|(line, _)| has_main_leg(line));
        let (Some(&(line, account_id)), None) = (main_lines.next(), main_lines.next()) else {
            return None;
        };
        if self.line_type(line) != Some(ImportLineTypeDto::AssetTrade)
            || line.date.to_offset(UtcOffset::UTC).time() == Time::MIDNIGHT
        {
            return None;
        }
        let mut sums: BTreeMap<i32, Decimal> = BTreeMap::new();
        for leg in line.legs.iter().filter(|leg| leg.fee_type.is_none()) {
            *sums.entry(self.assets[&leg.asset_ref]).or_default() += leg.quantity;
        }
        sums.retain(|_, quantity| !quantity.is_zero());
        let mut sums = sums.into_iter();
        let (Some((asset_id, quantity)), None) = (sums.next(), sums.next()) else {
            return None;
        };
        self.currency_assets
            .contains(&asset_id)
            .then_some(ExchangeSide {
                group: index,
                account_id,
                date: line.date,
                asset_id,
                incoming: quantity > Decimal::ZERO,
            })
    }

    fn pair_exchanges<'a>(&self, mut groups: Vec<Vec<LinkedLine<'a>>>) -> Vec<Vec<LinkedLine<'a>>> {
        let sides: Vec<ExchangeSide> = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| self.lone_exchange_side(index, group))
            .collect();
        let partners = |side: &ExchangeSide| -> Vec<usize> {
            sides
                .iter()
                .enumerate()
                .filter(|(_, other)| {
                    other.account_id == side.account_id
                        && other.date == side.date
                        && other.asset_id != side.asset_id
                        && other.incoming != side.incoming
                })
                .map(|(index, _)| index)
                .collect()
        };
        for (index, side) in sides.iter().enumerate() {
            let [partner] = partners(side)[..] else {
                continue;
            };
            if index < partner && partners(&sides[partner]) == [index] {
                let moved = std::mem::take(&mut groups[sides[partner].group]);
                groups[side.group].extend(moved);
            }
        }
        groups.retain(|group| !group.is_empty());
        groups
    }

    fn attach_withholding<'a>(
        &self,
        mut groups: Vec<Vec<LinkedLine<'a>>>,
    ) -> Vec<Vec<LinkedLine<'a>>> {
        let dividends: Vec<(usize, (Uuid, i32, Date))> = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                let line = main_line(group)?;
                let legs = self.legs(group, false);
                let paying_asset = self.paying_asset(line, &legs)?;
                Some((index, (legs[0].account_id, paying_asset, line.date.date())))
            })
            .collect();
        for index in 0..groups.len() {
            let [(line, account_id)] = groups[index][..] else {
                continue;
            };
            let Some(asset_ref) = &line.related_asset_ref else {
                continue;
            };
            if has_main_leg(line)
                || !line
                    .legs
                    .iter()
                    .any(|leg| leg.fee_type == Some(FeeEntryTypesDto::WithholdingTax))
            {
                continue;
            }
            let taxed = (account_id, self.assets[asset_ref], line.date.date());
            let mut paid = dividends.iter().filter(|(_, dividend)| *dividend == taxed);
            if let (Some((dividend, _)), None) = (paid.next(), paid.next()) {
                let moved = std::mem::take(&mut groups[index]);
                groups[*dividend].extend(moved);
            }
        }
        groups.retain(|group| !group.is_empty());
        groups
    }

    fn legs(&self, group: &[LinkedLine], fees: bool) -> Vec<Leg> {
        let mut sums: BTreeMap<(Uuid, i32), Decimal> = BTreeMap::new();
        for (line, account_id) in group {
            for leg in line
                .legs
                .iter()
                .filter(|leg| leg.fee_type.is_some() == fees)
            {
                *sums
                    .entry((*account_id, self.assets[&leg.asset_ref]))
                    .or_default() += leg.quantity;
            }
        }
        sums.into_iter()
            .filter(|(_, quantity)| !quantity.is_zero())
            .map(|((account_id, asset_id), quantity)| Leg {
                account_id,
                asset_id,
                kind: if self.currency_assets.contains(&asset_id) {
                    AssetKind::Currency
                } else {
                    AssetKind::Security
                },
                quantity: quantity.normalize(),
            })
            .collect()
    }

    fn paying_asset(&self, line: &ImportParsedLineDto, legs: &[Leg]) -> Option<i32> {
        match legs {
            [leg] if leg.kind == AssetKind::Currency && leg.quantity > Decimal::ZERO => line
                .related_asset_ref
                .as_ref()
                .map(|asset_ref| self.assets[asset_ref]),
            _ => None,
        }
    }

    fn build(
        &self,
        group: &[LinkedLine],
        involves_left_out_asset: bool,
    ) -> Result<Option<StagedTransactionDto>, String> {
        let mut legs = self.legs(group, false);
        let has_main_legs = !legs.is_empty();
        let mut fees: Vec<FeeEntryDto> = Vec::new();
        if has_main_legs {
            let mut fee_sums: BTreeMap<(Uuid, i32, i32), (FeeEntryTypesDto, Decimal)> =
                BTreeMap::new();
            for (line, account_id) in group {
                for leg in &line.legs {
                    if let Some(fee_type) = &leg.fee_type {
                        fee_sums
                            .entry((
                                *account_id,
                                self.assets[&leg.asset_ref],
                                fee_type.clone() as i32,
                            ))
                            .or_insert((fee_type.clone(), Decimal::ZERO))
                            .1 += leg.quantity
                    }
                }
            }
            fees = fee_sums
                .into_iter()
                .filter(|(_, (_, quantity))| !quantity.is_zero())
                .map(
                    |((account_id, asset_id, _), (entry_type, quantity))| FeeEntryDto {
                        entry: EntryDto::new(asset_id, account_id, quantity.normalize()),
                        entry_type,
                    },
                )
                .collect();
        } else {
            if group
                .iter()
                .flat_map(|(line, _)| &line.legs)
                .any(|leg| leg.fee_type == Some(FeeEntryTypesDto::WithholdingTax))
            {
                return Err("Withholding tax without a dividend.".to_string());
            }
            legs = self.legs(group, true);
            if legs.is_empty() {
                return Ok(None);
            }
            if legs.iter().any(|leg| leg.kind == AssetKind::Security) {
                return Err("A fee taken in units of a security.".to_string());
            }
        }

        let line = main_line(group).unwrap_or(group[0].0);
        let securities: Vec<&Leg> = legs
            .iter()
            .filter(|leg| leg.kind == AssetKind::Security)
            .collect();
        let ledger_type = match (legs.as_slice(), securities.as_slice()) {
            ([leg], [_]) if leg.quantity > Decimal::ZERO => {
                DatabaseTransactionTypes::AssetTransferIn
            }
            ([_], [_]) => DatabaseTransactionTypes::AssetTransferOut,
            ([leg], _) if !has_main_legs && leg.quantity > Decimal::ZERO => {
                DatabaseTransactionTypes::RegularTransaction
            }
            _ if !has_main_legs => DatabaseTransactionTypes::AccountFees,
            ([leg], []) => match self.line_type(line) {
                _ if self.paying_asset(line, &legs).is_some() => {
                    DatabaseTransactionTypes::CashDividend
                }
                _ if involves_left_out_asset && leg.quantity > Decimal::ZERO => {
                    DatabaseTransactionTypes::CashTransferIn
                }
                _ if involves_left_out_asset => DatabaseTransactionTypes::CashTransferOut,
                Some(ImportLineTypeDto::AccountFees) if leg.quantity < Decimal::ZERO => {
                    DatabaseTransactionTypes::AccountFees
                }
                Some(ImportLineTypeDto::Regular | ImportLineTypeDto::AccountFees) | None => {
                    DatabaseTransactionTypes::RegularTransaction
                }
                Some(_) if leg.quantity > Decimal::ZERO => DatabaseTransactionTypes::CashTransferIn,
                Some(_) => DatabaseTransactionTypes::CashTransferOut,
            },
            ([a, b], []) if a.asset_id == b.asset_id => {
                DatabaseTransactionTypes::CashBalanceTransfer
            }
            (_, [security]) if security.quantity > Decimal::ZERO => {
                DatabaseTransactionTypes::AssetPurchase
            }
            (_, [_]) => DatabaseTransactionTypes::AssetSale,
            _ => DatabaseTransactionTypes::AssetTrade,
        };

        let transaction_type = match ledger_type {
            DatabaseTransactionTypes::RegularTransaction => {
                TransactionTypeDto::Regular(RegularTransactionMetadataDto {
                    description: line.description.clone(),
                    entry: single_leg(&legs, AssetKind::Currency)?.entry(),
                    category_id: self
                        .classifications
                        .get(&line.id)
                        .and_then(|classification| classification.category_id)
                        .unwrap_or(IMPORT_CATEGORY_ID),
                })
            }
            DatabaseTransactionTypes::CashTransferIn => {
                TransactionTypeDto::CashTransferIn(CashTransferInMetadataDto {
                    entry: single_leg(&legs, AssetKind::Currency)?.entry(),
                })
            }
            DatabaseTransactionTypes::CashTransferOut => {
                TransactionTypeDto::CashTransferOut(CashTransferOutMetadataDto {
                    entry: single_leg(&legs, AssetKind::Currency)?.entry(),
                })
            }
            DatabaseTransactionTypes::AccountFees => {
                TransactionTypeDto::AccountFees(AccountFeesMetadataDto {
                    entry: single_leg(&legs, AssetKind::Currency)?.entry(),
                })
            }
            DatabaseTransactionTypes::CashDividend => {
                TransactionTypeDto::CashDividend(CashDividendMetadataDto {
                    entry: single_leg(&legs, AssetKind::Currency)?.entry(),
                    origin_asset_id: self
                        .paying_asset(line, &legs)
                        .ok_or("A dividend needs the asset that paid it.".to_string())?,
                })
            }
            DatabaseTransactionTypes::AssetTransferIn => {
                TransactionTypeDto::AssetTransferIn(AssetTransferInMetadataDto {
                    entry: single_leg(&legs, AssetKind::Security)?.entry(),
                })
            }
            DatabaseTransactionTypes::AssetTransferOut => {
                TransactionTypeDto::AssetTransferOut(AssetTransferOutMetadataDto {
                    entry: single_leg(&legs, AssetKind::Security)?.entry(),
                })
            }
            DatabaseTransactionTypes::AssetPurchase => {
                let (security, cash) = security_and_cash(&legs)?;
                TransactionTypeDto::AssetPurchase(AssetPurchaseMetadataDto {
                    purchase: security.entry(),
                    sale: cash.entry(),
                })
            }
            DatabaseTransactionTypes::AssetSale => {
                let (security, cash) = security_and_cash(&legs)?;
                TransactionTypeDto::AssetSale(AssetSaleMetadataDto {
                    sale: security.entry(),
                    proceeds: cash.entry(),
                })
            }
            DatabaseTransactionTypes::AssetTrade => {
                let (outgoing, incoming) = out_and_in(&legs)?;
                TransactionTypeDto::AssetTrade(AssetTradeMetadataDto {
                    outgoing_entry: outgoing.entry(),
                    incoming_entry: incoming.entry(),
                })
            }
            DatabaseTransactionTypes::CashBalanceTransfer => {
                let (outgoing, incoming) = out_and_in(&legs)?;
                TransactionTypeDto::CashBalanceTransfer(CashBalanceTransferMetadataDto {
                    outgoing_change: outgoing.entry(),
                    incoming_change: incoming.entry(),
                })
            }
            DatabaseTransactionTypes::AssetDividend
            | DatabaseTransactionTypes::AssetBalanceTransfer => {
                return Err(format!("{ledger_type:?} can't be imported yet."))
            }
        };
        Ok(Some(StagedTransactionDto {
            id: staged_id(group),
            file_ids: file_ids(group),
            transaction: TransactionDto {
                transaction_id: None,
                date: line.date,
                visibility: TransactionVisibilityDto::Default,
                fee_entries: fees,
                transaction_type,
            },
            movements: self.movements(group, has_main_legs),
        }))
    }

    fn movements(&self, group: &[LinkedLine], has_main_legs: bool) -> Vec<StagedMovementDto> {
        let mut movements: BTreeMap<(Uuid, i32), StagedMovementDto> = BTreeMap::new();
        let mut with_main_leg: HashSet<(Uuid, i32)> = HashSet::new();
        let mut per_line: HashMap<(Uuid, i32), BTreeMap<Uuid, Decimal>> = HashMap::new();
        for (line, account_id) in group {
            for leg in &line.legs {
                let key = (*account_id, self.assets[&leg.asset_ref]);
                let movement = movements.entry(key).or_insert_with(|| StagedMovementDto {
                    account_id: key.0,
                    asset_id: key.1,
                    quantity: Decimal::ZERO,
                    file_id: line.file_id,
                    date: line.date.date(),
                    description: line.description.clone(),
                    itemised_fee: None,
                });
                movement.quantity += leg.quantity;
                *per_line.entry(key).or_default().entry(line.id).or_default() += leg.quantity;
                if leg.fee_type.is_some() && has_main_legs {
                    *movement.itemised_fee.get_or_insert(Decimal::ZERO) += leg.quantity;
                } else if with_main_leg.insert(key) {
                    movement.file_id = line.file_id;
                    movement.date = line.date.date();
                    movement.description = line.description.clone();
                }
            }
        }
        movements
            .into_iter()
            .filter(|(_, movement)| !movement.quantity.is_zero())
            .map(|(key, mut movement)| {
                if !with_main_leg.contains(&key) {
                    movement.itemised_fee = None;
                }
                if with_main_leg.contains(&key) {
                    movement.itemised_fee = movement
                        .itemised_fee
                        .or_else(|| unmarked_fee(&per_line[&key], movement.quantity));
                }
                movement.quantity = movement.quantity.normalize();
                movement.itemised_fee = movement.itemised_fee.map(|fee| fee.normalize());
                movement
            })
            .collect()
    }
}

fn unmarked_fee(lines: &BTreeMap<Uuid, Decimal>, total: Decimal) -> Option<Decimal> {
    if lines.len() != 2 {
        return None;
    }
    let largest = lines
        .values()
        .copied()
        .max_by_key(|quantity| quantity.abs())?;
    Some(total - largest)
}

fn group_key(line: &ImportParsedLineDto) -> GroupKey {
    match &line.transaction_key {
        Some(key) => GroupKey::Keyed(line.file_id, key.clone()),
        None => GroupKey::Single(line.id),
    }
}

fn has_main_leg(line: &ImportParsedLineDto) -> bool {
    line.legs.iter().any(|leg| leg.fee_type.is_none())
}

fn main_line<'a>(group: &[LinkedLine<'a>]) -> Option<&'a ImportParsedLineDto> {
    group
        .iter()
        .map(|(line, _)| *line)
        .filter(|line| has_main_leg(line))
        .rev()
        .max_by_key(|line| {
            line.legs
                .iter()
                .filter(|leg| leg.fee_type.is_none())
                .map(|leg| leg.quantity.abs())
                .sum::<Decimal>()
        })
}

fn single_leg(legs: &[Leg], kind: AssetKind) -> Result<&Leg, String> {
    match legs {
        [leg] if leg.kind == kind => Ok(leg),
        _ => {
            let matching = legs.iter().filter(|leg| leg.kind == kind).count();
            let kind = kind.as_str();
            Err(format!(
                "Expected a single {kind} leg, found {matching} {kind} and {} other.",
                legs.len() - matching
            ))
        }
    }
}

fn security_and_cash(legs: &[Leg]) -> Result<(&Leg, &Leg), String> {
    match legs {
        [a, b] if a.kind == AssetKind::Security && b.kind == AssetKind::Currency => Ok((a, b)),
        [a, b] if a.kind == AssetKind::Currency && b.kind == AssetKind::Security => Ok((b, a)),
        _ => {
            let currencies = legs
                .iter()
                .filter(|leg| leg.kind == AssetKind::Currency)
                .count();
            Err(format!(
                "Expected a security leg and a currency leg, found {} security and {currencies} currency.",
                legs.len() - currencies
            ))
        }
    }
}

fn out_and_in(legs: &[Leg]) -> Result<(&Leg, &Leg), String> {
    match legs {
        [a, b] if a.quantity < Decimal::ZERO && b.quantity > Decimal::ZERO => Ok((a, b)),
        [a, b] if b.quantity < Decimal::ZERO && a.quantity > Decimal::ZERO => Ok((b, a)),
        _ => {
            let outgoing = legs
                .iter()
                .filter(|leg| leg.quantity < Decimal::ZERO)
                .count();
            Err(format!(
                "Expected one leg out and one leg in, found {outgoing} out and {} in.",
                legs.len() - outgoing
            ))
        }
    }
}

fn staged_id(group: &[LinkedLine]) -> Uuid {
    let mut ids: Vec<Uuid> = group.iter().map(|(line, _)| line.id).collect();
    ids.sort();
    let name: Vec<u8> = ids.iter().flat_map(|id| *id.as_bytes()).collect();
    Uuid::new_v5(&Uuid::NAMESPACE_OID, &name)
}

fn file_ids(group: &[LinkedLine]) -> Vec<Uuid> {
    group
        .iter()
        .map(|(line, _)| line.file_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unstageable(group: &[LinkedLine], reason: String) -> UnstageableGroupDto {
    let line = main_line(group).unwrap_or(group[0].0);
    UnstageableGroupDto {
        id: staged_id(group),
        file_ids: file_ids(group),
        date: line.date,
        description: line.description.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::dtos::imports::import_classification_dto::ImportParsedLegDto;
    use crate::test_support::load_dynamic_enums;

    const CURRENT: Uuid = Uuid::from_u128(1);
    const SAVINGS: Uuid = Uuid::from_u128(2);
    const GBP: i32 = 10;
    const EUR: i32 = 11;
    const VUSA: i32 = 20;
    const VWRL: i32 = 21;
    const GROCERIES: i32 = 25;

    fn leg(asset_ref: &str, quantity: &str) -> ImportParsedLegDto {
        ImportParsedLegDto {
            asset_ref: asset_ref.to_string(),
            quantity: quantity.parse().unwrap(),
            fee_type: None,
        }
    }

    fn fee(asset_ref: &str, quantity: &str, fee_type: FeeEntryTypesDto) -> ImportParsedLegDto {
        ImportParsedLegDto {
            fee_type: Some(fee_type),
            ..leg(asset_ref, quantity)
        }
    }

    fn line(
        file_id: Uuid,
        account_ref: &str,
        key: Option<&str>,
        description: &str,
        legs: Vec<ImportParsedLegDto>,
    ) -> ImportParsedLineDto {
        ImportParsedLineDto {
            id: Uuid::now_v7(),
            file_id,
            transaction_key: key.map(str::to_string),
            account_ref: account_ref.to_string(),
            date: datetime!(2026-02-01 10:00 UTC),
            description: Some(description.to_string()),
            related_asset_ref: None,
            legs,
        }
    }

    fn typed(
        line: &ImportParsedLineDto,
        line_type: ImportLineTypeDto,
    ) -> ImportLineClassificationDto {
        ImportLineClassificationDto {
            parsed_line_id: line.id,
            line_type,
            category_id: None,
        }
    }

    fn account(
        file_id: Uuid,
        account_ref: &str,
        account_id: Option<Uuid>,
    ) -> ImportAccountMappingModel {
        ImportAccountMappingModel {
            file_id,
            account_ref: account_ref.to_string(),
            account_id,
        }
    }

    fn asset(asset_ref: &str, asset_id: i32) -> ImportAssetMappingModel {
        ImportAssetMappingModel {
            asset_ref: asset_ref.to_string(),
            asset_id: Some(asset_id),
        }
    }

    fn left_out(asset_ref: &str) -> ImportAssetMappingModel {
        ImportAssetMappingModel {
            asset_ref: asset_ref.to_string(),
            asset_id: None,
        }
    }

    fn staging(file_id: Uuid, lines: Vec<ImportParsedLineDto>) -> SessionStaging {
        load_dynamic_enums();
        let mut staging = SessionStaging::new(Uuid::nil());
        staging.add_account_mappings(&[
            account(file_id, "Current", Some(CURRENT)),
            account(file_id, "Savings", Some(SAVINGS)),
            account(file_id, "Pocket", None),
        ]);
        staging.add_asset_mappings(&[
            asset("GBP", GBP),
            asset("EUR", EUR),
            asset("VUSA", VUSA),
            asset("VWRL", VWRL),
        ]);
        staging.add_currency_assets([GBP, EUR]);
        staging.add_lines(lines);
        staging
    }

    fn content(staged: &StagedTransactionDto) -> String {
        format!("{:?}", staged.transaction)
    }

    type Shape = (&'static str, Vec<(Uuid, i32, Decimal)>);

    fn shape(name: &'static str, entries: &[(Uuid, i32, &str)]) -> Shape {
        let entries = entries
            .iter()
            .map(|(account_id, asset_id, quantity)| {
                (*account_id, *asset_id, quantity.parse().unwrap())
            })
            .collect();
        (name, entries)
    }

    fn shapes(stage: &SessionStage) -> Vec<Shape> {
        let entry = |entry: &EntryDto| (entry.account_id, entry.asset_id, entry.quantity);
        stage
            .staged
            .iter()
            .map(|row| match &row.transaction.transaction_type {
                TransactionTypeDto::Regular(regular) => ("regular", vec![entry(&regular.entry)]),
                TransactionTypeDto::CashTransferIn(transfer) => {
                    ("cash transfer in", vec![entry(&transfer.entry)])
                }
                TransactionTypeDto::CashTransferOut(transfer) => {
                    ("cash transfer out", vec![entry(&transfer.entry)])
                }
                TransactionTypeDto::AccountFees(fees) => ("account fees", vec![entry(&fees.entry)]),
                TransactionTypeDto::CashDividend(dividend) => {
                    ("cash dividend", vec![entry(&dividend.entry)])
                }
                TransactionTypeDto::AssetTransferIn(transfer) => {
                    ("asset transfer in", vec![entry(&transfer.entry)])
                }
                TransactionTypeDto::AssetTransferOut(transfer) => {
                    ("asset transfer out", vec![entry(&transfer.entry)])
                }
                TransactionTypeDto::AssetPurchase(purchase) => (
                    "asset purchase",
                    vec![entry(&purchase.purchase), entry(&purchase.sale)],
                ),
                TransactionTypeDto::AssetSale(sale) => {
                    ("asset sale", vec![entry(&sale.sale), entry(&sale.proceeds)])
                }
                TransactionTypeDto::AssetTrade(trade) => (
                    "asset trade",
                    vec![entry(&trade.outgoing_entry), entry(&trade.incoming_entry)],
                ),
                TransactionTypeDto::CashBalanceTransfer(transfer) => (
                    "cash balance transfer",
                    vec![
                        entry(&transfer.outgoing_change),
                        entry(&transfer.incoming_change),
                    ],
                ),
                other => panic!("the import never stages {other:?}"),
            })
            .collect()
    }

    fn first_staged(staging: SessionStaging) -> StagedTransactionDto {
        staging.stage().unwrap().staged.remove(0)
    }

    #[test]
    fn fees_of_one_kind_sum_into_one_entry_and_a_fee_only_group_is_an_account_fee() {
        let file = Uuid::now_v7();
        let payment = line(
            file,
            "Current",
            None,
            "Card",
            vec![
                leg("GBP", "-20"),
                fee("GBP", "-0.50", FeeEntryTypesDto::Transaction),
                fee("GBP", "-0.25", FeeEntryTypesDto::Transaction),
            ],
        );
        let monthly = line(
            file,
            "Current",
            None,
            "Monthly fee",
            vec![fee("GBP", "-2", FeeEntryTypesDto::Transaction)],
        );
        let withholding = line(
            file,
            "Current",
            None,
            "Tax",
            vec![fee("GBP", "-0.30", FeeEntryTypesDto::WithholdingTax)],
        );
        let stage = staging(file, vec![payment, monthly, withholding])
            .stage()
            .unwrap();

        let fees: Vec<_> = stage
            .staged
            .iter()
            .map(|row| row.transaction.fee_entries.len())
            .collect();
        assert_eq!(fees, vec![1, 0]);
        assert_eq!(
            stage.staged[0].transaction.fee_entries[0].entry.quantity,
            "-0.75".parse().unwrap()
        );
        assert!(matches!(
            stage.staged[1].transaction.transaction_type,
            TransactionTypeDto::AccountFees(_)
        ));
        assert_eq!(stage.unstageable.len(), 1);
        assert_eq!(
            stage.unstageable[0].reason,
            "Withholding tax without a dividend."
        );
    }

    #[test]
    fn exchange_sides_pair_into_a_trade_and_a_lone_side_is_a_transfer() {
        let gbp_file = Uuid::now_v7();
        let eur_file = Uuid::now_v7();
        let out = line(
            gbp_file,
            "Current",
            None,
            "Exchanged to EUR",
            vec![leg("GBP", "-100")],
        );
        let back = line(
            eur_file,
            "Current",
            None,
            "Exchanged from GBP",
            vec![leg("EUR", "115")],
        );
        let mut lone = line(
            gbp_file,
            "Current",
            None,
            "Exchanged to EUR",
            vec![leg("GBP", "-50")],
        );
        lone.date = datetime!(2026-02-03 9:00 UTC);
        let mut staging = staging(gbp_file, vec![out.clone(), back.clone(), lone.clone()]);
        staging.add_account_mappings(&[account(eur_file, "Current", Some(CURRENT))]);
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::AssetTrade),
            typed(&back, ImportLineTypeDto::AssetTrade),
            typed(&lone, ImportLineTypeDto::AssetTrade),
        ]);

        let stage = staging.stage().unwrap();
        assert_eq!(stage.staged.len(), 2);
        assert!(matches!(
            stage.staged[0].transaction.transaction_type,
            TransactionTypeDto::AssetTrade(_)
        ));
        assert_eq!(stage.staged[0].file_ids.len(), 2);
        assert!(matches!(
            stage.staged[1].transaction.transaction_type,
            TransactionTypeDto::CashTransferOut(_)
        ));
    }

    #[test]
    fn an_ambiguous_exchange_stays_lone() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            None,
            "Exchanged to EUR",
            vec![leg("GBP", "-100")],
        );
        let first = line(file, "Current", None, "Exchanged", vec![leg("EUR", "115")]);
        let second = line(file, "Current", None, "Exchanged", vec![leg("EUR", "57")]);
        let mut staging = staging(file, vec![out.clone(), first.clone(), second.clone()]);
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::AssetTrade),
            typed(&first, ImportLineTypeDto::AssetTrade),
            typed(&second, ImportLineTypeDto::AssetTrade),
        ]);

        let stage = staging.stage().unwrap();
        let [out, first, second] = stage.staged.as_slice() else {
            panic!("expected three staged transactions");
        };
        assert!(matches!(
            out.transaction.transaction_type,
            TransactionTypeDto::CashTransferOut(_)
        ));
        assert!(matches!(
            first.transaction.transaction_type,
            TransactionTypeDto::CashTransferIn(_)
        ));
        assert!(matches!(
            second.transaction.transaction_type,
            TransactionTypeDto::CashTransferIn(_)
        ));
    }

    #[test]
    fn exchange_sides_without_a_printed_time_or_at_different_times_stay_lone() {
        let file = Uuid::now_v7();
        let stage_at = |out_date: OffsetDateTime, back_date: OffsetDateTime| {
            let mut out = line(file, "Current", None, "Exchanged", vec![leg("GBP", "-100")]);
            out.date = out_date;
            let mut back = line(file, "Current", None, "Exchanged", vec![leg("EUR", "115")]);
            back.date = back_date;
            let mut staging = staging(file, vec![out.clone(), back.clone()]);
            staging.add_classifications(vec![
                typed(&out, ImportLineTypeDto::AssetTrade),
                typed(&back, ImportLineTypeDto::AssetTrade),
            ]);
            staging.stage().unwrap()
        };

        for stage in [
            stage_at(
                datetime!(2026-02-01 0:00 UTC),
                datetime!(2026-02-01 0:00 UTC),
            ),
            stage_at(
                datetime!(2026-02-01 10:00 UTC),
                datetime!(2026-02-01 10:05 UTC),
            ),
        ] {
            let [out, back] = stage.staged.as_slice() else {
                panic!("expected two staged transactions");
            };
            assert!(matches!(
                out.transaction.transaction_type,
                TransactionTypeDto::CashTransferOut(_)
            ));
            assert!(matches!(
                back.transaction.transaction_type,
                TransactionTypeDto::CashTransferIn(_)
            ));
        }
    }

    #[test]
    fn a_fee_row_keyed_to_an_exchange_side_pairs_like_an_inline_fee() {
        let file = Uuid::now_v7();
        let inline_out = line(
            file,
            "Current",
            None,
            "Exchanged to EUR",
            vec![
                leg("GBP", "-100"),
                fee("GBP", "-1", FeeEntryTypesDto::Exchange),
            ],
        );
        let keyed_out = line(
            file,
            "Current",
            Some("x1"),
            "Exchanged to EUR",
            vec![leg("GBP", "-100")],
        );
        let fee_row = line(
            file,
            "Current",
            Some("x1"),
            "Exchange fee",
            vec![fee("GBP", "-1", FeeEntryTypesDto::Exchange)],
        );
        let back = line(
            file,
            "Current",
            None,
            "Exchanged from GBP",
            vec![leg("EUR", "115")],
        );
        let mut one = staging(file, vec![inline_out.clone(), back.clone()]);
        one.add_classifications(vec![
            typed(&inline_out, ImportLineTypeDto::AssetTrade),
            typed(&back, ImportLineTypeDto::AssetTrade),
        ]);
        let mut two = staging(file, vec![keyed_out.clone(), fee_row, back.clone()]);
        two.add_classifications(vec![
            typed(&keyed_out, ImportLineTypeDto::AssetTrade),
            typed(&back, ImportLineTypeDto::AssetTrade),
        ]);

        let one = first_staged(one);
        let two = two.stage().unwrap();

        let [two] = two.staged.as_slice() else {
            panic!("expected one staged transaction");
        };
        assert!(matches!(
            two.transaction.transaction_type,
            TransactionTypeDto::AssetTrade(_)
        ));
        assert_eq!(two.transaction.fee_entries.len(), 1);
        assert_eq!(content(&one), content(&two));
    }

    #[test]
    fn a_blocked_line_holds_back_its_group_and_a_dont_import_line_is_dropped() {
        let file = Uuid::now_v7();
        let trade = line(
            file,
            "Current",
            Some("t1"),
            "Buy ZZZQ",
            vec![leg("GBP", "-10"), leg("ZZZQ", "1")],
        );
        let fee_row = line(
            file,
            "Current",
            Some("t1"),
            "Fee",
            vec![fee("GBP", "-1", FeeEntryTypesDto::Transaction)],
        );
        let pocket = line(file, "Pocket", None, "Pocket", vec![leg("GBP", "-3")]);
        let stage = staging(file, vec![trade, fee_row, pocket]).stage().unwrap();

        assert!(stage.staged.is_empty());
        assert!(stage.unstageable.is_empty());
    }

    #[test]
    fn a_dont_import_line_leaves_the_rest_of_its_group() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To pocket",
            vec![leg("GBP", "-100")],
        );
        let pocket = line(
            file,
            "Pocket",
            Some("t1"),
            "From current",
            vec![leg("GBP", "100")],
        );
        let mut staging = staging(file, vec![out.clone(), pocket.clone()]);
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::CashTransferOut),
            typed(&pocket, ImportLineTypeDto::CashTransferIn),
        ]);

        let stage = staging.stage().unwrap();
        let [row] = stage.staged.as_slice() else {
            panic!("expected one staged transaction");
        };
        match &row.transaction.transaction_type {
            TransactionTypeDto::CashTransferOut(metadata) => assert_eq!(
                (metadata.entry.account_id, metadata.entry.quantity),
                (CURRENT, Decimal::from(-100))
            ),
            other => panic!("expected a transfer out, got {other:?}"),
        }
    }

    #[test]
    fn cash_paid_for_or_received_from_a_left_out_asset_is_a_transfer() {
        let file = Uuid::now_v7();
        let buy = line(
            file,
            "Current",
            None,
            "Buy SPX500",
            vec![
                leg("GBP", "-550"),
                leg("SPX500", "1"),
                fee("GBP", "-1", FeeEntryTypesDto::Transaction),
            ],
        );
        let sell = line(
            file,
            "Current",
            None,
            "Sell SPX500",
            vec![leg("GBP", "560"), leg("SPX500", "-1")],
        );
        let mut payout = line(file, "Current", None, "Adjustment", vec![leg("GBP", "2")]);
        payout.related_asset_ref = Some("SPX500".to_string());
        let mut staging = staging(file, vec![buy.clone(), sell, payout]);
        staging.add_asset_mappings(&[left_out("SPX500")]);
        staging.add_classifications(vec![typed(&buy, ImportLineTypeDto::Regular)]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("cash transfer out", &[(CURRENT, GBP, "-550")]),
                shape("cash transfer in", &[(CURRENT, GBP, "560")]),
                shape("cash transfer in", &[(CURRENT, GBP, "2")]),
            ]
        );
        let bought = &stage.staged[0];
        assert_eq!(
            bought.transaction.fee_entries[0].entry.quantity,
            Decimal::from(-1)
        );
        let [movement] = bought.movements.as_slice() else {
            panic!("expected one movement");
        };
        assert_eq!(
            (movement.asset_id, movement.quantity, movement.itemised_fee),
            (GBP, Decimal::from(-551), Some(Decimal::from(-1)))
        );
    }

    #[test]
    fn payments_sharing_a_reference_with_a_left_out_trade_keep_their_own_types() {
        let file = Uuid::now_v7();
        let payment = |description: &str, amount: &str| {
            line(
                file,
                "Current",
                Some("NOTPROVIDED"),
                description,
                vec![leg("GBP", amount)],
            )
        };
        let (tesco, shell) = (payment("TESCO", "-50"), payment("SHELL", "-20"));
        let buy = line(
            file,
            "Current",
            Some("NOTPROVIDED"),
            "Buy SPX500",
            vec![leg("GBP", "-300"), leg("SPX500", "3")],
        );
        let mut staging = staging(file, vec![tesco.clone(), shell.clone(), buy]);
        staging.add_asset_mappings(&[left_out("SPX500")]);
        staging.add_classifications(vec![
            typed(&tesco, ImportLineTypeDto::Regular),
            typed(&shell, ImportLineTypeDto::Regular),
        ]);

        let stage = staging.stage().unwrap();
        assert_eq!(
            shapes(&stage),
            vec![
                shape("regular", &[(CURRENT, GBP, "-50")]),
                shape("regular", &[(CURRENT, GBP, "-20")]),
                shape("cash transfer out", &[(CURRENT, GBP, "-300")]),
            ]
        );
    }

    #[test]
    fn tax_withheld_on_a_left_out_assets_payout_is_a_transfer_out() {
        let file = Uuid::now_v7();
        let mut payout = line(file, "Current", None, "Dividend", vec![leg("GBP", "10")]);
        payout.related_asset_ref = Some("SPX500".to_string());
        let mut tax = line(
            file,
            "Current",
            None,
            "Tax",
            vec![fee("GBP", "-1.50", FeeEntryTypesDto::WithholdingTax)],
        );
        tax.related_asset_ref = Some("SPX500".to_string());
        let mut staging = staging(file, vec![payout, tax]);
        staging.add_asset_mappings(&[left_out("SPX500")]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("cash transfer in", &[(CURRENT, GBP, "10")]),
                shape("cash transfer out", &[(CURRENT, GBP, "-1.5")]),
            ]
        );
    }

    #[test]
    fn a_line_moving_only_a_left_out_asset_disappears_and_holds_nothing_back() {
        let file = Uuid::now_v7();
        let units = line(
            file,
            "Unanswered",
            Some("t1"),
            "Buy SPX500",
            vec![leg("SPX500", "1")],
        );
        let cash = line(
            file,
            "Current",
            Some("t1"),
            "Transaction Related",
            vec![leg("GBP", "-550")],
        );
        let held = line(file, "Current", None, "Position", vec![leg("SPX500", "3")]);
        let mut staging = staging(file, vec![units, cash, held]);
        staging.add_asset_mappings(&[left_out("SPX500")]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![shape("cash transfer out", &[(CURRENT, GBP, "-550")])]
        );
    }

    #[test]
    fn one_key_in_two_files_is_two_groups() {
        let first_file = Uuid::now_v7();
        let second_file = Uuid::now_v7();
        let coffee = line(
            first_file,
            "Current",
            Some("t1"),
            "Coffee",
            vec![leg("GBP", "-3")],
        );
        let lunch = line(
            second_file,
            "Current",
            Some("t1"),
            "Lunch",
            vec![leg("GBP", "-9")],
        );
        let mut staging = staging(first_file, vec![coffee, lunch]);
        staging.add_account_mappings(&[account(second_file, "Current", Some(CURRENT))]);

        assert_eq!(staging.stage().unwrap().staged.len(), 2);
    }

    #[test]
    fn a_keyed_transfer_is_one_balance_transfer_whatever_its_lines_were_called() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To savings",
            vec![leg("GBP", "-100")],
        );
        let into = line(
            file,
            "Savings",
            Some("t1"),
            "From current",
            vec![leg("GBP", "100")],
        );
        let goal = line(
            file,
            "Current",
            Some("t2"),
            "Savings goal",
            vec![leg("GBP", "-40")],
        );
        let saved = line(
            file,
            "Savings",
            Some("t2"),
            "From current",
            vec![leg("GBP", "40")],
        );
        let mut staging = staging(
            file,
            vec![out.clone(), into.clone(), goal.clone(), saved.clone()],
        );
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::CashTransferOut),
            typed(&into, ImportLineTypeDto::CashTransferIn),
            typed(&goal, ImportLineTypeDto::Regular),
            typed(&saved, ImportLineTypeDto::CashTransferIn),
        ]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape(
                    "cash balance transfer",
                    &[(CURRENT, GBP, "-100"), (SAVINGS, GBP, "100")]
                ),
                shape(
                    "cash balance transfer",
                    &[(CURRENT, GBP, "-40"), (SAVINGS, GBP, "40")]
                ),
            ]
        );
    }

    #[test]
    fn a_keyed_transfer_that_is_not_one_amount_stages_each_account_by_itself() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To savings",
            vec![leg("GBP", "-100")],
        );
        let short = line(
            file,
            "Savings",
            Some("t1"),
            "From current",
            vec![leg("GBP", "99")],
        );
        let charge = line(
            file,
            "Savings",
            Some("t1"),
            "Transfer charge",
            vec![fee("GBP", "-0.50", FeeEntryTypesDto::Transaction)],
        );
        let fx_out = line(
            file,
            "Current",
            Some("t2"),
            "To savings",
            vec![leg("GBP", "-100")],
        );
        let fx_in = line(
            file,
            "Savings",
            Some("t2"),
            "From current",
            vec![leg("EUR", "115")],
        );
        let mut staging = staging(
            file,
            vec![
                out.clone(),
                short.clone(),
                charge,
                fx_out.clone(),
                fx_in.clone(),
            ],
        );
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::CashTransferOut),
            typed(&short, ImportLineTypeDto::CashTransferIn),
            typed(&fx_out, ImportLineTypeDto::CashTransferOut),
            typed(&fx_in, ImportLineTypeDto::CashTransferIn),
        ]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("cash transfer out", &[(CURRENT, GBP, "-100")]),
                shape("cash transfer in", &[(SAVINGS, GBP, "99")]),
                shape("cash transfer out", &[(CURRENT, GBP, "-100")]),
                shape("cash transfer in", &[(SAVINGS, EUR, "115")]),
            ]
        );
        let fees: Vec<usize> = stage
            .staged
            .iter()
            .map(|row| row.transaction.fee_entries.len())
            .collect();
        assert_eq!(fees, vec![0, 1, 0, 0]);
        assert_eq!(
            stage.staged[1].transaction.fee_entries[0].entry.account_id,
            SAVINGS
        );
    }

    #[test]
    fn a_transfer_between_two_names_of_one_account_stages_nothing() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To pot",
            vec![leg("GBP", "-100")],
        );
        let into = line(
            file,
            "Pot",
            Some("t1"),
            "From current",
            vec![leg("GBP", "100")],
        );
        let mut staging = staging(file, vec![out.clone(), into.clone()]);
        staging.add_account_mappings(&[account(file, "Pot", Some(CURRENT))]);
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::CashTransferOut),
            typed(&into, ImportLineTypeDto::CashTransferIn),
        ]);

        let stage = staging.stage().unwrap();
        assert!(stage.staged.is_empty());
        assert!(stage.unstageable.is_empty());
    }

    #[test]
    fn a_fee_on_a_transfer_that_cancels_inside_one_account_is_an_account_fee() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To pot",
            vec![leg("GBP", "-100")],
        );
        let into = line(
            file,
            "Pot",
            Some("t1"),
            "From current",
            vec![leg("GBP", "100")],
        );
        let charge = line(
            file,
            "Current",
            Some("t1"),
            "Transfer charge",
            vec![fee("GBP", "-0.50", FeeEntryTypesDto::Transaction)],
        );
        let mut staging = staging(file, vec![out, into, charge]);
        staging.add_account_mappings(&[account(file, "Pot", Some(CURRENT))]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![shape("account fees", &[(CURRENT, GBP, "-0.50")])]
        );
        assert_eq!(
            stage.staged[0].movements[0].quantity,
            "-0.50".parse().unwrap()
        );
    }

    #[test]
    fn two_currencies_are_a_trade_on_one_account_and_across_two_only_as_a_named_exchange() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "To EUR balance",
            vec![leg("GBP", "-100")],
        );
        let into = line(
            file,
            "Pot",
            Some("t1"),
            "From GBP balance",
            vec![leg("EUR", "115")],
        );
        let sent = line(
            file,
            "Current",
            Some("t2"),
            "To EUR account",
            vec![leg("GBP", "-50")],
        );
        let arrived = line(
            file,
            "Savings",
            Some("t2"),
            "From GBP account",
            vec![leg("EUR", "57")],
        );
        let mut staging = staging(
            file,
            vec![out.clone(), into.clone(), sent.clone(), arrived.clone()],
        );
        staging.add_account_mappings(&[account(file, "Pot", Some(CURRENT))]);
        staging.add_classifications(vec![
            typed(&out, ImportLineTypeDto::CashTransferOut),
            typed(&into, ImportLineTypeDto::CashTransferIn),
            typed(&sent, ImportLineTypeDto::AssetTrade),
            typed(&arrived, ImportLineTypeDto::AssetTrade),
        ]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape(
                    "asset trade",
                    &[(CURRENT, GBP, "-100"), (CURRENT, EUR, "115")]
                ),
                shape(
                    "asset trade",
                    &[(CURRENT, GBP, "-50"), (SAVINGS, EUR, "57")]
                ),
            ]
        );
    }

    #[test]
    fn money_in_that_names_a_paying_asset_is_a_dividend() {
        let file = Uuid::now_v7();
        let mut dividend = line(file, "Current", None, "Dividend", vec![leg("GBP", "4.20")]);
        dividend.related_asset_ref = Some("VUSA".to_string());
        let mut reversal = line(
            file,
            "Current",
            None,
            "Dividend reversal",
            vec![leg("GBP", "-4.20")],
        );
        reversal.related_asset_ref = Some("VUSA".to_string());
        let interest = line(file, "Current", None, "Interest", vec![leg("GBP", "1.20")]);
        let mut staging = staging(file, vec![dividend, reversal, interest.clone()]);
        staging.add_classifications(vec![typed(&interest, ImportLineTypeDto::CashTransferIn)]);

        let stage = staging.stage().unwrap();
        assert_eq!(
            shapes(&stage),
            vec![
                shape("cash dividend", &[(CURRENT, GBP, "4.20")]),
                shape("regular", &[(CURRENT, GBP, "-4.20")]),
                shape("cash transfer in", &[(CURRENT, GBP, "1.20")]),
            ]
        );
        match &stage.staged[0].transaction.transaction_type {
            TransactionTypeDto::CashDividend(metadata) => {
                assert_eq!(metadata.origin_asset_id, VUSA)
            }
            other => panic!("expected a dividend, got {other:?}"),
        }
    }

    #[test]
    fn a_lone_withholding_row_joins_the_one_dividend_of_its_asset_account_and_day() {
        let file = Uuid::now_v7();
        let dividend_of = |asset_ref: &str, quantity: &str| {
            let mut dividend = line(
                file,
                "Current",
                None,
                "Dividend",
                vec![leg("GBP", quantity)],
            );
            dividend.related_asset_ref = Some(asset_ref.to_string());
            dividend
        };
        let tax_of = |asset_ref: &str, quantity: &str| {
            let mut tax = line(
                file,
                "Current",
                None,
                "Withholding tax",
                vec![fee("GBP", quantity, FeeEntryTypesDto::WithholdingTax)],
            );
            tax.related_asset_ref = Some(asset_ref.to_string());
            tax
        };
        let mut inline = dividend_of("VUSA", "4.20");
        inline
            .legs
            .push(fee("GBP", "-0.63", FeeEntryTypesDto::WithholdingTax));

        let one = first_staged(staging(file, vec![inline]));
        let two = staging(
            file,
            vec![
                dividend_of("VUSA", "4.20"),
                tax_of("VUSA", "-0.63"),
                dividend_of("VWRL", "3"),
                dividend_of("VWRL", "5"),
                tax_of("VWRL", "-0.45"),
            ],
        )
        .stage()
        .unwrap();

        assert_eq!(two.staged.len(), 3);
        assert_eq!(content(&one), content(&two.staged[0]));
        let [unpaired] = two.unstageable.as_slice() else {
            panic!("expected one unstageable group");
        };
        assert_eq!(unpaired.reason, "Withholding tax without a dividend.");
    }

    #[test]
    fn a_sale_puts_the_security_in_sale_and_the_cash_in_proceeds() {
        let file = Uuid::now_v7();
        let sale = line(
            file,
            "Current",
            None,
            "Sell VUSA",
            vec![leg("GBP", "610"), leg("VUSA", "-2")],
        );
        let mut staging = staging(file, vec![sale.clone()]);
        staging.add_classifications(vec![typed(&sale, ImportLineTypeDto::AssetSale)]);

        match first_staged(staging).transaction.transaction_type {
            TransactionTypeDto::AssetSale(metadata) => assert_eq!(
                (
                    metadata.sale.asset_id,
                    metadata.sale.quantity,
                    metadata.proceeds.asset_id,
                    metadata.proceeds.quantity
                ),
                (VUSA, Decimal::from(-2), GBP, Decimal::from(610))
            ),
            other => panic!("expected a sale, got {other:?}"),
        }
    }

    #[test]
    fn a_reference_shared_by_separate_payments_stages_each_line() {
        let file = Uuid::now_v7();
        let shop = line(
            file,
            "Current",
            Some("NOTPROVIDED"),
            "Shop",
            vec![leg("GBP", "-12.40")],
        );
        let rent = line(
            file,
            "Current",
            Some("NOTPROVIDED"),
            "Rent",
            vec![leg("GBP", "-900")],
        );
        let salary = line(
            file,
            "Current",
            Some("NOTPROVIDED"),
            "Salary",
            vec![leg("GBP", "2500")],
        );
        let debit = line(
            file,
            "Current",
            Some("ref9"),
            "Gym",
            vec![leg("GBP", "-30")],
        );
        let reversal = line(
            file,
            "Current",
            Some("ref9"),
            "Gym reversal",
            vec![leg("GBP", "30")],
        );
        let mut staging = staging(file, vec![shop, rent, salary.clone(), debit, reversal]);
        staging.add_classifications(vec![typed(&salary, ImportLineTypeDto::CashTransferIn)]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("regular", &[(CURRENT, GBP, "-12.40")]),
                shape("regular", &[(CURRENT, GBP, "-900")]),
                shape("cash transfer in", &[(CURRENT, GBP, "2500")]),
                shape("regular", &[(CURRENT, GBP, "-30")]),
                shape("regular", &[(CURRENT, GBP, "30")]),
            ]
        );
    }

    #[test]
    fn a_group_the_ledger_rejects_is_unstageable_with_its_message() {
        let file = Uuid::now_v7();
        let contribution = line(
            file,
            "Current",
            None,
            "Contribution",
            vec![leg("GBP", "200"), leg("VUSA", "12.3")],
        );

        let stage = staging(file, vec![contribution]).stage().unwrap();
        assert!(stage.staged.is_empty());
        assert_eq!(
            stage.unstageable[0].reason,
            "cash_outgoings_change.amount: Must be a negative value."
        );
    }

    #[test]
    fn a_payment_and_an_unmarked_fee_on_one_reference_itemise_the_smaller_line() {
        let file = Uuid::now_v7();
        let payment = line(
            file,
            "Current",
            Some("t1"),
            "Card",
            vec![leg("GBP", "-200")],
        );
        let unmarked = line(
            file,
            "Current",
            Some("t1"),
            "Fee",
            vec![leg("GBP", "-0.41")],
        );
        let movement = first_staged(staging(file, vec![payment, unmarked])).movements[0].clone();

        assert_eq!(movement.quantity, "-200.41".parse().unwrap());
        assert_eq!(movement.itemised_fee, Some("-0.41".parse().unwrap()));
        assert_eq!(movement.description.as_deref(), Some("Card"));
    }

    #[test]
    fn ids_are_stable_and_the_content_follows_the_lines() {
        let file = Uuid::now_v7();
        let a = line(
            file,
            "Current",
            Some("t1"),
            "Buy",
            vec![leg("GBP", "-10"), leg("VUSA", "1")],
        );
        let b = line(
            file,
            "Current",
            Some("t1"),
            "Fee",
            vec![fee("GBP", "-1", FeeEntryTypesDto::Transaction)],
        );
        let stage_of = |lines: Vec<ImportParsedLineDto>| {
            let mut staging = staging(file, lines);
            staging.add_classifications(vec![typed(&a, ImportLineTypeDto::AssetPurchase)]);
            first_staged(staging)
        };
        let forward = stage_of(vec![a.clone(), b.clone()]);
        let backward = stage_of(vec![b.clone(), a.clone()]);
        let mut renamed = a.clone();
        renamed.description = Some("Bought VUSA".to_string());
        let renamed = stage_of(vec![renamed, b.clone()]);
        let mut changed_line = a.clone();
        changed_line.legs[0].quantity = "-11".parse().unwrap();
        let changed = stage_of(vec![changed_line, b.clone()]);

        assert_eq!(forward.id, backward.id);
        assert_eq!(content(&forward), content(&backward));
        assert_eq!(content(&forward), content(&renamed));
        assert_eq!(forward.id, changed.id);
        assert_ne!(content(&forward), content(&changed));
    }

    #[test]
    fn a_conversion_keyed_to_a_purchase_nets_out_and_leftover_change_keeps_the_rows_apart() {
        let file = Uuid::now_v7();
        let buy = line(
            file,
            "Current",
            Some("t1"),
            "Buy VUSA",
            vec![leg("EUR", "-500"), leg("VUSA", "3")],
        );
        let conversion = line(
            file,
            "Current",
            Some("t1"),
            "FX",
            vec![leg("GBP", "-400"), leg("EUR", "500")],
        );
        let mut with_change = conversion.clone();
        with_change.legs[1] = leg("EUR", "500.37");

        let exact = staging(file, vec![conversion.clone(), buy.clone()])
            .stage()
            .unwrap();
        let reordered = staging(file, vec![buy.clone(), conversion])
            .stage()
            .unwrap();
        let apart = staging(file, vec![buy, with_change]).stage().unwrap();

        assert_eq!(
            shapes(&exact),
            vec![shape(
                "asset purchase",
                &[(CURRENT, VUSA, "3"), (CURRENT, GBP, "-400")]
            )]
        );
        assert_eq!(shapes(&exact), shapes(&reordered));
        assert!(apart.unstageable.is_empty());
        assert_eq!(
            shapes(&apart),
            vec![
                shape(
                    "asset purchase",
                    &[(CURRENT, VUSA, "3"), (CURRENT, EUR, "-500")]
                ),
                shape(
                    "asset trade",
                    &[(CURRENT, GBP, "-400"), (CURRENT, EUR, "500.37")]
                ),
            ]
        );
    }

    #[test]
    fn securities_moved_between_two_accounts_are_two_asset_transfers() {
        let file = Uuid::now_v7();
        let out = line(
            file,
            "Current",
            Some("t1"),
            "Transfer out",
            vec![leg("VUSA", "-10")],
        );
        let into = line(
            file,
            "Savings",
            Some("t1"),
            "Transfer in",
            vec![leg("VUSA", "10")],
        );

        let stage = staging(file, vec![out, into]).stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("asset transfer out", &[(CURRENT, VUSA, "-10")]),
                shape("asset transfer in", &[(SAVINGS, VUSA, "10")]),
            ]
        );
    }

    #[test]
    fn a_type_that_contradicts_the_direction_of_the_money_follows_the_money() {
        let file = Uuid::now_v7();
        let paid = line(file, "Current", None, "J Smith", vec![leg("GBP", "-20")]);
        let received = line(file, "Current", None, "J Smith", vec![leg("GBP", "20")]);
        let refunded = line(file, "Current", None, "Fee refund", vec![leg("GBP", "5")]);
        let returned = line(
            file,
            "Current",
            None,
            "Fee returned",
            vec![fee("GBP", "2", FeeEntryTypesDto::Transaction)],
        );
        let mut staging = staging(
            file,
            vec![paid.clone(), received.clone(), refunded.clone(), returned],
        );
        staging.add_classifications(vec![
            typed(&paid, ImportLineTypeDto::CashTransferIn),
            typed(&received, ImportLineTypeDto::CashTransferOut),
            typed(&refunded, ImportLineTypeDto::AccountFees),
        ]);

        let stage = staging.stage().unwrap();
        assert!(stage.unstageable.is_empty());
        assert_eq!(
            shapes(&stage),
            vec![
                shape("cash transfer out", &[(CURRENT, GBP, "-20")]),
                shape("cash transfer in", &[(CURRENT, GBP, "20")]),
                shape("regular", &[(CURRENT, GBP, "5")]),
                shape("regular", &[(CURRENT, GBP, "2")]),
            ]
        );
    }

    #[test]
    fn a_keyed_line_the_classifier_called_a_fee_is_a_fee_entry_of_its_payment() {
        let file = Uuid::now_v7();
        let inline = line(
            file,
            "Current",
            None,
            "Card",
            vec![
                leg("GBP", "-200"),
                fee("GBP", "-0.41", FeeEntryTypesDto::Transaction),
            ],
        );
        let payment = line(
            file,
            "Current",
            Some("t1"),
            "Card",
            vec![leg("GBP", "-200")],
        );
        let charge = line(
            file,
            "Current",
            Some("t1"),
            "Bank charge",
            vec![leg("GBP", "-0.41")],
        );
        let monthly = line(file, "Current", None, "Monthly fee", vec![leg("GBP", "-2")]);
        let mut one = staging(file, vec![inline.clone()]);
        one.add_classifications(vec![typed(&inline, ImportLineTypeDto::Regular)]);
        let mut two = staging(file, vec![charge.clone(), payment.clone(), monthly.clone()]);
        two.add_classifications(vec![
            typed(&payment, ImportLineTypeDto::Regular),
            typed(&charge, ImportLineTypeDto::AccountFees),
            typed(&monthly, ImportLineTypeDto::AccountFees),
        ]);

        let one = first_staged(one);
        let two = two.stage().unwrap();

        assert_eq!(content(&one), content(&two.staged[0]));
        assert_eq!(
            two.staged[0].movements[0].quantity,
            "-200.41".parse().unwrap()
        );
        assert_eq!(
            two.staged[0].movements[0].itemised_fee,
            Some("-0.41".parse().unwrap())
        );
        assert_eq!(
            shapes(&two)[1],
            shape("account fees", &[(CURRENT, GBP, "-2")])
        );
    }

    #[test]
    fn a_fee_taken_in_units_is_unstageable() {
        let file = Uuid::now_v7();
        let charge = line(
            file,
            "Current",
            None,
            "Platform fee",
            vec![fee("VUSA", "-0.012", FeeEntryTypesDto::Transaction)],
        );
        let rebate = line(
            file,
            "Current",
            None,
            "Fee rebate",
            vec![fee("VWRL", "0.004", FeeEntryTypesDto::Transaction)],
        );
        let mixed = line(
            file,
            "Current",
            None,
            "Custody",
            vec![
                fee("VUSA", "-0.01", FeeEntryTypesDto::Transaction),
                fee("GBP", "-1", FeeEntryTypesDto::Transaction),
            ],
        );

        let stage = staging(file, vec![charge, rebate, mixed]).stage().unwrap();
        assert!(stage.staged.is_empty());
        assert_eq!(
            stage
                .unstageable
                .iter()
                .map(|group| (group.description.as_deref(), group.reason.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (Some("Platform fee"), "A fee taken in units of a security."),
                (Some("Fee rebate"), "A fee taken in units of a security."),
                (Some("Custody"), "A fee taken in units of a security."),
            ]
        );
    }

    #[test]
    fn a_keyed_line_called_a_fee_beside_two_other_assets_is_the_trades_fee_entry_in_a_currency() {
        let file = Uuid::now_v7();
        let trade = |charged_in: &str, amount: &str, called_a_fee: bool| {
            let bought = line(file, "Current", Some("t1"), "Buy", vec![leg("VUSA", "3")]);
            let paid = line(
                file,
                "Current",
                Some("t1"),
                "Transaction Related",
                vec![leg("GBP", "-550")],
            );
            let charge = line(
                file,
                "Current",
                Some("t1"),
                "Fee",
                vec![leg(charged_in, amount)],
            );
            let mut staging = staging(file, vec![bought, paid, charge.clone()]);
            if called_a_fee {
                staging.add_classifications(vec![typed(&charge, ImportLineTypeDto::AccountFees)]);
            }
            staging.stage().unwrap()
        };

        let named = trade("EUR", "-1", true);
        assert_eq!(
            shapes(&named),
            vec![shape(
                "asset purchase",
                &[(CURRENT, VUSA, "3"), (CURRENT, GBP, "-550")]
            )]
        );
        let [charged] = named.staged[0].transaction.fee_entries.as_slice() else {
            panic!("expected one fee entry");
        };
        assert_eq!(
            (charged.entry.asset_id, charged.entry.quantity),
            (EUR, Decimal::from(-1))
        );
        assert_eq!(
            trade("EUR", "-1", false).unstageable[0].reason,
            "Expected a security leg and a currency leg, found 1 security and 2 currency."
        );
        let in_units = trade("VWRL", "-0.001", true);
        assert!(in_units.staged.is_empty());
        assert_eq!(
            in_units.unstageable[0].reason,
            "Expected one leg out and one leg in, found 2 out and 1 in."
        );
    }

    #[test]
    fn a_line_called_a_fee_beside_a_conversion_that_nets_out_stays_a_main_leg() {
        let file = Uuid::now_v7();
        let buy = line(
            file,
            "Current",
            Some("t1"),
            "Buy VUSA",
            vec![leg("EUR", "-600"), leg("VUSA", "3")],
        );
        let debit = line(
            file,
            "Current",
            Some("t1"),
            "FX debit",
            vec![leg("GBP", "-520")],
        );
        let credit = line(
            file,
            "Current",
            Some("t1"),
            "FX credit",
            vec![leg("EUR", "600")],
        );
        let mut staging = staging(file, vec![buy, debit.clone(), credit]);
        staging.add_classifications(vec![typed(&debit, ImportLineTypeDto::AccountFees)]);

        let stage = staging.stage().unwrap();
        assert_eq!(
            shapes(&stage),
            vec![shape(
                "asset purchase",
                &[(CURRENT, VUSA, "3"), (CURRENT, GBP, "-520")]
            )]
        );
        assert!(stage.staged[0].transaction.fee_entries.is_empty());
    }

    #[test]
    fn the_largest_line_gives_the_date_the_description_and_the_category() {
        let file = Uuid::now_v7();
        let charge = line(
            file,
            "Current",
            Some("t1"),
            "Bank charge",
            vec![leg("GBP", "-0.41")],
        );
        let mut payment = line(
            file,
            "Current",
            Some("t1"),
            "Tesco",
            vec![leg("GBP", "-200")],
        );
        payment.date = datetime!(2026-02-02 9:00 UTC);
        let mut staging = staging(file, vec![charge, payment.clone()]);
        staging.add_classifications(vec![ImportLineClassificationDto {
            parsed_line_id: payment.id,
            line_type: ImportLineTypeDto::Regular,
            category_id: Some(GROCERIES),
        }]);

        let staged = first_staged(staging);
        assert_eq!(staged.transaction.date, payment.date);
        match staged.transaction.transaction_type {
            TransactionTypeDto::Regular(regular) => assert_eq!(
                (
                    regular.description.as_deref(),
                    regular.category_id,
                    regular.entry.quantity
                ),
                (Some("Tesco"), GROCERIES, "-200.41".parse().unwrap())
            ),
            other => panic!("expected regular, got {other:?}"),
        }
    }
}
