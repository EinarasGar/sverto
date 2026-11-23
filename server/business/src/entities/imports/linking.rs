use std::collections::{BTreeSet, HashMap};

use dal::enums::account_types::DatabaseAccountTypes;
use dal::models::asset_models::{asset_type_ids, AssetRefModel, ExchangeCurrencyModel};
use dal::models::import_models::{
    ImportAccountMappingModel, ImportAccountRefModel, ImportAssetMappingModel, ImportAssetRefModel,
};
use rust_decimal::Decimal;
use time::{Date, Duration};
use uuid::Uuid;

use crate::dtos::accounts::full_account_dto::FullAccountDto;
use crate::dtos::asset_dto::AssetDto;
use crate::dtos::imports::import_ask_dto::{
    AccountCandidateDto, AccountMappingAskDto, AssetCandidateDto, AssetMappingAskDto,
    ImportAskChoiceDto, ImportAskDto, ImportOpenAskDto,
};
use crate::dtos::imports::import_balance_dto::{ImportAssertionDto, ImportAssertionKindDto};
use crate::dtos::imports::import_classification_dto::ImportParsedLineDto;
use crate::dtos::imports::import_pending_dto::{ImportPendingLegDto, ImportPendingLineDto};
use crate::dynamic_enums::{account_types::AccountTypes, DynamicEnum};

pub const ASSET_SUGGESTIONS: usize = 5;

const MIN_FRAGMENT_DIGITS: usize = 3;
const MIN_NUMBER_DIGITS: usize = 6;
const MIN_TAIL_DIGITS: usize = 8;
const MAX_DAYS_BETWEEN_BALANCES: i64 = 7;
const SECURITIES_ACCOUNT_TYPES: [DatabaseAccountTypes; 3] = [
    DatabaseAccountTypes::Investment,
    DatabaseAccountTypes::PersonalPension,
    DatabaseAccountTypes::WorkplacePension,
];

pub struct SessionLinking {
    accounts: Vec<FullAccountDto>,
    account_refs: Vec<ImportAccountRefModel>,
    account_answers: Vec<ImportAccountMappingModel>,
    balances: Vec<ImportAssertionDto>,
    asset_refs: Vec<ImportAssetRefModel>,
    asset_answers: Vec<ImportAssetMappingModel>,
    asset_matches: Vec<AssetRefModel>,
    exchange_currencies: HashMap<String, i32>,
    asset_suggestions: HashMap<String, Vec<AssetCandidateDto>>,
    links: SessionLinks,
}

#[derive(Default)]
pub struct SessionLinks {
    pub account_mappings: Vec<ImportAccountMappingModel>,
    pub asset_mappings: Vec<ImportAssetMappingModel>,
    pub asks: Vec<ImportOpenAskDto>,
    pub account_types: HashMap<Uuid, i32>,
}

enum AccountLink {
    Map(Option<Uuid>),
    Ask(AccountMappingAskDto),
}

enum AssetLink {
    Map(i32),
    Ask(Vec<AssetCandidateDto>),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IdentifierMatch {
    Part,
    Whole,
}

impl SessionLinking {
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
            account_refs: Vec::new(),
            account_answers: Vec::new(),
            balances: Vec::new(),
            asset_refs: Vec::new(),
            asset_answers: Vec::new(),
            asset_matches: Vec::new(),
            exchange_currencies: HashMap::new(),
            asset_suggestions: HashMap::new(),
            links: SessionLinks::default(),
        }
    }

    pub fn add_accounts(&mut self, accounts: Vec<FullAccountDto>) {
        self.accounts.extend(accounts);
    }

    pub fn add_account_refs(&mut self, refs: Vec<ImportAccountRefModel>) {
        self.account_refs.extend(refs);
    }

    pub fn add_account_answers(&mut self, answers: Vec<ImportAccountMappingModel>) {
        self.account_answers.extend(answers);
    }

    pub fn repeats_an_answered_name(&self) -> bool {
        self.account_refs.iter().any(|reference| {
            let mut answered_sources = self
                .account_answers
                .iter()
                .filter(|answer| answer.account_ref == reference.account_ref)
                .map(|answer| answer.source_id);
            answered_sources
                .clone()
                .any(|source_id| source_id != reference.source_id)
                && !answered_sources.any(|source_id| source_id == reference.source_id)
        })
    }

    pub fn add_balances(&mut self, balances: Vec<ImportAssertionDto>) {
        self.balances.extend(balances);
    }

    pub fn add_asset_refs(&mut self, refs: Vec<ImportAssetRefModel>) {
        self.asset_refs.extend(refs);
    }

    pub fn add_asset_answers(&mut self, answers: Vec<ImportAssetMappingModel>) {
        self.asset_answers.extend(answers);
    }

    pub fn add_asset_matches(&mut self, matches: Vec<AssetRefModel>) {
        self.asset_matches.extend(matches);
    }

    pub fn add_exchange_currencies(&mut self, mut currencies: Vec<ExchangeCurrencyModel>) {
        currencies.sort_by_key(|currency| currency.listings);
        self.exchange_currencies.extend(
            currencies
                .into_iter()
                .map(|currency| (currency.exchange, currency.base_pair_id)),
        );
    }

    pub fn add_asset_suggestions(&mut self, asset_ref: String, assets: Vec<AssetDto>) {
        let suggestions = assets
            .into_iter()
            .take(ASSET_SUGGESTIONS)
            .map(|asset| AssetCandidateDto {
                id: asset.asset_id,
                ticker: asset.ticker,
                name: asset.name,
            })
            .collect();
        self.asset_suggestions.insert(asset_ref, suggestions);
    }

    pub fn link_accounts(&mut self) {
        for reference in std::mem::take(&mut self.account_refs) {
            let answer = self.account_answers.iter().find(|answer| {
                answer.source_id == reference.source_id
                    && answer.account_ref == reference.account_ref
            });
            let link = match answer {
                Some(answer) => AccountLink::Map(answer.account_id),
                None => self.link_account(reference.source_id, &reference.account_ref),
            };
            match link {
                AccountLink::Map(account_id) => {
                    self.links.account_mappings.push(ImportAccountMappingModel {
                        source_id: reference.source_id,
                        account_ref: reference.account_ref,
                        account_id,
                    })
                }
                AccountLink::Ask(ask) => self.links.asks.push(ImportOpenAskDto {
                    ask_key: account_ask_key(ask.source_id, &ask.raw_account),
                    ask: ImportAskDto::AccountMapping(ask),
                }),
            }
        }
    }

    fn imported_assets(&self) -> Vec<String> {
        self.imported_asset_refs()
            .map(|reference| reference.asset_ref.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn asset_search_terms(&self) -> Vec<String> {
        let mut terms = BTreeSet::new();
        for asset_ref in self.imported_assets() {
            match printed_isin(&asset_ref) {
                Some(_) => terms.extend(words(&asset_ref).map(str::to_string)),
                None => terms.extend(
                    asset_ref
                        .rsplit_once('.')
                        .map(|(symbol, _)| symbol.to_string()),
                ),
            }
            terms.insert(asset_ref);
        }
        terms.into_iter().collect()
    }

    pub fn answered_asset_ids(&self) -> Vec<i32> {
        self.asset_answers
            .iter()
            .filter_map(|answer| answer.asset_id)
            .collect()
    }

    pub fn exchanges_of_candidates(&self) -> Vec<String> {
        let mut exchanges = BTreeSet::new();
        for asset_ref in self.unmapped_assets() {
            if let AssetLink::Ask(candidates) = self.link_asset(&asset_ref) {
                if candidates.len() > 1 {
                    exchanges.extend(candidates.iter().filter_map(|candidate| {
                        let (_, exchange) = candidate.ticker.rsplit_once('.')?;
                        Some(exchange.to_string())
                    }));
                }
            }
        }
        exchanges.into_iter().collect()
    }

    pub fn unmapped_assets(&self) -> Vec<String> {
        self.imported_assets()
            .into_iter()
            .filter(|asset_ref| {
                !self
                    .asset_answers
                    .iter()
                    .any(|answer| &answer.asset_ref == asset_ref)
            })
            .collect()
    }

    pub fn assets_needing_suggestions(&self) -> Vec<String> {
        self.unmapped_assets()
            .into_iter()
            .filter(|asset_ref| {
                matches!(self.link_asset(asset_ref), AssetLink::Ask(candidates) if candidates.is_empty())
            })
            .collect()
    }

    pub fn link(mut self) -> SessionLinks {
        for asset_ref in self.unmapped_assets() {
            let mut candidates = match self.link_asset(&asset_ref) {
                AssetLink::Map(asset_id) => {
                    self.links.asset_mappings.push(ImportAssetMappingModel {
                        asset_ref,
                        asset_id: Some(asset_id),
                    });
                    continue;
                }
                AssetLink::Ask(candidates) => candidates,
            };
            if candidates.is_empty() {
                candidates = self
                    .asset_suggestions
                    .remove(&asset_ref)
                    .unwrap_or_default();
            }
            let choices = match self.currency(&asset_ref) {
                Some(_) => vec![ImportAskChoiceDto::Map],
                None => vec![ImportAskChoiceDto::Map, ImportAskChoiceDto::Exclude],
            };
            self.links.asks.push(ImportOpenAskDto {
                ask_key: asset_ask_key(&asset_ref),
                ask: ImportAskDto::AssetMapping(AssetMappingAskDto {
                    raw: asset_ref,
                    candidates,
                    choices,
                }),
            });
        }
        self.links.asset_mappings.extend(self.asset_answers);
        self.links
            .asks
            .sort_by(|left, right| left.ask_key.cmp(&right.ask_key));
        self.links.account_types = self
            .accounts
            .iter()
            .map(|account| (account.id, account.account_type.id))
            .collect();
        self.links
    }

    fn link_account(&self, source_id: Uuid, account_ref: &str) -> AccountLink {
        let mut by_identifier: Vec<(IdentifierMatch, &FullAccountDto)> = self
            .accounts
            .iter()
            .filter_map(|account| {
                let closest = account
                    .identifiers
                    .iter()
                    .filter_map(|identifier| identifier_match(account_ref, &identifier.value))
                    .max()?;
                Some((closest, account))
            })
            .collect();
        by_identifier.sort_by_key(|(closeness, _)| std::cmp::Reverse(*closeness));
        if let [(IdentifierMatch::Whole, account)]
        | [(IdentifierMatch::Whole, account), (IdentifierMatch::Part, _), ..] =
            by_identifier.as_slice()
        {
            return AccountLink::Map(Some(account.id));
        }

        let mut earlier: Vec<&ImportAccountMappingModel> = self
            .account_answers
            .iter()
            .filter(|answer| answer.source_id != source_id && answer.account_ref == account_ref)
            .collect();
        let continued = self.continued_sources(source_id, account_ref);
        earlier.sort_by_key(|answer| !continued.contains(&answer.source_id));
        let mut answered: Vec<Option<Uuid>> = Vec::new();
        for answer in earlier {
            if !answered.contains(&answer.account_id) {
                answered.push(answer.account_id);
            }
        }
        if let ([account_id], true) = (answered.as_slice(), is_specific(account_ref)) {
            return AccountLink::Map(*account_id);
        }

        let answered_before = answered
            .into_iter()
            .flatten()
            .filter_map(|id| self.accounts.iter().find(|account| account.id == id));
        let reference = account_ref.to_lowercase();
        let similarly_named = self.accounts.iter().filter(|account| {
            let name = account.account_name.to_lowercase();
            name.contains(&reference) || reference.contains(&name)
        });
        let mut candidates: Vec<AccountCandidateDto> = Vec::new();
        for account in answered_before
            .chain(by_identifier.iter().map(|(_, account)| *account))
            .chain(similarly_named)
        {
            if !candidates.iter().any(|known| known.id == account.id) {
                candidates.push(AccountCandidateDto {
                    id: account.id,
                    name: account.account_name.clone(),
                });
            }
        }
        AccountLink::Ask(AccountMappingAskDto {
            source_id,
            raw_account: account_ref.to_string(),
            candidates,
            choices: vec![ImportAskChoiceDto::Map, ImportAskChoiceDto::Exclude],
            answer_covers_all_sources: is_specific(account_ref),
        })
    }

    fn continued_sources(&self, source_id: Uuid, account_ref: &str) -> Vec<Uuid> {
        let stated = |source_id: Uuid| {
            let mut by_asset: HashMap<Option<&str>, Vec<(Date, Decimal)>> = HashMap::new();
            for balance in &self.balances {
                if balance.source_id != source_id
                    || balance.account_ref != account_ref
                    || balance.kind == ImportAssertionKindDto::ValueTotal
                {
                    continue;
                }
                let at = if balance.kind == ImportAssertionKindDto::Opening {
                    balance.date - Duration::days(1)
                } else {
                    balance.date
                };
                by_asset
                    .entry(balance.asset_ref.as_deref())
                    .or_default()
                    .push((at, balance.quantity));
            }
            by_asset
        };
        let picks_up = |later: Uuid, earlier: Uuid| {
            let earlier = stated(earlier);
            stated(later).into_iter().any(|(asset_ref, first)| {
                let last = earlier
                    .get(&asset_ref)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let starts_at = first.iter().map(|(at, _)| *at).min();
                let ended_at = last
                    .iter()
                    .map(|(at, _)| *at)
                    .filter(|at| Some(*at) <= starts_at)
                    .max();
                let (Some(starts_at), Some(ended_at)) = (starts_at, ended_at) else {
                    return false;
                };
                (starts_at - ended_at).whole_days() <= MAX_DAYS_BETWEEN_BALANCES
                    && first.iter().any(|start| {
                        start.0 == starts_at
                            && !start.1.is_zero()
                            && last.contains(&(ended_at, start.1))
                    })
            })
        };
        let sources: BTreeSet<Uuid> = self
            .balances
            .iter()
            .filter(|balance| balance.account_ref == account_ref)
            .map(|balance| balance.source_id)
            .collect();
        let mut chain = vec![source_id];
        let mut reached = 0;
        while let Some(&link) = chain.get(reached) {
            for &other in &sources {
                if !chain.contains(&other) && (picks_up(link, other) || picks_up(other, link)) {
                    chain.push(other);
                }
            }
            reached += 1;
        }
        chain
    }

    fn link_asset(&self, asset_ref: &str) -> AssetLink {
        let paid_in: BTreeSet<i32> = self
            .counter_refs(asset_ref)
            .filter_map(|counter| self.currency(counter))
            .collect();
        let quoted_in_paid_currency =
            |asset: &AssetRefModel| asset.base_pair_id.is_some_and(|id| paid_in.contains(&id));
        let quoted_in_home_currency = |asset: &AssetRefModel| {
            let home = asset
                .ticker
                .rsplit_once('.')
                .and_then(|(_, exchange)| self.exchange_currencies.get(exchange));
            home.is_some() && home == asset.base_pair_id.as_ref()
        };

        let exact: Vec<&AssetRefModel> = self
            .asset_matches
            .iter()
            .filter(|asset| asset.ticker.eq_ignore_ascii_case(asset_ref))
            .collect();
        let printed_isin = printed_isin(asset_ref);
        let isin = printed_isin.unwrap_or(asset_ref);
        let beside_isin: Vec<&str> = match printed_isin {
            Some(isin) => words(asset_ref).filter(|word| *word != isin).collect(),
            None => Vec::new(),
        };
        let mut listings: Vec<&AssetRefModel> = self
            .asset_matches
            .iter()
            .filter(|asset| {
                carries_isin(asset, isin) || (printed_isin.is_none() && symbol_is(asset, asset_ref))
            })
            .collect();
        if listings.is_empty() && exact.is_empty() {
            let symbol = match (printed_isin, beside_isin.as_slice()) {
                (Some(_), [ticker]) => Some(*ticker),
                (Some(_), _) => None,
                (None, _) => asset_ref.rsplit_once('.').map(|(symbol, _)| symbol),
            };
            listings = self
                .asset_matches
                .iter()
                .filter(|asset| {
                    symbol.is_some_and(|symbol| symbol_is(asset, symbol))
                        && (printed_isin.is_none() || asset.isin.is_none())
                })
                .collect();
        }
        let beside_its_isin =
            |asset: &AssetRefModel| beside_isin.iter().any(|word| symbol_is(asset, word));
        listings.sort_by_key(|listing| {
            (
                !beside_its_isin(listing),
                !quoted_in_paid_currency(listing),
                !quoted_in_home_currency(listing),
            )
        });

        let linked = match exact.as_slice() {
            [asset] if listings.is_empty() => Some(*asset),
            [asset] => {
                let account_types = || {
                    self.imported_asset_refs()
                        .filter(|reference| reference.asset_ref == asset_ref)
                        .map(|reference| {
                            self.links
                                .account_mapping(reference.source_id, &reference.account_ref)
                                .and_then(|mapping| mapping.account_id)
                                .and_then(|id| {
                                    self.accounts.iter().find(|account| account.id == id)
                                })
                                .and_then(|account| {
                                    AccountTypes::try_from_dynamic_enum(account.account_type.id)
                                        .ok()
                                })
                        })
                };
                let exchanged_for = || {
                    self.counter_refs(asset_ref)
                        .map(|counter| self.readings(counter).collect::<Vec<_>>())
                };
                let is_what_its_code_says = match asset.asset_type {
                    asset_type_ids::CURRENCY => {
                        let pays_for_a_security_quoted_in_it =
                            exchanged_for().flatten().any(|bought| {
                                bought.asset_type != asset_type_ids::CURRENCY
                                    && bought.base_pair_id == Some(asset.id)
                            });
                        let on_a_securities_account = account_types()
                            .flatten()
                            .any(|account_type| SECURITIES_ACCOUNT_TYPES.contains(&account_type));
                        let beside_currencies_only = self
                            .imported_asset_refs()
                            .filter(|reference| reference.asset_ref == asset_ref)
                            .all(|reference| {
                                self.imported_asset_refs()
                                    .filter(|beside| {
                                        beside.source_id == reference.source_id
                                            && beside.account_ref == reference.account_ref
                                    })
                                    .all(|beside| self.currency(&beside.asset_ref).is_some())
                            });
                        pays_for_a_security_quoted_in_it
                            || (!on_a_securities_account && beside_currencies_only)
                    }
                    asset_type_ids::CRYPTO => {
                        account_types().all(|account_type| {
                            account_type == Some(DatabaseAccountTypes::CryptoWallet)
                        }) || exchanged_for().any(|readings| {
                            !readings.is_empty()
                                && readings
                                    .iter()
                                    .all(|coin| coin.asset_type == asset_type_ids::CRYPTO)
                        })
                    }
                    _ => false,
                };
                Some(*asset).filter(|_| is_what_its_code_says)
            }
            [] => {
                let named = listings
                    .iter()
                    .take_while(|listing| beside_its_isin(listing))
                    .count();
                let choices = if named == 0 {
                    listings.as_slice()
                } else {
                    &listings[..named]
                };
                let mut in_paid_currency = choices
                    .iter()
                    .filter(|listing| paid_in.len() == 1 && quoted_in_paid_currency(listing));
                let singled_out = match (choices, in_paid_currency.next(), in_paid_currency.next())
                {
                    ([listing], _, _) | (_, Some(listing), None) => Some(*listing),
                    _ => None,
                };
                singled_out.filter(|listing| carries_isin(listing, isin))
            }
            _ => None,
        };
        if let Some(asset) = linked {
            return AssetLink::Map(asset.id);
        }

        let likelier = match exact.as_slice() {
            [asset] if asset.asset_type == asset_type_ids::CURRENCY => listings
                .iter()
                .take_while(|listing| quoted_in_paid_currency(listing))
                .count(),
            _ => 0,
        };
        let (likelier, others) = listings.split_at(likelier);
        AssetLink::Ask(
            likelier
                .iter()
                .chain(&exact)
                .chain(others)
                .map(|asset| AssetCandidateDto {
                    id: asset.id,
                    ticker: asset.ticker.clone(),
                    name: asset.asset_name.clone(),
                })
                .collect(),
        )
    }

    fn imported_asset_refs(&self) -> impl Iterator<Item = &ImportAssetRefModel> {
        self.asset_refs.iter().filter(|reference| {
            !self
                .links
                .is_excluded(reference.source_id, &reference.account_ref)
        })
    }

    fn counter_refs<'a>(&'a self, asset_ref: &'a str) -> impl Iterator<Item = &'a str> {
        self.imported_asset_refs()
            .filter(move |reference| reference.asset_ref == asset_ref)
            .filter_map(|reference| reference.counter_asset_ref.as_deref())
    }

    fn readings<'a>(&'a self, asset_ref: &'a str) -> impl Iterator<Item = &'a AssetRefModel> {
        let answer = self
            .asset_answers
            .iter()
            .find(|answer| answer.asset_ref == asset_ref);
        let isin = printed_isin(asset_ref).unwrap_or(asset_ref);
        self.asset_matches.iter().filter(move |asset| match answer {
            Some(answer) => answer.asset_id == Some(asset.id),
            None => {
                asset.ticker.eq_ignore_ascii_case(asset_ref)
                    || symbol_is(asset, asset_ref)
                    || carries_isin(asset, isin)
            }
        })
    }

    fn currency(&self, asset_ref: &str) -> Option<i32> {
        self.readings(asset_ref)
            .find(|asset| asset.asset_type == asset_type_ids::CURRENCY)
            .map(|asset| asset.id)
    }
}

impl SessionLinks {
    pub fn sources_answered_with(&self, source_id: Uuid, account_ref: &str) -> Vec<Uuid> {
        let mut sources = vec![source_id];
        if is_specific(account_ref) {
            sources.extend(self.asks.iter().filter_map(|open| match &open.ask {
                ImportAskDto::AccountMapping(ask)
                    if ask.raw_account == account_ref && ask.source_id != source_id =>
                {
                    Some(ask.source_id)
                }
                _ => None,
            }));
        }
        sources
    }

    pub fn pending_lines(&self, lines: Vec<ImportParsedLineDto>) -> Vec<ImportPendingLineDto> {
        let mut group_questions: HashMap<(Uuid, String), Vec<String>> = HashMap::new();
        for line in &lines {
            let Some(key) = &line.transaction_key else {
                continue;
            };
            let questions = group_questions
                .entry((line.source_id, key.clone()))
                .or_default();
            for ask_key in self.blocked_by(line) {
                if !questions.contains(&ask_key) {
                    questions.push(ask_key);
                }
            }
        }

        lines
            .into_iter()
            .filter_map(|line| {
                let mut blocked_by = self.blocked_by(&line);
                let group = match &line.transaction_key {
                    Some(key)
                        if !self.is_excluded(line.source_id, &line.account_ref)
                            && !self.moves_only_excluded_assets(&line) =>
                    {
                        group_questions.get(&(line.source_id, key.clone()))
                    }
                    _ => None,
                };
                for ask_key in group.into_iter().flatten() {
                    if !blocked_by.contains(ask_key) {
                        blocked_by.push(ask_key.clone());
                    }
                }
                if blocked_by.is_empty() {
                    return None;
                }
                Some(ImportPendingLineDto {
                    id: line.id,
                    source_id: line.source_id,
                    account_ref: line.account_ref,
                    date: line.date,
                    description: line.description,
                    legs: line
                        .legs
                        .into_iter()
                        .filter(|leg| !self.is_asset_excluded(&leg.asset_ref))
                        .map(|leg| ImportPendingLegDto {
                            asset_ref: leg.asset_ref,
                            quantity: leg.quantity,
                            fee_type: leg.fee_type,
                        })
                        .collect(),
                    blocked_by,
                })
            })
            .collect()
    }

    fn blocked_by(&self, line: &ImportParsedLineDto) -> Vec<String> {
        let mut blocked_by = Vec::new();
        if self.moves_only_excluded_assets(line) {
            return blocked_by;
        }
        if self
            .account_mapping(line.source_id, &line.account_ref)
            .is_none()
        {
            blocked_by.push(account_ask_key(line.source_id, &line.account_ref));
        } else if self.is_excluded(line.source_id, &line.account_ref) {
            return blocked_by;
        }
        let asset_refs = line
            .legs
            .iter()
            .map(|leg| leg.asset_ref.as_str())
            .chain(line.related_asset_ref.as_deref());
        for asset_ref in asset_refs {
            let key = asset_ask_key(asset_ref);
            let mapped = self
                .asset_mappings
                .iter()
                .any(|mapping| mapping.asset_ref == asset_ref);
            if !mapped && !blocked_by.contains(&key) {
                blocked_by.push(key);
            }
        }
        blocked_by
    }

    fn account_mapping(
        &self,
        source_id: Uuid,
        account_ref: &str,
    ) -> Option<&ImportAccountMappingModel> {
        self.account_mappings
            .iter()
            .find(|mapping| mapping.source_id == source_id && mapping.account_ref == account_ref)
    }

    fn is_excluded(&self, source_id: Uuid, account_ref: &str) -> bool {
        self.account_mapping(source_id, account_ref)
            .is_some_and(|mapping| mapping.account_id.is_none())
    }

    fn is_asset_excluded(&self, asset_ref: &str) -> bool {
        self.asset_mappings
            .iter()
            .any(|mapping| mapping.asset_ref == asset_ref && mapping.asset_id.is_none())
    }

    fn moves_only_excluded_assets(&self, line: &ImportParsedLineDto) -> bool {
        !line.legs.is_empty()
            && line
                .legs
                .iter()
                .all(|leg| self.is_asset_excluded(&leg.asset_ref))
    }
}

fn is_specific(account_ref: &str) -> bool {
    account_ref
        .split(|c: char| !c.is_ascii_digit())
        .any(|run| match run.len() {
            0..=3 => false,
            4 => !("1900"..="2099").contains(&run),
            _ => true,
        })
}

pub fn account_ask_key(source_id: Uuid, account_ref: &str) -> String {
    format!("account_mapping:{source_id}:{account_ref}")
}

pub fn asset_ask_key(asset_ref: &str) -> String {
    format!("asset_mapping:{asset_ref}")
}

fn symbol_is(asset: &AssetRefModel, symbol: &str) -> bool {
    asset
        .ticker
        .rsplit_once('.')
        .is_some_and(|(listed, _)| listed.eq_ignore_ascii_case(symbol))
}

fn carries_isin(asset: &AssetRefModel, isin: &str) -> bool {
    asset
        .isin
        .as_deref()
        .is_some_and(|carried| carried.eq_ignore_ascii_case(isin))
}

fn words(asset_ref: &str) -> impl Iterator<Item = &str> {
    asset_ref
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
}

fn printed_isin(asset_ref: &str) -> Option<&str> {
    words(asset_ref).find(|word| {
        let shaped = word.len() == 12
            && word.bytes().take(2).all(|c| c.is_ascii_alphabetic())
            && word.bytes().last().is_some_and(|c| c.is_ascii_digit());
        let digits = word.chars().rev().flat_map(|c| {
            let value = c.to_digit(36).unwrap_or_default();
            [value % 10, value / 10]
                .into_iter()
                .take(if value < 10 { 1 } else { 2 })
        });
        let sum: u32 = digits
            .enumerate()
            .map(|(place, digit)| match place % 2 {
                0 => digit,
                _ => digit * 2 / 10 + digit * 2 % 10,
            })
            .sum();
        shaped && sum % 10 == 0
    })
}

fn identifier_match(account_ref: &str, identifier: &str) -> Option<IdentifierMatch> {
    let digits = |value: &str| -> String { value.chars().filter(char::is_ascii_digit).collect() };
    let (printed, stored) = (digits(account_ref), digits(identifier));
    let (shorter, longer) = if printed.len() <= stored.len() {
        (printed, stored)
    } else {
        (stored, printed)
    };
    let same_number = shorter == longer && shorter.len() >= MIN_NUMBER_DIGITS;
    let number_is_tail = longer.ends_with(&shorter) && shorter.len() >= MIN_TAIL_DIGITS;
    if same_number || number_is_tail {
        return Some(IdentifierMatch::Whole);
    }
    if shorter.len() < MIN_FRAGMENT_DIGITS {
        return None;
    }
    let last_four = &shorter[shorter.len().saturating_sub(4)..];
    (longer.contains(&shorter) || longer.ends_with(last_four)).then_some(IdentifierMatch::Part)
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use time::macros::date;
    use time::OffsetDateTime;

    use super::*;
    use crate::dtos::accounts::account_identifier_dto::{
        AccountIdentifierDto, AccountIdentifierKind,
    };
    use crate::dtos::accounts::account_liquidity_type_dto::AccountLiquidityTypeDto;
    use crate::dtos::accounts::account_type_dto::AccountTypeDto;
    use crate::dtos::fee_entry_types_dto::FeeEntryTypesDto;
    use crate::dtos::imports::import_classification_dto::ImportParsedLegDto;

    const STOCK: i32 = 2;
    const EUR: i32 = 42;
    const GBP: i32 = 45;
    const USD: i32 = 132;
    const CAD: i32 = 24;
    const PLN: i32 = 104;

    fn source(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn candidate_ids(links: &SessionLinks) -> Vec<Uuid> {
        account_ask(links)
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect()
    }

    fn account_type_id(account_type: DatabaseAccountTypes) -> i32 {
        crate::test_support::load_dynamic_enums();
        AccountTypes::try_into_dynamic_enum(account_type).unwrap()
    }

    fn account(name: &str, identifiers: &[&str]) -> FullAccountDto {
        FullAccountDto {
            id: Uuid::new_v4(),
            user_id: Uuid::nil(),
            account_name: name.to_string(),
            account_type: AccountTypeDto {
                id: 1,
                name: "Current".to_string(),
            },
            liquidity_type: AccountLiquidityTypeDto {
                id: 1,
                name: "Liquid".to_string(),
            },
            ownership_share: Decimal::ONE,
            identifiers: identifiers
                .iter()
                .map(|value| AccountIdentifierDto {
                    kind: AccountIdentifierKind::Iban,
                    value: value.to_string(),
                })
                .collect(),
            suggested_currency: None,
        }
    }

    fn account_ref(source_id: Uuid, name: &str) -> ImportAccountRefModel {
        ImportAccountRefModel {
            source_id,
            account_ref: name.to_string(),
        }
    }

    fn mapping(source_id: Uuid, name: &str, account_id: Option<Uuid>) -> ImportAccountMappingModel {
        ImportAccountMappingModel {
            source_id,
            account_ref: name.to_string(),
            account_id,
        }
    }

    fn balance(
        source_id: Uuid,
        account: &str,
        kind: ImportAssertionKindDto,
        on: Date,
        quantity: Decimal,
    ) -> ImportAssertionDto {
        ImportAssertionDto {
            source_id,
            account_ref: account.to_string(),
            asset_ref: Some("EUR".to_string()),
            kind,
            date: on,
            quantity,
        }
    }

    fn opening(source_id: Uuid, account: &str, on: Date, quantity: Decimal) -> ImportAssertionDto {
        balance(
            source_id,
            account,
            ImportAssertionKindDto::Opening,
            on,
            quantity,
        )
    }

    fn closing(source_id: Uuid, account: &str, on: Date, quantity: Decimal) -> ImportAssertionDto {
        balance(
            source_id,
            account,
            ImportAssertionKindDto::Closing,
            on,
            quantity,
        )
    }

    fn linked_with_balances(
        accounts: Vec<FullAccountDto>,
        refs: Vec<ImportAccountRefModel>,
        answers: Vec<ImportAccountMappingModel>,
        balances: Vec<ImportAssertionDto>,
    ) -> SessionLinks {
        let mut linking = SessionLinking::new();
        linking.add_accounts(accounts);
        linking.add_account_refs(refs);
        linking.add_account_answers(answers);
        assert!(linking.repeats_an_answered_name());
        linking.add_balances(balances);
        linking.link_accounts();
        linking.link()
    }

    fn asset_ref(source_id: Uuid, account: &str, asset: &str) -> ImportAssetRefModel {
        ImportAssetRefModel {
            source_id,
            account_ref: account.to_string(),
            asset_ref: asset.to_string(),
            counter_asset_ref: None,
        }
    }

    fn trades(paid: &[(&str, &str)]) -> Vec<ImportAssetRefModel> {
        paid.iter()
            .flat_map(|(asset, paid_in)| [(asset, paid_in), (paid_in, asset)])
            .map(|(asset, counter)| ImportAssetRefModel {
                counter_asset_ref: Some(counter.to_string()),
                ..asset_ref(source(1), "Broker", asset)
            })
            .collect()
    }

    fn asset(id: i32, ticker: &str, isin: Option<&str>) -> AssetRefModel {
        AssetRefModel {
            id,
            ticker: ticker.to_string(),
            asset_name: format!("{ticker} name"),
            isin: isin.map(str::to_string),
            asset_type: STOCK,
            base_pair_id: None,
        }
    }

    fn listing(id: i32, ticker: &str, isin: Option<&str>, quoted_in: i32) -> AssetRefModel {
        AssetRefModel {
            base_pair_id: Some(quoted_in),
            ..asset(id, ticker, isin)
        }
    }

    fn currency(id: i32, ticker: &str) -> AssetRefModel {
        AssetRefModel {
            asset_type: asset_type_ids::CURRENCY,
            ..asset(id, ticker, None)
        }
    }

    fn crypto(id: i32, ticker: &str) -> AssetRefModel {
        AssetRefModel {
            asset_type: asset_type_ids::CRYPTO,
            ..listing(id, ticker, None, USD)
        }
    }

    fn linked_assets(refs: Vec<ImportAssetRefModel>, matches: Vec<AssetRefModel>) -> SessionLinks {
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(refs);
        linking.add_asset_matches(matches);
        linking.link()
    }

    fn held_at_a_broker(
        refs: Vec<ImportAssetRefModel>,
        matches: Vec<AssetRefModel>,
    ) -> SessionLinks {
        let mut broker = account("Broker", &[]);
        broker.account_type.id = account_type_id(DatabaseAccountTypes::Investment);
        let mut linking = SessionLinking::new();
        linking.add_account_refs(vec![account_ref(source(1), "Broker")]);
        linking.add_account_answers(vec![mapping(source(1), "Broker", Some(broker.id))]);
        linking.add_accounts(vec![broker]);
        linking.link_accounts();
        linking.add_asset_refs(refs);
        linking.add_asset_matches(matches);
        linking.link()
    }

    fn asset_questions(links: &SessionLinks) -> Vec<(String, Vec<String>)> {
        asks(links)
            .into_iter()
            .filter_map(|ask| match ask {
                ImportAskDto::AssetMapping(ask) => Some((
                    ask.raw,
                    ask.candidates.into_iter().map(|c| c.ticker).collect(),
                )),
                _ => None,
            })
            .collect()
    }

    fn question(asset_ref: &str, tickers: &[&str]) -> (String, Vec<String>) {
        (
            asset_ref.to_string(),
            tickers.iter().map(|ticker| ticker.to_string()).collect(),
        )
    }

    fn mapped_assets(links: &SessionLinks) -> Vec<(&str, i32)> {
        links
            .asset_mappings
            .iter()
            .filter_map(|mapping| Some((mapping.asset_ref.as_str(), mapping.asset_id?)))
            .collect()
    }

    fn asked_tickers(links: &SessionLinks) -> Vec<String> {
        match asks(links).as_slice() {
            [ImportAskDto::AssetMapping(ask)] => {
                ask.candidates.iter().map(|c| c.ticker.clone()).collect()
            }
            _ => panic!("expected one asset question"),
        }
    }

    fn search_result(id: i32, ticker: &str) -> AssetDto {
        AssetDto {
            ticker: ticker.to_string(),
            name: format!("{ticker} name"),
            category: String::new(),
            asset_id: id,
            owner: None,
        }
    }

    fn linked_accounts(
        accounts: Vec<FullAccountDto>,
        refs: Vec<ImportAccountRefModel>,
        answers: Vec<ImportAccountMappingModel>,
    ) -> SessionLinks {
        let mut linking = SessionLinking::new();
        linking.add_accounts(accounts);
        linking.add_account_refs(refs);
        linking.add_account_answers(answers);
        linking.link_accounts();
        linking.link()
    }

    fn asks(links: &SessionLinks) -> Vec<ImportAskDto> {
        links.asks.iter().map(|ask| ask.ask.clone()).collect()
    }

    fn account_ask(links: &SessionLinks) -> AccountMappingAskDto {
        match asks(links).as_slice() {
            [ImportAskDto::AccountMapping(ask)] => ask.clone(),
            _ => panic!("expected one account question"),
        }
    }

    fn mapped(links: &SessionLinks) -> Vec<(Uuid, Option<Uuid>)> {
        links
            .account_mappings
            .iter()
            .map(|mapping| (mapping.source_id, mapping.account_id))
            .collect()
    }

    #[test]
    fn a_number_stored_as_digits_links_when_printed_with_letters_or_words() {
        for (stored, printed) in [
            ("58201144", "GIA-58201144"),
            ("204417", "MCB-204417"),
            ("1234567", "普通　1234567"),
            ("004125012345", "Chequing 00412-5012345"),
            ("20320613152170", "20-32-06 13152170"),
        ] {
            let target = account("Target", &[stored]);
            let id = target.id;

            let links =
                linked_accounts(vec![target], vec![account_ref(source(1), printed)], vec![]);

            assert_eq!(mapped(&links), vec![(source(1), Some(id))], "{printed}");
        }
    }

    #[test]
    fn a_whole_number_ending_one_accounts_longer_identifier_links() {
        for (stored, printed) in [
            ("GB65ALMB40116231926819", "31926819"),
            ("40017725", "089250/40017725"),
        ] {
            let target = account("Target", &[stored]);
            let id = target.id;
            let other = account("Other", &["GB04HAFE30941241887260"]);

            let links = linked_accounts(
                vec![other, target],
                vec![account_ref(source(1), printed)],
                vec![],
            );

            assert_eq!(mapped(&links), vec![(source(1), Some(id))], "{printed}");
        }
    }

    #[test]
    fn a_number_ending_two_accounts_identifiers_is_asked_about_with_both_first() {
        let named = account("Old 31926819", &[]);
        let first = account("First", &["GB65ALMB40116231926819"]);
        let second = account("Second", &["60161331926819"]);
        let ids = vec![first.id, second.id, named.id];

        let links = linked_accounts(
            vec![named, first, second],
            vec![account_ref(source(1), "31926819")],
            vec![],
        );

        assert!(links.account_mappings.is_empty());
        assert_eq!(candidate_ids(&links), ids);
    }

    #[test]
    fn a_masked_or_partial_number_never_links_but_offers_its_account_first() {
        for (printed, identifier) in [
            ("GB29 **** **** **** **68 19", "GB29NWBK60161331926819"),
            ("****6819", "GB29NWBK60161331926819"),
            ("**** **** **** 1234", "1234"),
            ("1234", "1234"),
            ("Card ending 1234", "1234"),
            ("Individual ...821", "4821"),
            ("Account 926819", "GB29NWBK60161331926819"),
        ] {
            let named = account("Card", &[]);
            let target = account("Target", &[identifier]);
            let unrelated = account("Unrelated", &["LT121000011101001000"]);
            let id = target.id;

            let links = linked_accounts(
                vec![named, unrelated, target],
                vec![account_ref(source(1), printed)],
                vec![],
            );

            assert!(links.account_mappings.is_empty(), "{printed}");
            assert_eq!(candidate_ids(&links)[0], id, "{printed}");
            assert!(candidate_ids(&links).len() <= 2, "{printed}");
        }
    }

    #[test]
    fn a_name_with_a_counter_a_year_or_a_rate_is_asked_with_the_earlier_answer_first() {
        for name in [
            "Account 1",
            "ISA 2025/26",
            "Saver 3.5%",
            "ISA 2 2025/26",
            "Pot 1 2025",
        ] {
            let other = account("Everyday", &["GB29NWBK60161331926819"]);
            let earlier = account("Old Bank Current", &[]);
            let id = earlier.id;

            let links = linked_accounts(
                vec![other, earlier],
                vec![account_ref(source(2), name)],
                vec![mapping(source(1), name, Some(id))],
            );

            assert!(links.account_mappings.is_empty(), "{name}");
            assert_eq!(candidate_ids(&links), vec![id], "{name}");
            assert!(!account_ask(&links).answer_covers_all_sources, "{name}");
            assert_eq!(
                links.sources_answered_with(source(2), name),
                vec![source(2)]
            );
        }
    }

    #[test]
    fn a_name_with_digits_reuses_another_files_answer() {
        for name in ["Card ****1234", "LT12 1000 0111 0100 1000"] {
            let target = Uuid::new_v4();

            let links = linked_accounts(
                vec![],
                vec![account_ref(source(1), name), account_ref(source(2), name)],
                vec![mapping(source(1), name, Some(target))],
            );

            assert_eq!(
                mapped(&links),
                vec![(source(1), Some(target)), (source(2), Some(target))],
                "{name}"
            );
            assert!(links.asks.is_empty(), "{name}");
        }
    }

    #[test]
    fn a_name_with_digits_answered_two_ways_is_asked_about_with_both_answers() {
        let first = account("First", &[]);
        let second = account("Second", &[]);
        let (first_id, second_id) = (first.id, second.id);

        let links = linked_accounts(
            vec![first, second],
            vec![account_ref(source(3), "Card ****1234")],
            vec![
                mapping(source(1), "Card ****1234", Some(first_id)),
                mapping(source(2), "Card ****1234", Some(second_id)),
            ],
        );

        assert_eq!(candidate_ids(&links), vec![first_id, second_id]);
        assert!(account_ask(&links).answer_covers_all_sources);
    }

    #[test]
    fn an_answer_outranks_a_matching_identifier() {
        let baltic = account("Baltic Bank", &["LT12 1000 0111 0100 1000"]);

        let links = linked_accounts(
            vec![baltic],
            vec![account_ref(source(1), "LT121000011101001000")],
            vec![mapping(source(1), "LT121000011101001000", None)],
        );

        assert_eq!(mapped(&links), vec![(source(1), None)]);
        assert!(links.asks.is_empty());
    }

    #[test]
    fn an_answer_to_a_name_with_digits_covers_every_file_asking_about_it() {
        let links = linked_accounts(
            vec![],
            vec![
                account_ref(source(1), "Card ****1234"),
                account_ref(source(2), "Card ****1234"),
                account_ref(source(3), "Current"),
                account_ref(source(4), "Current"),
            ],
            vec![],
        );

        assert_eq!(
            links.sources_answered_with(source(1), "Card ****1234"),
            vec![source(1), source(2)]
        );
        assert_eq!(
            links.sources_answered_with(source(3), "Current"),
            vec![source(3)]
        );
    }

    #[test]
    fn a_generic_name_is_asked_with_the_account_of_the_files_its_balances_continue_first() {
        let (other, northwind) = (account("Other bank", &[]), account("Northwind", &[]));
        let (other_id, northwind_id) = (other.id, northwind.id);

        let links = linked_with_balances(
            vec![other, northwind],
            vec![
                account_ref(source(3), "Current"),
                account_ref(source(4), "Current"),
                account_ref(source(5), "Current"),
            ],
            vec![
                mapping(source(1), "Current", Some(other_id)),
                mapping(source(2), "Current", Some(northwind_id)),
            ],
            vec![
                closing(source(1), "Current", date!(2026 - 02 - 28), dec!(52.40)),
                closing(source(2), "Current", date!(2026 - 02 - 28), dec!(1204.33)),
                opening(source(3), "Current", date!(2026 - 03 - 01), dec!(1204.33)),
                closing(source(3), "Current", date!(2026 - 03 - 29), dec!(88.10)),
                opening(source(4), "Current", date!(2026 - 04 - 03), dec!(88.1)),
                opening(source(5), "Current", date!(2026 - 03 - 01), dec!(700)),
            ],
        );

        let candidates: Vec<(Uuid, Vec<Uuid>)> = asks(&links)
            .into_iter()
            .filter_map(|ask| match ask {
                ImportAskDto::AccountMapping(ask) => Some((
                    ask.source_id,
                    ask.candidates
                        .iter()
                        .map(|candidate| candidate.id)
                        .collect(),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            candidates,
            vec![
                (source(3), vec![northwind_id, other_id]),
                (source(4), vec![northwind_id, other_id]),
                (source(5), vec![other_id, northwind_id]),
            ]
        );
    }

    #[test]
    fn two_accounts_under_one_generic_name_holding_the_same_amount_are_not_linked_as_one() {
        let (bank_one, bank_two) = (account("Bank one", &[]), account("Bank two", &[]));
        let one = bank_one.id;

        let links = linked_with_balances(
            vec![bank_one, bank_two],
            vec![account_ref(source(2), "Savings")],
            vec![mapping(source(1), "Savings", Some(one))],
            vec![
                closing(source(1), "Savings", date!(2026 - 01 - 31), dec!(10000.00)),
                opening(source(2), "Savings", date!(2026 - 02 - 03), dec!(10000.00)),
            ],
        );

        assert!(links.account_mappings.is_empty());
        assert_eq!(candidate_ids(&links), vec![one]);
    }

    #[test]
    fn a_file_starting_inside_an_answered_files_dates_continues_a_balance_of_that_day() {
        let (other, northwind) = (account("Other bank", &[]), account("Northwind", &[]));
        let (other_id, northwind_id) = (other.id, northwind.id);
        let stated = |on: Date, quantity: Decimal| {
            balance(
                source(2),
                "Current",
                ImportAssertionKindDto::Snapshot,
                on,
                quantity,
            )
        };

        let links = linked_with_balances(
            vec![other, northwind],
            vec![account_ref(source(3), "Current")],
            vec![
                mapping(source(1), "Current", Some(other_id)),
                mapping(source(2), "Current", Some(northwind_id)),
            ],
            vec![
                stated(date!(2026 - 02 - 10), dec!(52.40)),
                stated(date!(2026 - 02 - 14), dec!(61.00)),
                stated(date!(2026 - 02 - 14), dec!(47.15)),
                stated(date!(2026 - 02 - 14), dec!(30.00)),
                stated(date!(2026 - 02 - 20), dec!(12.00)),
                opening(source(3), "Current", date!(2026 - 02 - 17), dec!(47.15)),
            ],
        );

        assert_eq!(candidate_ids(&links), vec![northwind_id, other_id]);
    }

    #[test]
    fn balances_that_do_not_meet_leave_the_earlier_answers_in_their_order() {
        let continued = |ends: ImportAssertionDto, starts: ImportAssertionDto| {
            let (other, northwind) = (account("Other bank", &[]), account("Northwind", &[]));
            let (other_id, northwind_id) = (other.id, northwind.id);
            let links = linked_with_balances(
                vec![other, northwind],
                vec![account_ref(source(3), "Current")],
                vec![
                    mapping(source(1), "Current", Some(other_id)),
                    mapping(source(2), "Current", Some(northwind_id)),
                ],
                vec![ends, starts],
            );
            candidate_ids(&links) == vec![northwind_id, other_id]
        };
        let ends =
            |quantity: Decimal| closing(source(2), "Current", date!(2026 - 01 - 31), quantity);
        let starts = |on: Date, quantity: Decimal| opening(source(3), "Current", on, quantity);
        let in_dollars = |mut balance: ImportAssertionDto| {
            balance.asset_ref = Some("USD".to_string());
            balance
        };
        let market_value = |mut balance: ImportAssertionDto| {
            balance.kind = ImportAssertionKindDto::ValueTotal;
            balance
        };

        assert!(!continued(
            ends(dec!(250)),
            starts(date!(2026 - 02 - 01), dec!(250.01))
        ));
        assert!(!continued(
            ends(dec!(0)),
            starts(date!(2026 - 02 - 01), dec!(0))
        ));
        assert!(!continued(
            ends(dec!(250)),
            in_dollars(starts(date!(2026 - 02 - 01), dec!(250)))
        ));
        assert!(!continued(
            ends(dec!(250)),
            starts(date!(2026 - 02 - 09), dec!(250))
        ));
        assert!(!continued(
            ends(dec!(250)),
            starts(date!(2026 - 01 - 20), dec!(250))
        ));
        assert!(!continued(
            market_value(ends(dec!(250))),
            starts(date!(2026 - 02 - 01), dec!(250))
        ));
        assert!(continued(
            ends(dec!(250)),
            starts(date!(2026 - 02 - 08), dec!(250))
        ));
    }

    #[test]
    fn a_bare_ticker_is_asked_about_even_with_one_listing_in_the_currency_it_was_paid_in() {
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(trades(&[("NVDA", "USD"), ("ROAD", "USD"), ("LLOY", "USD")]));
        linking.add_asset_answers(vec![ImportAssetMappingModel {
            asset_ref: "USD".to_string(),
            asset_id: Some(USD),
        }]);

        assert_eq!(
            linking.imported_assets(),
            vec!["LLOY", "NVDA", "ROAD", "USD"]
        );
        assert_eq!(linking.unmapped_assets(), vec!["LLOY", "NVDA", "ROAD"]);
        assert_eq!(linking.answered_asset_ids(), vec![USD]);
        linking.add_asset_matches(vec![
            listing(1, "NVDA.TO", None, CAD),
            listing(2, "NVDA.NASDAQ", Some("US67066G1040"), USD),
            listing(3, "NVDA.XETRA", None, EUR),
            listing(4, "ROAD.NASDAQ", Some("US21044C1071"), GBP),
            listing(5, "ROAD.LSE", None, USD),
            listing(6, "LLOY.LSE", Some("GB0008706128"), USD),
            currency(USD, "USD"),
        ]);

        let links = linking.link();
        assert_eq!(mapped_assets(&links), vec![("USD", USD)]);
        assert_eq!(
            asset_questions(&links),
            vec![
                question("LLOY", &["LLOY.LSE"]),
                question("NVDA", &["NVDA.NASDAQ", "NVDA.TO", "NVDA.XETRA"]),
                question("ROAD", &["ROAD.LSE", "ROAD.NASDAQ"]),
            ]
        );
    }

    #[test]
    fn a_security_paid_for_in_two_currencies_is_asked_about() {
        let isin = "IE00B3XXRP09";
        let links = linked_assets(
            trades(&[(isin, "GBP"), (isin, "EUR")]),
            vec![
                listing(1, "VUSA.AS", Some(isin), EUR),
                listing(2, "VUSA.LSE", Some(isin), GBP),
                currency(EUR, "EUR"),
                currency(GBP, "GBP"),
            ],
        );

        assert_eq!(asked_tickers(&links), vec!["VUSA.AS", "VUSA.LSE"]);
    }

    #[test]
    fn an_isin_on_several_listings_links_to_the_only_one_quoted_in_the_currency_it_was_paid_in() {
        let isin = "IE00B3XXRP09";
        let listings = || {
            vec![
                listing(1, "VUSA.AS", Some(isin), EUR),
                listing(2, "VUSA.LSE", Some(isin), GBP),
                listing(3, "VUSD.LSE", Some(isin), USD),
                listing(4, "VUSA.XETRA", Some(isin), EUR),
                currency(GBP, "GBP"),
                currency(EUR, "EUR"),
            ]
        };
        let dividend = ImportAssetRefModel {
            counter_asset_ref: Some("GBP".to_string()),
            ..asset_ref(source(1), "ISA", isin)
        };

        let bought = linked_assets(trades(&[(isin, "GBP")]), listings());
        let paid_a_dividend = linked_assets(
            vec![dividend, asset_ref(source(1), "ISA", "GBP")],
            listings(),
        );
        let bought_in_euros = linked_assets(trades(&[(isin, "EUR")]), listings());

        assert_eq!(mapped_assets(&bought), vec![("GBP", GBP), (isin, 2)]);
        assert_eq!(
            mapped_assets(&paid_a_dividend),
            vec![("GBP", GBP), (isin, 2)]
        );
        assert_eq!(
            asked_tickers(&bought_in_euros),
            vec!["VUSA.AS", "VUSA.XETRA", "VUSA.LSE", "VUSD.LSE"]
        );
    }

    #[test]
    fn a_currency_code_listings_share_is_money_when_it_pays_for_a_security_quoted_in_it() {
        let isin = "US0378331005";
        let matches = || {
            vec![
                currency(4, "AMD"),
                listing(5, "AMD.XETRA", None, EUR),
                listing(6, "AMD.NASDAQ", None, USD),
                listing(7, "AAPL.NASDAQ", Some(isin), USD),
                listing(8, "USD.NYSE ARCA", None, USD),
                listing(9, "XLNX.NASDAQ", None, USD),
                currency(USD, "USD"),
            ]
        };

        let bought = held_at_a_broker(trades(&[("AMD", "USD"), (isin, "USD")]), matches());
        let merged = held_at_a_broker(trades(&[("AMD", "XLNX")]), matches());

        assert_eq!(mapped_assets(&bought), vec![(isin, 7), ("USD", USD)]);
        assert_eq!(
            asset_questions(&bought),
            vec![question("AMD", &["AMD.NASDAQ", "AMD", "AMD.XETRA"])]
        );
        assert!(merged.asset_mappings.is_empty());
        assert_eq!(
            asset_questions(&merged),
            vec![
                question("AMD", &["AMD", "AMD.XETRA", "AMD.NASDAQ"]),
                question("XLNX", &["XLNX.NASDAQ"]),
            ]
        );
    }

    #[test]
    fn a_currency_code_listings_share_is_asked_about_where_it_could_be_a_holding() {
        let matches = || {
            vec![
                currency(4, "AMD"),
                listing(6, "AMD.NASDAQ", None, USD),
                listing(7, "AAPL.NASDAQ", Some("US0378331005"), USD),
                listing(8, "USD.NYSE ARCA", None, USD),
                currency(USD, "USD"),
            ]
        };
        let stated = |assets: &[&str]| {
            assets
                .iter()
                .map(|asset| asset_ref(source(1), "Broker", asset))
                .collect::<Vec<_>>()
        };

        let on_a_securities_account = held_at_a_broker(stated(&["AMD", "USD"]), matches());
        let beside_a_security = linked_assets(stated(&["AMD", "US0378331005"]), matches());
        let beside_an_unknown_name = linked_assets(stated(&["AMD", "GBX"]), matches());
        let beside_currencies = linked_assets(stated(&["AMD", "USD"]), matches());

        assert!(on_a_securities_account.asset_mappings.is_empty());
        assert_eq!(
            asset_questions(&on_a_securities_account),
            vec![
                question("AMD", &["AMD", "AMD.NASDAQ"]),
                question("USD", &["USD", "USD.NYSE ARCA"]),
            ]
        );
        assert_eq!(mapped_assets(&beside_a_security), vec![("US0378331005", 7)]);
        assert!(mapped_assets(&beside_an_unknown_name).is_empty());
        assert_eq!(
            mapped_assets(&beside_currencies),
            vec![("AMD", 4), ("USD", USD)]
        );
    }

    #[test]
    fn a_cash_name_the_catalogue_lacks_shows_nothing_until_it_is_answered() {
        let isin = "IE00B3XXRP09";
        let matches = || {
            vec![
                crypto(1, "META"),
                listing(2, "META.NASDAQ", None, USD),
                listing(3, "META.LSE", None, GBP),
                listing(4, "VUSA.AS", Some(isin), EUR),
                listing(5, "VUSA.LSE", Some(isin), GBP),
                currency(GBP, "GBP"),
            ]
        };
        let bought = || trades(&[("META", "GBX"), (isin, "GBX")]);

        let unanswered = held_at_a_broker(bought(), matches());
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(bought());
        linking.add_asset_answers(vec![ImportAssetMappingModel {
            asset_ref: "GBX".to_string(),
            asset_id: Some(GBP),
        }]);
        linking.add_asset_matches(matches());
        let answered = linking.link();

        assert!(unanswered.asset_mappings.is_empty());
        assert_eq!(
            asset_questions(&unanswered),
            vec![
                question("GBX", &[]),
                question(isin, &["VUSA.AS", "VUSA.LSE"]),
                question("META", &["META", "META.NASDAQ", "META.LSE"]),
            ]
        );
        assert_eq!(mapped_assets(&answered), vec![(isin, 5), ("GBX", GBP)]);
        assert_eq!(
            asset_questions(&answered),
            vec![question("META", &["META", "META.LSE", "META.NASDAQ"])]
        );
    }

    #[test]
    fn an_asset_of_the_users_own_whose_symbol_listings_share_is_asked_about_with_it_first() {
        let links = linked_assets(
            trades(&[("VUSA", "GBP"), ("MYFUND", "GBP")]),
            vec![
                asset(900, "VUSA", None),
                asset(901, "MYFUND", None),
                listing(1, "VUSA.LSE", None, GBP),
                listing(2, "VUSA.AS", None, EUR),
                currency(GBP, "GBP"),
            ],
        );

        assert_eq!(mapped_assets(&links), vec![("GBP", GBP), ("MYFUND", 901)]);
        assert_eq!(
            asset_questions(&links),
            vec![question("VUSA", &["VUSA", "VUSA.LSE", "VUSA.AS"])]
        );
    }

    #[test]
    fn a_crypto_symbol_bought_with_cash_links_on_a_crypto_wallet_despite_listed_namesakes() {
        let linked_on = |account_type: DatabaseAccountTypes, answered: bool| {
            let mut holder = account("Jordan Example", &[]);
            holder.account_type.id = account_type_id(account_type);
            let id = holder.id;
            let mut linking = SessionLinking::new();
            linking.add_accounts(vec![holder]);
            linking.add_account_refs(vec![account_ref(source(1), "Broker")]);
            if answered {
                linking.add_account_answers(vec![mapping(source(1), "Broker", Some(id))]);
            }
            linking.link_accounts();
            linking.add_asset_refs(trades(&[("BTC", "EUR")]));
            linking.add_asset_matches(vec![
                crypto(1, "BTC"),
                listing(2, "BTC.PA", Some("FR0014002IH8"), EUR),
                currency(EUR, "EUR"),
            ]);
            mapped_assets(&linking.link())
                .into_iter()
                .map(|(asset_ref, id)| (asset_ref.to_string(), id))
                .collect::<Vec<_>>()
        };
        let euros = ("EUR".to_string(), EUR);

        assert_eq!(
            linked_on(DatabaseAccountTypes::CryptoWallet, true),
            vec![("BTC".to_string(), 1), euros.clone()]
        );
        assert_eq!(
            linked_on(DatabaseAccountTypes::Investment, true),
            vec![euros.clone()]
        );
        assert_eq!(
            linked_on(DatabaseAccountTypes::CryptoWallet, false),
            vec![euros]
        );
    }

    #[test]
    fn a_crypto_symbol_listings_share_links_when_swapped_for_what_can_only_be_a_coin() {
        let matches = || {
            vec![
                crypto(1, "BTC"),
                listing(2, "BTC.PA", Some("FR0014002IH8"), EUR),
                crypto(3, "ETH"),
                listing(4, "ETH.NYSE ARCA", None, USD),
                crypto(5, "USDT"),
                currency(EUR, "EUR"),
            ]
        };

        let bought = linked_assets(trades(&[("BTC", "EUR")]), matches());
        let for_a_coin = linked_assets(trades(&[("BTC", "EUR"), ("BTC", "USDT")]), matches());
        let for_a_shared_symbol = linked_assets(trades(&[("ETH", "BTC")]), matches());
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(trades(&[("ETH", "BTC")]));
        linking.add_asset_answers(vec![ImportAssetMappingModel {
            asset_ref: "BTC".to_string(),
            asset_id: Some(1),
        }]);
        linking.add_asset_matches(matches());
        let for_an_answered_coin = linking.link();

        assert_eq!(asked_tickers(&bought), vec!["BTC", "BTC.PA"]);
        assert_eq!(
            mapped_assets(&for_a_coin),
            vec![("BTC", 1), ("EUR", EUR), ("USDT", 5)]
        );
        assert!(for_a_shared_symbol.asset_mappings.is_empty());
        assert_eq!(
            mapped_assets(&for_an_answered_coin),
            vec![("ETH", 3), ("BTC", 1)]
        );
    }

    #[test]
    fn a_symbol_with_the_statements_own_exchange_suffix_is_offered_the_symbols_listings() {
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(trades(&[
            ("XRAY.US", "USD"),
            ("SPYL.DE", "EUR"),
            ("TXT.PL", "PLN"),
            ("VUSA.LSE", "EUR"),
        ]));

        assert_eq!(
            linking.asset_search_terms(),
            vec![
                "EUR", "PLN", "SPYL", "SPYL.DE", "TXT", "TXT.PL", "USD", "VUSA", "VUSA.LSE",
                "XRAY", "XRAY.US"
            ]
        );
        linking.add_asset_matches(vec![
            listing(1, "XRAY.NASDAQ", Some("US24906P1093"), USD),
            listing(2, "SPYL.AS", Some("IE000XZSV718"), EUR),
            listing(3, "SPYL.LSE", Some("IE000XZSV718"), USD),
            listing(4, "SPYL.XETRA", Some("IE000XZSV718"), EUR),
            listing(5, "TXT.NYSE", Some("US8832031012"), USD),
            listing(6, "TXT.WAR", Some("PLLVTSF00010"), PLN),
            listing(7, "VUSA.LSE", None, GBP),
            listing(8, "VUSA.AS", None, EUR),
            currency(USD, "USD"),
            currency(EUR, "EUR"),
            currency(PLN, "PLN"),
        ]);

        let links = linking.link();
        assert_eq!(
            mapped_assets(&links),
            vec![("EUR", EUR), ("PLN", PLN), ("USD", USD), ("VUSA.LSE", 7)]
        );
        assert_eq!(
            asset_questions(&links),
            vec![
                question("SPYL.DE", &["SPYL.AS", "SPYL.XETRA", "SPYL.LSE"]),
                question("TXT.PL", &["TXT.WAR", "TXT.NYSE"]),
                question("XRAY.US", &["XRAY.NASDAQ"]),
            ]
        );
    }

    #[test]
    fn an_isin_is_found_inside_a_name_by_its_shape_and_check_digit() {
        assert_eq!(printed_isin("US0378331005"), Some("US0378331005"));
        assert_eq!(printed_isin("US67066G1040 NVDA"), Some("US67066G1040"));
        assert_eq!(
            printed_isin("Apple Inc (US0378331005)"),
            Some("US0378331005")
        );
        assert_eq!(printed_isin("ie000xzsv718"), Some("ie000xzsv718"));
        assert_eq!(printed_isin("US0378331006"), None);
        assert_eq!(printed_isin("ACCUMULATING"), None);
        assert_eq!(printed_isin("100378331005"), None);
        assert_eq!(printed_isin("AAPL"), None);
    }

    #[test]
    fn a_ticker_printed_with_an_isin_narrows_its_listings_to_that_symbol() {
        let asml = "NL0010273215";
        let apple = "US0378331005";
        let matches = || {
            vec![
                listing(1, "ASML.AS", Some(asml), EUR),
                listing(2, "ASME.F", Some(asml), EUR),
                listing(3, "ASME.XETRA", Some(asml), EUR),
                listing(4, "ASML.NASDAQ", Some("USN070592100"), USD),
                listing(5, "APC.F", Some(apple), EUR),
                listing(6, "AAPL.MX", Some(apple), 89),
                listing(7, "AAPL.NASDAQ", Some(apple), USD),
                currency(EUR, "EUR"),
            ]
        };
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(trades(&[("NL0010273215 ASML", "EUR")]));

        assert_eq!(
            linking.asset_search_terms(),
            vec!["ASML", "EUR", "NL0010273215", "NL0010273215 ASML"]
        );
        linking.add_asset_matches(matches());
        let bought = linking.link();
        let held = linked_assets(
            vec![asset_ref(source(1), "ISA", "AAPL (US0378331005)")],
            matches(),
        );
        let named_otherwise =
            linked_assets(trades(&[("NL0010273215 ASML Holding", "EUR")]), matches());

        assert_eq!(
            mapped_assets(&bought),
            vec![("EUR", EUR), ("NL0010273215 ASML", 1)]
        );
        assert!(held.asset_mappings.is_empty());
        assert_eq!(
            asked_tickers(&held),
            vec!["AAPL.MX", "AAPL.NASDAQ", "APC.F"]
        );
        assert_eq!(
            mapped_assets(&named_otherwise),
            vec![("EUR", EUR), ("NL0010273215 ASML Holding", 1)]
        );
    }

    #[test]
    fn a_ticker_beside_an_isin_the_catalogue_lacks_is_offered_and_never_links() {
        let links = linked_assets(
            trades(&[("US67066G1040 NVDA", "USD")]),
            vec![
                listing(1, "NVDA.NASDAQ", None, USD),
                listing(2, "NVDA.TO", Some("CA67080D1087"), CAD),
                currency(USD, "USD"),
            ],
        );

        assert_eq!(mapped_assets(&links), vec![("USD", USD)]);
        assert_eq!(asked_tickers(&links), vec!["NVDA.NASDAQ"]);
    }

    #[test]
    fn listings_in_the_paid_currency_are_asked_with_the_exchange_trading_in_it_first() {
        let isin = "US0378331005";
        let exchange = |exchange: &str, base_pair_id: i32, listings: i64| ExchangeCurrencyModel {
            exchange: exchange.to_string(),
            base_pair_id,
            listings,
        };
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(trades(&[(isin, "USD")]));
        linking.add_asset_matches(vec![
            listing(1, "APC.XETRA", Some(isin), EUR),
            listing(2, "AAPL.LIM", Some(isin), USD),
            listing(3, "0R2V.LSE", Some(isin), USD),
            listing(4, "AAPL.NASDAQ", Some(isin), USD),
            currency(USD, "USD"),
        ]);

        assert_eq!(
            linking.exchanges_of_candidates(),
            vec!["LIM", "LSE", "NASDAQ", "XETRA"]
        );
        linking.add_exchange_currencies(vec![
            exchange("LIM", 100, 33),
            exchange("LIM", USD, 12),
            exchange("LSE", USD, 2179),
            exchange("LSE", GBP, 3329),
            exchange("NASDAQ", USD, 5326),
            exchange("XETRA", EUR, 3577),
        ]);

        let links = linking.link();
        assert_eq!(mapped_assets(&links), vec![("USD", USD)]);
        assert_eq!(
            asked_tickers(&links),
            vec!["AAPL.NASDAQ", "AAPL.LIM", "0R2V.LSE", "APC.XETRA"]
        );
    }

    #[test]
    fn an_unknown_asset_is_asked_with_the_search_suggestions() {
        let mut linking = SessionLinking::new();
        linking.add_asset_refs(vec![asset_ref(source(1), "ISA", "VUSA")]);

        assert_eq!(linking.assets_needing_suggestions(), vec!["VUSA"]);
        linking.add_asset_suggestions(
            "VUSA".to_string(),
            (1..=8).map(|id| search_result(id, "VUSA.X")).collect(),
        );

        let links = linking.link();
        assert_eq!(links.asks[0].ask_key, asset_ask_key("VUSA"));
        match asks(&links).as_slice() {
            [ImportAskDto::AssetMapping(ask)] => {
                assert_eq!(ask.candidates.len(), ASSET_SUGGESTIONS);
                assert_eq!(
                    ask.choices,
                    vec![ImportAskChoiceDto::Map, ImportAskChoiceDto::Exclude]
                );
            }
            _ => panic!("expected one asset question"),
        }
    }

    #[test]
    fn assets_only_on_lines_marked_dont_import_are_never_asked_about() {
        let mut linking = SessionLinking::new();
        linking.add_account_refs(vec![
            account_ref(source(1), "Pocket"),
            account_ref(source(1), "Current"),
        ]);
        linking.add_account_answers(vec![
            mapping(source(1), "Pocket", None),
            mapping(source(1), "Current", Some(Uuid::new_v4())),
        ]);
        linking.add_asset_refs(vec![
            asset_ref(source(1), "Pocket", "ZZZQ"),
            asset_ref(source(1), "Current", "VUSA"),
            asset_ref(source(1), "Pocket", "VUSA"),
        ]);
        linking.link_accounts();

        assert_eq!(linking.unmapped_assets(), vec!["VUSA"]);
    }

    fn parsed_line(
        source_n: u128,
        key: Option<&str>,
        account: &str,
        legs: Vec<ImportParsedLegDto>,
    ) -> ImportParsedLineDto {
        ImportParsedLineDto {
            id: Uuid::new_v4(),
            source_id: source(source_n),
            transaction_key: key.map(str::to_string),
            account_ref: account.to_string(),
            date: OffsetDateTime::UNIX_EPOCH,
            description: None,
            related_asset_ref: None,
            external_id: None,
            legs,
        }
    }

    fn parsed_leg(
        asset_ref: &str,
        quantity: i64,
        fee_type: Option<FeeEntryTypesDto>,
    ) -> ImportParsedLegDto {
        ImportParsedLegDto {
            asset_ref: asset_ref.to_string(),
            quantity: Decimal::from(quantity),
            fee_type,
        }
    }

    fn gbp() -> ImportAssetMappingModel {
        ImportAssetMappingModel {
            asset_ref: "GBP".to_string(),
            asset_id: Some(45),
        }
    }

    fn left_out(asset_ref: &str) -> ImportAssetMappingModel {
        ImportAssetMappingModel {
            asset_ref: asset_ref.to_string(),
            asset_id: None,
        }
    }

    fn questions(lines: &[ImportPendingLineDto]) -> Vec<(Uuid, Vec<String>)> {
        lines
            .iter()
            .map(|line| (line.id, line.blocked_by.clone()))
            .collect()
    }

    #[test]
    fn a_pending_line_carries_its_legs_and_each_blocking_question_once() {
        let links = SessionLinks {
            asset_mappings: vec![gbp()],
            ..SessionLinks::default()
        };
        let mut line = parsed_line(
            1,
            None,
            "Current",
            vec![
                parsed_leg("GBP", -550, None),
                parsed_leg("GBP", -1, Some(FeeEntryTypesDto::Transaction)),
                parsed_leg("VUSA", 3, None),
            ],
        );
        line.related_asset_ref = Some("VUSA".to_string());

        let lines = links.pending_lines(vec![line]);

        let [line] = lines.as_slice() else {
            panic!("expected one line");
        };
        assert_eq!(line.legs.len(), 3);
        assert_eq!(line.legs[1].fee_type, Some(FeeEntryTypesDto::Transaction));
        assert_eq!(
            line.blocked_by,
            vec![account_ask_key(source(1), "Current"), asset_ask_key("VUSA")]
        );
    }

    #[test]
    fn a_sibling_of_a_blocked_line_waits_on_the_same_question() {
        let links = SessionLinks {
            account_mappings: vec![mapping(source(1), "Current", Some(Uuid::from_u128(7)))],
            asset_mappings: vec![gbp()],
            ..SessionLinks::default()
        };
        let trade = parsed_line(
            1,
            Some("t1"),
            "Current",
            vec![parsed_leg("GBP", -10, None), parsed_leg("ZZZQ", 1, None)],
        );
        let fee = parsed_line(
            1,
            Some("t1"),
            "Current",
            vec![parsed_leg("GBP", -1, Some(FeeEntryTypesDto::Transaction))],
        );

        let lines = links.pending_lines(vec![trade.clone(), fee.clone()]);

        assert_eq!(
            questions(&lines),
            vec![
                (trade.id, vec![asset_ask_key("ZZZQ")]),
                (fee.id, vec![asset_ask_key("ZZZQ")]),
            ]
        );
    }

    #[test]
    fn a_siblings_questions_come_only_from_its_own_group() {
        let links = SessionLinks {
            account_mappings: vec![
                mapping(source(1), "Current", Some(Uuid::from_u128(7))),
                mapping(source(1), "Ignored", None),
                mapping(source(2), "Current", Some(Uuid::from_u128(7))),
            ],
            ..SessionLinks::default()
        };
        let mapped_line = parsed_line(1, Some("t1"), "Current", vec![]);
        let keyless = parsed_line(1, None, "Current", vec![parsed_leg("DDD", 1, None)]);
        let other_keyless = parsed_line(1, None, "Current", vec![parsed_leg("AAA", 1, None)]);
        let excluded_sibling =
            parsed_line(1, Some("t1"), "Ignored", vec![parsed_leg("BBB", 1, None)]);
        let other_source = parsed_line(2, Some("t1"), "Current", vec![parsed_leg("CCC", 1, None)]);

        let lines = links.pending_lines(vec![
            mapped_line,
            keyless.clone(),
            other_keyless.clone(),
            excluded_sibling,
            other_source.clone(),
        ]);

        assert_eq!(
            questions(&lines),
            vec![
                (keyless.id, vec![asset_ask_key("DDD")]),
                (other_keyless.id, vec![asset_ask_key("AAA")]),
                (other_source.id, vec![asset_ask_key("CCC")]),
            ]
        );
    }

    #[test]
    fn a_dont_import_line_is_never_pending_even_in_a_blocked_group() {
        let links = SessionLinks {
            account_mappings: vec![
                mapping(source(1), "Current", Some(Uuid::from_u128(7))),
                mapping(source(1), "Ignored", None),
            ],
            ..SessionLinks::default()
        };
        let blocked = parsed_line(1, Some("t1"), "Current", vec![parsed_leg("ZZZQ", 1, None)]);
        let excluded = parsed_line(1, Some("t1"), "Ignored", vec![parsed_leg("NVDA", 1, None)]);

        let lines = links.pending_lines(vec![blocked.clone(), excluded]);

        assert_eq!(
            questions(&lines),
            vec![(blocked.id, vec![asset_ask_key("ZZZQ")])]
        );
    }

    #[test]
    fn a_leg_in_a_left_out_asset_is_neither_shown_nor_waited_on() {
        let links = SessionLinks {
            asset_mappings: vec![gbp(), left_out("SPX500")],
            ..SessionLinks::default()
        };
        let trade = parsed_line(
            1,
            None,
            "Current",
            vec![parsed_leg("GBP", -550, None), parsed_leg("SPX500", 1, None)],
        );

        let lines = links.pending_lines(vec![trade]);

        let [line] = lines.as_slice() else {
            panic!("expected one line");
        };
        assert_eq!(line.blocked_by, vec![account_ask_key(source(1), "Current")]);
        let [leg] = line.legs.as_slice() else {
            panic!("expected one leg");
        };
        assert_eq!(leg.asset_ref, "GBP");
    }

    #[test]
    fn a_line_moving_only_left_out_assets_is_never_pending_nor_holds_back_its_group() {
        let links = SessionLinks {
            account_mappings: vec![mapping(source(1), "Current", Some(Uuid::from_u128(7)))],
            asset_mappings: vec![gbp(), left_out("SPX500")],
            ..SessionLinks::default()
        };
        let units = parsed_line(
            1,
            Some("t1"),
            "Unanswered",
            vec![parsed_leg("SPX500", 1, None)],
        );
        let cash = parsed_line(
            1,
            Some("t1"),
            "Current",
            vec![parsed_leg("GBP", -550, None)],
        );
        let blocked = parsed_line(1, Some("t2"), "Current", vec![parsed_leg("ZZZQ", 1, None)]);
        let beside_blocked = parsed_line(
            1,
            Some("t2"),
            "Current",
            vec![parsed_leg("SPX500", -1, None)],
        );

        let lines = links.pending_lines(vec![units, cash, blocked.clone(), beside_blocked]);

        assert_eq!(
            questions(&lines),
            vec![(blocked.id, vec![asset_ask_key("ZZZQ")])]
        );
    }
}
