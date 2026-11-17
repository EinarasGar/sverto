use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairKind {
    BankBank,
    BankUser,
    UserUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountLevel {
    Exact,
    Close,
    Rounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateLevel {
    BeforePurchase,
    Same,
    OneDay,
    TwoDays,
    ThreeDays,
    Week,
    Long,
}

impl DateLevel {
    pub fn of_gap(days: i64) -> Self {
        match days {
            0 => Self::Same,
            1 => Self::OneDay,
            2 => Self::TwoDays,
            3 => Self::ThreeDays,
            4..=7 => Self::Week,
            _ => Self::Long,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordingLevel {
    Strong,
    Good,
    Weak,
    Different,
    Unknown,
}

impl WordingLevel {
    pub fn of(similarity: Option<f64>) -> Self {
        match similarity {
            None => Self::Unknown,
            Some(s) if s >= 90.0 => Self::Strong,
            Some(s) if s >= 70.0 => Self::Good,
            Some(s) if s >= 50.0 => Self::Weak,
            Some(_) => Self::Different,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountLevel {
    Same,
    Unknown,
    Mismatch,
}

#[derive(Debug, Clone, Copy)]
pub struct Features {
    pub amount: AmountLevel,
    pub date: DateLevel,
    pub wording: WordingLevel,
    pub account: AccountLevel,
}

pub struct LinkPolicy {
    pub link: Option<f64>,
    pub ask: f64,
}

impl PairKind {
    pub fn policy(self) -> LinkPolicy {
        match self {
            Self::BankBank => LinkPolicy {
                link: Some(0.95),
                ask: 0.30,
            },
            Self::BankUser => LinkPolicy {
                link: Some(0.90),
                ask: 0.25,
            },
            Self::UserUser => LinkPolicy {
                link: None,
                ask: 0.60,
            },
        }
    }

    pub fn may_link(self, features: Features) -> bool {
        match self {
            Self::BankBank => features.amount == AmountLevel::Exact,
            Self::BankUser => {
                features.amount == AmountLevel::Exact && features.account != AccountLevel::Mismatch
            }
            Self::UserUser => false,
        }
    }

    pub fn score(self, features: Features) -> f64 {
        match self {
            Self::BankBank => bank_bank(features),
            Self::BankUser => bank_user(features),
            Self::UserUser => user_user(features),
        }
    }
}

pub fn probability(score: f64) -> f64 {
    1.0 / (1.0 + 2f64.powf(-score))
}

fn bank_bank(f: Features) -> f64 {
    let amount = match f.amount {
        AmountLevel::Exact => 6.0,
        AmountLevel::Close | AmountLevel::Rounded => 0.5,
    };
    let date = match f.date {
        DateLevel::Same => 2.0,
        DateLevel::OneDay => 1.5,
        DateLevel::TwoDays => 0.5,
        DateLevel::ThreeDays => -1.0,
        DateLevel::Week => -4.0,
        DateLevel::Long | DateLevel::BeforePurchase => -7.0,
    };
    let wording = match f.wording {
        WordingLevel::Strong => 2.0,
        WordingLevel::Good => 1.0,
        _ => 0.0,
    };
    let account = match f.account {
        AccountLevel::Unknown => -1.0,
        _ => 0.0,
    };
    amount + date + wording + account - 3.0
}

fn lag_weight(f: DateLevel) -> f64 {
    let (matched, unmatched) = match f {
        DateLevel::Same => (0.040, 1.0 / 33.0),
        DateLevel::OneDay => (0.678, 2.0 / 33.0),
        DateLevel::TwoDays => (0.201, 2.0 / 33.0),
        DateLevel::ThreeDays => (0.053, 2.0 / 33.0),
        DateLevel::Week => (0.020, 8.0 / 33.0),
        DateLevel::Long => (0.008, 18.0 / 33.0),
        DateLevel::BeforePurchase => return -3.0,
    };
    f64::log2(matched / unmatched)
}

fn bank_user(f: Features) -> f64 {
    let amount = match f.amount {
        AmountLevel::Exact => 3.0,
        AmountLevel::Rounded => 0.5,
        AmountLevel::Close => -1.5,
    };
    let account = match f.account {
        AccountLevel::Same => 0.5,
        AccountLevel::Unknown => 0.0,
        AccountLevel::Mismatch => -4.0,
    };
    amount + lag_weight(f.date) + account
}

fn user_user(f: Features) -> f64 {
    let amount = match f.amount {
        AmountLevel::Exact => 3.0,
        AmountLevel::Rounded => 1.0,
        AmountLevel::Close => 0.0,
    };
    let date = match f.date {
        DateLevel::Same => 2.0,
        DateLevel::OneDay => 1.0,
        DateLevel::TwoDays => 0.0,
        DateLevel::ThreeDays => -1.0,
        DateLevel::Week => -3.0,
        DateLevel::Long | DateLevel::BeforePurchase => -6.0,
    };
    let wording = match f.wording {
        WordingLevel::Strong => 2.0,
        WordingLevel::Good => 1.0,
        WordingLevel::Different => -1.0,
        _ => 0.0,
    };
    let account = match f.account {
        AccountLevel::Same => 0.5,
        AccountLevel::Unknown => 0.0,
        AccountLevel::Mismatch => -3.0,
    };
    f64::log2(0.3 / 0.7) + amount + date + wording + account
}

pub fn amount_level(
    kind: PairKind,
    incoming: Decimal,
    existing: Decimal,
    incoming_typed: bool,
    existing_typed: bool,
) -> Option<AmountLevel> {
    if (incoming > Decimal::ZERO) != (existing > Decimal::ZERO) {
        return None;
    }
    if incoming == existing {
        return Some(AmountLevel::Exact);
    }
    match kind {
        PairKind::BankBank => {
            let difference = (incoming - existing).abs();
            let largest = incoming.abs().max(existing.abs());
            (difference / largest <= Decimal::new(5, 3)).then_some(AmountLevel::Close)
        }
        _ => ((existing_typed && is_rounding_of(existing, incoming))
            || (incoming_typed && is_rounding_of(incoming, existing)))
        .then_some(AmountLevel::Rounded),
    }
}

fn is_rounding_of(typed: Decimal, exact: Decimal) -> bool {
    let size = exact.abs();
    let moved = (typed - exact).abs();
    if size < Decimal::ONE {
        return false;
    }
    if typed.fract().is_zero() && moved < Decimal::ONE {
        return true;
    }
    let mut leading = Decimal::ONE;
    while leading
        .checked_mul(Decimal::TEN)
        .is_some_and(|next| next <= size)
    {
        leading *= Decimal::TEN;
    }
    leading >= Decimal::TEN
        && [leading / Decimal::TWO, leading]
            .into_iter()
            .any(|step| (typed % step).is_zero() && moved * Decimal::TWO <= step)
}

pub fn is_transposition_of(typed: Decimal, exact: Decimal) -> bool {
    let Some((typed, exact)) = digits_of(typed, exact) else {
        return false;
    };
    if typed.len() != exact.len() {
        return false;
    }
    let differing: Vec<usize> = (0..typed.len())
        .filter(|&place| typed[place] != exact[place])
        .collect();
    matches!(
        differing[..],
        [a, b] if b == a + 1 && typed[a] == exact[b] && typed[b] == exact[a]
    )
}

pub fn is_digit_dropped_or_added(typed: Decimal, exact: Decimal) -> bool {
    let Some((typed, exact)) = digits_of(typed, exact) else {
        return false;
    };
    let (longer, shorter) = if typed.len() > exact.len() {
        (typed, exact)
    } else {
        (exact, typed)
    };
    longer.len() == shorter.len() + 1
        && (0..longer.len()).any(|left_out| {
            longer
                .iter()
                .enumerate()
                .filter(|(place, _)| *place != left_out)
                .map(|(_, digit)| digit)
                .eq(shorter.iter())
        })
}

fn digits_of(typed: Decimal, exact: Decimal) -> Option<(Vec<u8>, Vec<u8>)> {
    if typed.is_sign_negative() != exact.is_sign_negative() {
        return None;
    }
    let scale = typed.scale().max(exact.scale());
    let digits = |mut amount: Decimal| {
        amount.rescale(scale);
        amount.mantissa().unsigned_abs().to_string().into_bytes()
    };
    Some((digits(typed), digits(exact)))
}
