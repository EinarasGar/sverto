use std::collections::HashSet;

use dal::models::asset_models::{AssetByIsin, AssetByRootSymbol};
use uuid::Uuid;

use crate::dtos::portfolio::holding::HoldingDto;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetCandidate {
    pub id: i32,
    pub base_currency: Option<String>,
}

impl From<AssetByIsin> for AssetCandidate {
    fn from(row: AssetByIsin) -> Self {
        Self {
            id: row.id,
            base_currency: row.base_currency,
        }
    }
}

impl From<AssetByRootSymbol> for AssetCandidate {
    fn from(row: AssetByRootSymbol) -> Self {
        Self {
            id: row.id,
            base_currency: row.base_currency,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct HeldAssets {
    in_account: HashSet<i32>,
    by_user: HashSet<i32>,
}

impl HeldAssets {
    pub(crate) fn new(holdings: &[HoldingDto], account_id: Uuid) -> Self {
        Self {
            in_account: holdings
                .iter()
                .filter(|holding| holding.account_id == account_id)
                .map(|holding| holding.asset_id)
                .collect(),
            by_user: holdings.iter().map(|holding| holding.asset_id).collect(),
        }
    }

    /// Tiers: held in the account, held by the user, quoted in `currency`, the only candidate.
    pub(crate) fn select(&self, candidates: &[AssetCandidate], currency: &str) -> Option<i32> {
        fn single<'a>(mut matches: impl Iterator<Item = &'a AssetCandidate>) -> Option<i32> {
            match (matches.next(), matches.next()) {
                (Some(only), None) => Some(only.id),
                _ => None,
            }
        }

        let quoted_in_currency = |candidate: &&AssetCandidate| {
            candidate
                .base_currency
                .as_deref()
                .is_some_and(|base| base.eq_ignore_ascii_case(currency))
        };

        for held in [&self.in_account, &self.by_user] {
            let held: Vec<&AssetCandidate> = candidates
                .iter()
                .filter(|candidate| held.contains(&candidate.id))
                .collect();
            match held.as_slice() {
                [] => continue,
                [only] => return Some(only.id),
                _ => return single(held.into_iter().filter(quoted_in_currency)),
            }
        }
        let quoted: Vec<&AssetCandidate> = candidates.iter().filter(quoted_in_currency).collect();
        if quoted.is_empty() {
            single(candidates.iter())
        } else {
            single(quoted.into_iter())
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    const ACCOUNT: Uuid = Uuid::from_u128(1);
    const OTHER_ACCOUNT: Uuid = Uuid::from_u128(2);

    fn candidate(id: i32, base_currency: &str) -> AssetCandidate {
        AssetCandidate {
            id,
            base_currency: Some(base_currency.to_string()),
        }
    }

    fn held(in_account: &[i32], in_other_account: &[i32]) -> HeldAssets {
        let holding = |asset_id: i32, account_id: Uuid| HoldingDto {
            asset_id,
            account_id,
            units: Decimal::ONE,
        };
        let holdings: Vec<HoldingDto> = in_account
            .iter()
            .map(|&id| holding(id, ACCOUNT))
            .chain(
                in_other_account
                    .iter()
                    .map(|&id| holding(id, OTHER_ACCOUNT)),
            )
            .collect();
        HeldAssets::new(&holdings, ACCOUNT)
    }

    #[test]
    fn prefers_asset_held_in_account() {
        let candidates = [
            candidate(1, "EUR"),
            candidate(2, "GBP"),
            candidate(3, "EUR"),
        ];
        assert_eq!(held(&[2], &[1]).select(&candidates, "EUR"), Some(2));
    }

    #[test]
    fn falls_back_to_asset_held_in_other_account() {
        let candidates = [candidate(1, "CZK"), candidate(2, "EUR")];
        assert_eq!(held(&[], &[1]).select(&candidates, "EUR"), Some(1));
    }

    #[test]
    fn narrows_several_holdings_by_currency() {
        let candidates = [candidate(1, "CZK"), candidate(2, "EUR")];
        assert_eq!(held(&[1, 2], &[]).select(&candidates, "eur"), Some(2));
    }

    #[test]
    fn ambiguous_holdings_do_not_fall_through_to_unheld_assets() {
        let candidates = [
            candidate(1, "CZK"),
            candidate(2, "GBP"),
            candidate(3, "EUR"),
        ];
        assert_eq!(held(&[1, 2], &[]).select(&candidates, "EUR"), None);
    }

    #[test]
    fn picks_unheld_asset_quoted_in_transaction_currency() {
        let candidates = [candidate(1, "CZK"), candidate(2, "EUR")];
        assert_eq!(held(&[], &[]).select(&candidates, "EUR"), Some(2));
    }

    #[test]
    fn picks_the_only_candidate_whatever_its_currency() {
        assert_eq!(
            held(&[], &[]).select(&[candidate(1, "GBP")], "USD"),
            Some(1)
        );
    }

    #[test]
    fn leaves_ambiguous_or_unmatched_currency_unresolved() {
        let candidates = [
            candidate(1, "EUR"),
            candidate(2, "EUR"),
            candidate(3, "CZK"),
        ];
        assert_eq!(held(&[], &[]).select(&candidates, "EUR"), None);
        assert_eq!(held(&[], &[]).select(&candidates, "USD"), None);
    }
}
