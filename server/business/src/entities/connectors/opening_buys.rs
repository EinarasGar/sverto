use std::collections::{BTreeMap, HashMap, HashSet};

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::dtos::entry_dto::EntryDto;
use crate::dtos::transaction_dto::{
    AssetPurchaseMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
};

pub(crate) const SYNTHETIC_OPENING_BUY_PREFIX: &str = "synthetic:opening:";

/// Shortfalls at or below this many shares are fractional-share rounding dust, not missing buys.
const SHORTFALL_EPSILON: Decimal = dec!(0.000001);

const CASH_AMOUNT_DECIMAL_PLACES: u32 = 8;

#[derive(Clone, Debug)]
pub(crate) struct PositionEvent {
    pub asset_id: i32,
    pub transaction_id: Uuid,
    pub date: OffsetDateTime,
    pub quantity: Decimal,
    /// Set for sells the binding imported: only those get synthetic buys.
    pub sell_external_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequiredOpeningBuy {
    pub asset_id: i32,
    pub sell_transaction_id: Uuid,
    pub external_id: String,
    pub quantity: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExistingOpeningBuy {
    pub transaction_id: Uuid,
    pub external_id: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct OpeningBuyPlan {
    pub create: Vec<RequiredOpeningBuy>,
    pub keep: Vec<(ExistingOpeningBuy, RequiredOpeningBuy)>,
    pub delete: Vec<ExistingOpeningBuy>,
    pub owned_by_user: Vec<RequiredOpeningBuy>,
}

pub(crate) fn synthetic_external_id(sell_external_id: &str) -> String {
    format!("{SYNTHETIC_OPENING_BUY_PREFIX}{sell_external_id}")
}

pub(crate) fn is_synthetic_external_id(external_id: &str) -> bool {
    external_id.starts_with(SYNTHETIC_OPENING_BUY_PREFIX)
}

/// One buy per sell taking the position below -epsilon; the position then resets to zero.
pub(crate) fn required_opening_buys(events: &[PositionEvent]) -> Vec<RequiredOpeningBuy> {
    let mut netted: BTreeMap<(i32, Uuid), PositionEvent> = BTreeMap::new();
    for event in events {
        netted
            .entry((event.asset_id, event.transaction_id))
            .and_modify(|existing| {
                existing.quantity += event.quantity;
                if existing.sell_external_id.is_none() {
                    existing.sell_external_id = event.sell_external_id.clone();
                }
            })
            .or_insert_with(|| event.clone());
    }

    let mut by_asset: BTreeMap<i32, Vec<PositionEvent>> = BTreeMap::new();
    for event in netted.into_values() {
        by_asset.entry(event.asset_id).or_default().push(event);
    }

    let mut required = Vec::new();
    for (asset_id, mut timeline) in by_asset {
        timeline.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| b.quantity.cmp(&a.quantity))
                .then_with(|| a.transaction_id.cmp(&b.transaction_id))
        });

        let mut position = Decimal::ZERO;
        for event in timeline {
            position += event.quantity;
            if position >= -SHORTFALL_EPSILON {
                continue;
            }
            if let Some(sell_external_id) = &event.sell_external_id {
                required.push(RequiredOpeningBuy {
                    asset_id,
                    sell_transaction_id: event.transaction_id,
                    external_id: synthetic_external_id(sell_external_id),
                    quantity: -position,
                });
            }
            position = Decimal::ZERO;
        }
    }
    required
}

/// Buys in `taken_external_ids` (edited or deleted by the user) are never recreated.
pub(crate) fn plan_opening_buys(
    required: Vec<RequiredOpeningBuy>,
    existing: Vec<ExistingOpeningBuy>,
    taken_external_ids: &HashSet<String>,
) -> OpeningBuyPlan {
    let mut existing_by_id: HashMap<String, ExistingOpeningBuy> = existing
        .into_iter()
        .map(|buy| (buy.external_id.clone(), buy))
        .collect();

    let mut plan = OpeningBuyPlan::default();
    for buy in required {
        match existing_by_id.remove(&buy.external_id) {
            Some(existing) => plan.keep.push((existing, buy)),
            None if taken_external_ids.contains(&buy.external_id) => plan.owned_by_user.push(buy),
            None => plan.create.push(buy),
        }
    }
    plan.delete = existing_by_id.into_values().collect();
    plan.delete
        .sort_by(|a, b| a.external_id.cmp(&b.external_id));
    plan
}

impl RequiredOpeningBuy {
    pub fn to_transaction_dto(
        &self,
        sell_quantity: Decimal,
        sell_proceeds: &EntryDto,
        date: OffsetDateTime,
    ) -> Option<TransactionDto> {
        if sell_quantity.is_zero() {
            return None;
        }
        let cash_amount = (self.quantity * sell_proceeds.quantity.abs() / sell_quantity.abs())
            .round_dp(CASH_AMOUNT_DECIMAL_PLACES);

        Some(TransactionDto {
            transaction_id: None,
            date,
            visibility: TransactionVisibilityDto::Action,
            fee_entries: vec![],
            transaction_type: TransactionTypeDto::AssetPurchase(AssetPurchaseMetadataDto {
                purchase: EntryDto::new(self.asset_id, sell_proceeds.account_id, self.quantity),
                sale: EntryDto::new(
                    sell_proceeds.asset_id,
                    sell_proceeds.account_id,
                    -cash_amount,
                ),
            }),
        })
    }
}

pub(crate) fn opening_buy_changed(existing: &TransactionDto, desired: &TransactionDto) -> bool {
    let (
        TransactionTypeDto::AssetPurchase(existing_purchase),
        TransactionTypeDto::AssetPurchase(desired_purchase),
    ) = (&existing.transaction_type, &desired.transaction_type)
    else {
        return true;
    };
    existing.date != desired.date
        || existing_purchase.purchase.asset_id != desired_purchase.purchase.asset_id
        || existing_purchase.purchase.quantity != desired_purchase.purchase.quantity
        || existing_purchase.sale.asset_id != desired_purchase.sale.asset_id
        || existing_purchase.sale.quantity != desired_purchase.sale.quantity
        || !existing.fee_entries.is_empty()
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    const ASSET: i32 = 42;
    const OTHER_ASSET: i32 = 43;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn buy(tx: u128, date: OffsetDateTime, quantity: Decimal) -> PositionEvent {
        PositionEvent {
            asset_id: ASSET,
            transaction_id: id(tx),
            date,
            quantity,
            sell_external_id: None,
        }
    }

    fn sell(tx: u128, date: OffsetDateTime, quantity: Decimal) -> PositionEvent {
        PositionEvent {
            asset_id: ASSET,
            transaction_id: id(tx),
            date,
            quantity,
            sell_external_id: Some(format!("sell-{tx}")),
        }
    }

    fn required(tx: u128, quantity: Decimal) -> RequiredOpeningBuy {
        RequiredOpeningBuy {
            asset_id: ASSET,
            sell_transaction_id: id(tx),
            external_id: synthetic_external_id(&format!("sell-{tx}")),
            quantity,
        }
    }

    fn existing(n: u128, sell_tx: u128) -> ExistingOpeningBuy {
        ExistingOpeningBuy {
            transaction_id: id(n),
            external_id: synthetic_external_id(&format!("sell-{sell_tx}")),
        }
    }

    #[test]
    fn sell_without_prior_position_needs_full_buy() {
        let events = vec![sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-1.5))];

        assert_eq!(required_opening_buys(&events), vec![required(1, dec!(1.5))]);
    }

    #[test]
    fn partial_position_needs_only_the_shortfall() {
        let events = vec![
            buy(1, datetime!(2024-08-25 0:00 UTC), dec!(1)),
            sell(2, datetime!(2024-09-01 0:00 UTC), dec!(-1.5)),
        ];

        assert_eq!(required_opening_buys(&events), vec![required(2, dec!(0.5))]);
    }

    #[test]
    fn covered_sell_needs_nothing() {
        let events = vec![
            buy(1, datetime!(2024-08-25 0:00 UTC), dec!(2)),
            sell(2, datetime!(2024-09-01 0:00 UTC), dec!(-2)),
        ];

        assert!(required_opening_buys(&events).is_empty());
    }

    #[test]
    fn multiple_short_sells_each_get_their_own_buy() {
        let events = vec![
            sell(2, datetime!(2024-10-01 0:00 UTC), dec!(-3)),
            sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-2)),
        ];

        assert_eq!(
            required_opening_buys(&events),
            vec![required(1, dec!(2)), required(2, dec!(3))]
        );
    }

    #[test]
    fn multiple_short_sells_are_priced_at_their_own_sell() {
        let account = id(99);
        let date = datetime!(2024-08-22 0:00 UTC);
        let first = required(1, dec!(2))
            .to_transaction_dto(dec!(-2), &EntryDto::new(1, account, dec!(200)), date)
            .unwrap();
        let second = required(2, dec!(1.5))
            .to_transaction_dto(dec!(-3), &EntryDto::new(1, account, dec!(330)), date)
            .unwrap();

        let amounts: Vec<(Decimal, Decimal)> = [first, second]
            .iter()
            .map(|dto| match &dto.transaction_type {
                TransactionTypeDto::AssetPurchase(p) => (p.purchase.quantity, p.sale.quantity),
                _ => panic!("expected an asset purchase"),
            })
            .collect();
        assert_eq!(
            amounts,
            vec![(dec!(2), dec!(-200)), (dec!(1.5), dec!(-165))]
        );
    }

    #[test]
    fn opening_buy_dto_is_flagged_and_uses_the_sell_currency_and_account() {
        let account = id(99);
        let date = datetime!(2024-08-22 0:00 UTC);
        let dto = required(1, dec!(0.5))
            .to_transaction_dto(dec!(-1.5), &EntryDto::new(7, account, dec!(150)), date)
            .unwrap();

        assert_eq!(dto.visibility, TransactionVisibilityDto::Action);
        assert_eq!(dto.date, date);
        assert!(dto.fee_entries.is_empty());
        let TransactionTypeDto::AssetPurchase(purchase) = dto.transaction_type else {
            panic!("expected an asset purchase");
        };
        assert_eq!(purchase.purchase.asset_id, ASSET);
        assert_eq!(purchase.purchase.account_id, account);
        assert_eq!(purchase.sale.asset_id, 7);
        assert_eq!(purchase.sale.quantity, dec!(-50));
    }

    #[test]
    fn zero_quantity_sell_cannot_be_priced() {
        let dto = required(1, dec!(1)).to_transaction_dto(
            dec!(0),
            &EntryDto::new(7, id(99), dec!(10)),
            datetime!(2024-08-22 0:00 UTC),
        );

        assert!(dto.is_none());
    }

    #[test]
    fn same_day_buy_is_counted_before_the_sell() {
        let day = datetime!(2024-09-01 0:00 UTC);
        let events = vec![sell(1, day, dec!(-2)), buy(2, day, dec!(2))];

        assert!(required_opening_buys(&events).is_empty());
    }

    #[test]
    fn earlier_timestamp_wins_over_buy_first_on_the_same_day() {
        let events = vec![
            sell(1, datetime!(2024-09-01 10:00 UTC), dec!(-2)),
            buy(2, datetime!(2024-09-01 15:00 UTC), dec!(2)),
        ];

        assert_eq!(required_opening_buys(&events), vec![required(1, dec!(2))]);
    }

    #[test]
    fn shortfall_at_epsilon_is_ignored() {
        let events = vec![
            buy(1, datetime!(2024-08-25 0:00 UTC), dec!(1)),
            sell(2, datetime!(2024-09-01 0:00 UTC), dec!(-1.000001)),
        ];

        assert!(required_opening_buys(&events).is_empty());
    }

    #[test]
    fn shortfall_above_epsilon_is_bought() {
        let events = vec![
            buy(1, datetime!(2024-08-25 0:00 UTC), dec!(1)),
            sell(2, datetime!(2024-09-01 0:00 UTC), dec!(-1.0000011)),
        ];

        assert_eq!(
            required_opening_buys(&events),
            vec![required(2, dec!(0.0000011))]
        );
    }

    #[test]
    fn position_resets_after_a_shortfall() {
        let events = vec![
            sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-2)),
            buy(2, datetime!(2024-09-10 0:00 UTC), dec!(1)),
            sell(3, datetime!(2024-09-20 0:00 UTC), dec!(-1)),
        ];

        assert_eq!(required_opening_buys(&events), vec![required(1, dec!(2))]);
    }

    #[test]
    fn non_binding_outflow_resets_without_a_buy() {
        let mut manual_sell = sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-2));
        manual_sell.sell_external_id = None;
        let events = vec![
            manual_sell,
            sell(2, datetime!(2024-09-10 0:00 UTC), dec!(-1)),
        ];

        assert_eq!(required_opening_buys(&events), vec![required(2, dec!(1))]);
    }

    #[test]
    fn assets_are_replayed_independently() {
        let mut other = sell(2, datetime!(2024-09-01 0:00 UTC), dec!(-4));
        other.asset_id = OTHER_ASSET;
        let events = vec![buy(1, datetime!(2024-08-25 0:00 UTC), dec!(10)), other];

        let result = required_opening_buys(&events);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].asset_id, OTHER_ASSET);
        assert_eq!(result[0].quantity, dec!(4));
    }

    #[test]
    fn entries_of_one_transaction_are_netted() {
        let date = datetime!(2024-09-01 0:00 UTC);
        let events = vec![sell(1, date, dec!(-2)), sell(1, date, dec!(-0.5))];

        assert_eq!(required_opening_buys(&events), vec![required(1, dec!(2.5))]);
    }

    #[test]
    fn first_sync_creates_every_needed_buy() {
        let plan = plan_opening_buys(
            vec![required(1, dec!(2)), required(2, dec!(3))],
            vec![],
            &HashSet::new(),
        );

        assert_eq!(
            plan.create,
            vec![required(1, dec!(2)), required(2, dec!(3))]
        );
        assert!(plan.keep.is_empty() && plan.delete.is_empty() && plan.owned_by_user.is_empty());
    }

    #[test]
    fn reprojection_keeps_existing_buys() {
        let plan = plan_opening_buys(
            vec![required(1, dec!(2))],
            vec![existing(10, 1)],
            &HashSet::new(),
        );

        assert!(plan.create.is_empty() && plan.delete.is_empty());
        assert_eq!(plan.keep, vec![(existing(10, 1), required(1, dec!(2)))]);
    }

    #[test]
    fn later_real_buy_shrinks_the_synthetic_buy() {
        let events = vec![
            buy(5, datetime!(2024-08-01 0:00 UTC), dec!(1)),
            sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-1.5)),
        ];

        let plan = plan_opening_buys(
            required_opening_buys(&events),
            vec![existing(10, 1)],
            &HashSet::new(),
        );

        assert_eq!(plan.keep, vec![(existing(10, 1), required(1, dec!(0.5)))]);
        assert!(plan.create.is_empty() && plan.delete.is_empty());
    }

    #[test]
    fn later_real_buy_removes_the_synthetic_buy() {
        let events = vec![
            buy(5, datetime!(2024-08-01 0:00 UTC), dec!(2)),
            sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-1.5)),
        ];

        let plan = plan_opening_buys(
            required_opening_buys(&events),
            vec![existing(10, 1)],
            &HashSet::new(),
        );

        assert!(plan.create.is_empty() && plan.keep.is_empty());
        assert_eq!(plan.delete, vec![existing(10, 1)]);
    }

    #[test]
    fn edited_synthetic_buy_counts_as_a_normal_entry_and_is_left_alone() {
        let mut edited = buy(10, datetime!(2024-08-22 0:00 UTC), dec!(1.5));
        edited.sell_external_id = None;
        let events = vec![edited, sell(1, datetime!(2024-09-01 0:00 UTC), dec!(-1.5))];

        let plan = plan_opening_buys(required_opening_buys(&events), vec![], &HashSet::new());

        assert_eq!(plan, OpeningBuyPlan::default());
    }

    #[test]
    fn user_owned_external_id_is_never_recreated() {
        let taken: HashSet<String> = [synthetic_external_id("sell-1")].into_iter().collect();

        let plan = plan_opening_buys(vec![required(1, dec!(1.5))], vec![], &taken);

        assert!(plan.create.is_empty());
        assert_eq!(plan.owned_by_user, vec![required(1, dec!(1.5))]);
    }

    #[test]
    fn changed_date_or_amount_is_detected() {
        let account = id(99);
        let date = datetime!(2024-08-22 0:00 UTC);
        let proceeds = EntryDto::new(7, account, dec!(150));
        let current = required(1, dec!(1.5))
            .to_transaction_dto(dec!(-1.5), &proceeds, date)
            .unwrap();

        let same = required(1, dec!(1.5))
            .to_transaction_dto(dec!(-1.5), &proceeds, date)
            .unwrap();
        let smaller = required(1, dec!(0.5))
            .to_transaction_dto(dec!(-1.5), &proceeds, date)
            .unwrap();
        let earlier = required(1, dec!(1.5))
            .to_transaction_dto(dec!(-1.5), &proceeds, datetime!(2024-08-01 0:00 UTC))
            .unwrap();

        assert!(!opening_buy_changed(&current, &same));
        assert!(opening_buy_changed(&current, &smaller));
        assert!(opening_buy_changed(&current, &earlier));
    }
}
