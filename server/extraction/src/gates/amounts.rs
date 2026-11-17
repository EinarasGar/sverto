use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use rust_decimal::Decimal;

const DIGIT_ZEROS: [u32; 37] = [
    0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66,
    0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90,
    0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x1D7CE,
];
const DECIMAL_SEPARATORS: [char; 2] = ['\u{066B}', '\u{FF0E}'];
const THOUSANDS_SEPARATORS: [char; 2] = ['\u{066C}', '\u{FF0C}'];
const TWO_DECIMAL_CURRENCIES: [&str; 137] = [
    "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
    "BGN", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD", "CDF", "CHF",
    "CNY", "COP", "CRC", "CUP", "CVE", "CZK", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR",
    "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG",
    "HUF", "IDR", "ILS", "INR", "IRR", "JMD", "KES", "KGS", "KHR", "KPW", "KYD", "KZT", "LAK",
    "LBP", "LKR", "LRD", "LSL", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP", "MRU", "MUR",
    "MVR", "MWK", "MXN", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD", "PAB", "PEN",
    "PGK", "PHP", "PKR", "PLN", "QAR", "RON", "RSD", "RUB", "SAR", "SBD", "SCR", "SDG", "SEK",
    "SGD", "SHP", "SLE", "SOS", "SRD", "SSP", "STN", "SVC", "SYP", "SZL", "THB", "TJS", "TMT",
    "TOP", "TRY", "TTD", "TWD", "TZS", "UAH", "USD", "UYU", "UZS", "VED", "VES", "WST", "XCD",
    "YER", "ZAR", "ZMW", "ZWG", "ZWL", "SLL", "CUC",
];
const THREE_DECIMAL_CURRENCIES: [&str; 7] = ["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];

static SPACE_GROUPED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[.,]?\d{1,3}([ \u{a0}\u{202f}\u{2009}'’]\d{3})+([.,]\d+)?$").expect("valid regex")
});
static AMOUNT_SHAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\d[.,]\d{1,8}\b|\d{1,3}(?:[.,'’\u{a0}\u{202f}\u{2009}]\d{3})+\b|\p{Sc}\s?\d|\d\s?[\p{Sc}円元]",
    )
    .expect("valid regex")
});

pub fn normalize_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            c if c.is_ascii() => c,
            c if DECIMAL_SEPARATORS.contains(&c) => '.',
            c if THOUSANDS_SEPARATORS.contains(&c) => ',',
            c => DIGIT_ZEROS
                .iter()
                .find_map(|zero| {
                    let offset = (c as u32).checked_sub(*zero)?;
                    (offset < 10).then(|| char::from(b'0' + offset as u8))
                })
                .unwrap_or(c),
        })
        .collect()
}

static PERCENTAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9][0-9.,]*\s?[%‰]").expect("valid regex"));

pub fn contains_amount_shape(text: &str) -> bool {
    AMOUNT_SHAPE.is_match(&PERCENTAGE.replace_all(text, " "))
}

static EXPONENT_NOTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([0-9]+(?:[.,][0-9]+)?)[eE]([+-]?[0-9]{1,2})$").expect("valid regex")
});

pub fn amount_matches(
    amount_text: &str,
    quantity: Decimal,
    decimal_separator: Option<char>,
) -> bool {
    magnitudes(&normalize_digits(amount_text), decimal_separator).contains(&quantity.abs())
}

pub fn decimal_separator(amount_text: &str) -> Option<char> {
    let text = normalize_digits(amount_text);
    let core = separated_digits(digit_span(&text)?)?;
    let other = |separator| if separator == '.' { ',' } else { '.' };
    match (core.matches('.').count(), core.matches(',').count()) {
        (0, 0) => None,
        (dots, commas) if dots > 0 && commas > 0 => {
            let decimal = if core.rfind('.') > core.rfind(',') {
                '.'
            } else {
                ','
            };
            magnitudes(&text, None).first().map(|_| decimal)
        }
        (dots, commas) => {
            let separator = if dots > 0 { '.' } else { ',' };
            match (dots + commas, groups_thousands(&core, separator)) {
                (1, false) => Some(separator),
                (2.., true) => Some(other(separator)),
                _ => None,
            }
        }
    }
}

pub fn reads_in_minor_units(amount_text: &str, quantity: Decimal, asset_ref: &str) -> bool {
    let code = asset_ref.trim().to_ascii_uppercase();
    let exponent = if TWO_DECIMAL_CURRENCIES.contains(&code.as_str()) {
        2
    } else if THREE_DECIMAL_CURRENCIES.contains(&code.as_str()) {
        3
    } else {
        return false;
    };
    let text = normalize_digits(amount_text);
    digit_span(&text).is_some_and(|span| span.chars().all(|c| c.is_ascii_digit()))
        && magnitudes(&text, None).first().is_some_and(|whole| {
            let mut shifted = *whole;
            shifted.set_scale(exponent).is_ok() && shifted == quantity.abs()
        })
}

pub fn appears_as_token(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        !joined_on_left(&haystack[..start], needle)
            && !joined_on_right(&haystack[start + needle.len()..], needle)
    })
}

pub fn appears_as_compact_date(haystack: &str, needle: &str) -> bool {
    haystack
        .match_indices(needle)
        .any(|(start, _)| !joined_on_left(&haystack[..start], needle))
}

fn joined_on_left(before: &str, needle: &str) -> bool {
    if !needle.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let mut chars = before.chars().rev();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some(c) if is_separator(c) => {
            let leading = needle.chars().take_while(char::is_ascii_digit).count();
            let group_head = chars.take_while(char::is_ascii_digit).count();
            let decimals = is_group_space(c)
                && before[..before.len() - c.len_utf8() - group_head]
                    .strip_suffix(['.', ','])
                    .is_some_and(|whole| whole.ends_with(|d: char| d.is_ascii_digit()));
            leading == 3 && (1..=3).contains(&group_head) && !decimals
        }
        _ => false,
    }
}

fn joined_on_right(after: &str, needle: &str) -> bool {
    if !needle.ends_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let mut chars = after.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some(c) if is_separator(c) => {
            let following = chars.take_while(char::is_ascii_digit).count();
            let needle_has_decimal = needle.contains(['.', ',']);
            let trailing = needle
                .chars()
                .rev()
                .take_while(char::is_ascii_digit)
                .count();
            match (following, needle_has_decimal) {
                (0, _) => false,
                (_, false) => matches!(c, '.' | ',') || following == 3,
                (following, true) => following == 3 && trailing == 3,
            }
        }
        _ => false,
    }
}

pub fn numeric_value(text: &str) -> Option<Decimal> {
    let text = normalize_digits(text.trim());
    if text.is_empty()
        || !text.chars().all(|c| {
            c.is_ascii_digit()
                || is_separator(c)
                || matches!(c, '-' | '−' | '+' | '(' | ')')
                || c.is_ascii_uppercase()
                || !c.is_alphanumeric()
        })
    {
        return None;
    }
    let [magnitude] = magnitudes(&text, None)[..] else {
        return None;
    };
    let negative = text.starts_with(['-', '−'])
        || text.ends_with('-')
        || (text.starts_with('(') && text.ends_with(')'));
    Some(if negative { -magnitude } else { magnitude })
}

fn is_separator(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | '\'' | '’' | ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}'
    )
}

fn is_group_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '\u{2009}' | '\'' | '’')
}

fn digit_span(text: &str) -> Option<&str> {
    let first = text.find(|c: char| c.is_ascii_digit())?;
    let last = text.rfind(|c: char| c.is_ascii_digit())?;
    let first = if text[..first].ends_with(['.', ',']) {
        first - 1
    } else {
        first
    };
    Some(&text[first..=last])
}

fn separated_digits(span: &str) -> Option<String> {
    if span.contains(is_group_space) && !SPACE_GROUPED.is_match(span) {
        return None;
    }
    let core: String = span.chars().filter(|c| !is_group_space(*c)).collect();
    core.chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
        .then_some(core)
}

fn groups_thousands(integer: &str, separator: char) -> bool {
    let mut groups = integer.split(separator);
    let head = groups.next().unwrap_or_default();
    let rest: Vec<&str> = groups.collect();
    let Some((last, middle)) = rest.split_last() else {
        return false;
    };
    (1..=3).contains(&head.len())
        && !head.starts_with('0')
        && last.len() == 3
        && (middle.iter().all(|group| group.len() == 3)
            || (head.len() <= 2 && middle.iter().all(|group| group.len() == 2)))
}

fn magnitudes(text: &str, decimal_separator: Option<char>) -> Vec<Decimal> {
    let Some(span) = digit_span(text) else {
        return Vec::new();
    };
    if let Some(parts) = EXPONENT_NOTATION.captures(span) {
        let mantissa = parts[1].replace(',', ".");
        return Decimal::from_scientific(&format!("{mantissa}e{}", &parts[2]))
            .into_iter()
            .collect();
    }
    let Some(core) = separated_digits(span) else {
        return Vec::new();
    };

    let dots = core.matches('.').count();
    let commas = core.matches(',').count();
    let readings = match (dots, commas) {
        (0, 0) => vec![core],
        (_, 0) | (0, _) => {
            let separator = if dots > 0 { '.' } else { ',' };
            let groups = groups_thousands(&core, separator);
            let decimal = dots + commas == 1
                && (!groups || decimal_separator.is_none_or(|decimal| decimal == separator));
            let grouped = groups
                && (dots + commas > 1
                    || decimal_separator.is_none_or(|decimal| decimal != separator));
            decimal
                .then(|| core.replace(',', "."))
                .into_iter()
                .chain(grouped.then(|| core.replace(separator, "")))
                .collect()
        }
        _ => {
            let (decimal, group) = if core.rfind('.') > core.rfind(',') {
                ('.', ',')
            } else {
                (',', '.')
            };
            match core.split_once(decimal) {
                Some((integer, fraction))
                    if !fraction.contains(decimal) && groups_thousands(integer, group) =>
                {
                    vec![core.replace(group, "").replace(decimal, ".")]
                }
                _ => Vec::new(),
            }
        }
    };
    readings
        .iter()
        .filter_map(|reading| Decimal::from_str(&format!("0{reading}")).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn amounts_match_in_any_common_notation() {
        for (text, quantity) in [
            ("-12.34", dec!(-12.34)),
            ("12.34", dec!(-12.34)),
            ("£3,418.27", dec!(3418.27)),
            ("−1.00", dec!(-1)),
            ("+248.36", dec!(248.36)),
            ("1 284.67", dec!(1284.67)),
            ("1\u{a0}284,67", dec!(1284.67)),
            ("1\u{202f}234\u{202f}567,89 €", dec!(1234567.89)),
            ("1.234,56 EUR", dec!(-1234.56)),
            ("(12.34)", dec!(-12.34)),
            ("12.34-", dec!(-12.34)),
            ("12.34 DR", dec!(-12.34)),
            ("1'234.50", dec!(1234.5)),
            ("CHF 1’234.50", dec!(1234.5)),
            ("3,50", dec!(-3.5)),
            ("1,234", dec!(1234)),
            ("1,234", dec!(1.234)),
            ("1.234.567", dec!(1234567)),
            ("12,34,567.89", dec!(1234567.89)),
            ("¥1,234", dec!(-1234)),
            ("12.345 KWD", dec!(12.345)),
            ("0.00012345 BTC", dec!(0.00012345)),
            ("1500", dec!(1500.00)),
            ("-12.3", dec!(-12.30)),
            (".75", dec!(-0.75)),
            ("-,50", dec!(-0.5)),
            ("١٬٢٣٤٫٥٦", dec!(1234.56)),
            ("१२३.४५", dec!(123.45)),
            ("１２３．４５", dec!(123.45)),
        ] {
            assert!(
                amount_matches(text, quantity, None),
                "{text} should read as {quantity}"
            );
        }
    }

    #[test]
    fn amounts_that_differ_do_not_match() {
        for (text, quantity) in [
            ("12.34", dec!(12.43)),
            ("1.00", dec!(11)),
            ("EUR", dec!(1)),
            ("12a34", dec!(1234)),
            ("1,234", dec!(123.4)),
            ("01       47.83", dec!(147.83)),
            ("16 3.95", dec!(163.95)),
            ("12  345.00", dec!(12345)),
        ] {
            assert!(
                !amount_matches(text, quantity, None),
                "{text} should not read as {quantity}"
            );
        }
    }

    #[test]
    fn a_separator_groups_thousands_only_before_groups_of_three_digits() {
        for (text, quantity) in [
            ("12,50", dec!(1250)),
            ("-3,50", dec!(350)),
            ("1000,00", dec!(100000)),
            ("0,500", dec!(500)),
            ("1234.567", dec!(1234567)),
            ("1.2.3", dec!(123)),
            ("1,23.45", dec!(123.45)),
            ("12,3456.78", dec!(123456.78)),
        ] {
            assert!(
                !amount_matches(text, quantity, None),
                "{text} should not read as {quantity}"
            );
        }
    }

    #[test]
    fn a_token_that_reads_both_ways_takes_the_decimal_separator_of_the_file() {
        for (text, separator) in [
            ("12,50", Some(',')),
            ("1.234,56 EUR", Some(',')),
            ("1,234,567", Some('.')),
            ("£3,418.27", Some('.')),
            ("0.123", Some('.')),
            (".75", Some('.')),
            ("1,234", None),
            ("12.345 KWD", None),
            ("1500", None),
        ] {
            assert_eq!(decimal_separator(text), separator, "{text}");
        }

        assert!(amount_matches("1,234", dec!(1234), Some('.')));
        assert!(!amount_matches("1,234", dec!(1.234), Some('.')));
        assert!(amount_matches("1,234", dec!(1.234), Some(',')));
        assert!(!amount_matches("1,234", dec!(1234), Some(',')));
        assert!(amount_matches("1.234", dec!(1234), Some(',')));
        assert!(amount_matches("1,234,567", dec!(1234567), Some(',')));
    }

    #[test]
    fn exponent_notation_is_read() {
        for (text, quantity) in [
            ("5.0E-5", dec!(0.00005)),
            ("1e-8", dec!(-0.00000001)),
            ("1,5E3", dec!(1500)),
            ("2.5e+2 USD", dec!(250)),
        ] {
            assert!(
                amount_matches(text, quantity, None),
                "{text} should read as {quantity}"
            );
        }
        assert_eq!(numeric_value("-5.0E-5"), Some(dec!(-0.00005)));
        assert!(!amount_matches("5.0E-5", dec!(5.05), None));
    }

    #[test]
    fn a_number_inside_a_longer_number_is_not_a_token() {
        for (haystack, needle) in [
            ("-11.00", "1.00"),
            ("3,418.27", "418.27"),
            ("1 284.67", "284.67"),
            ("12.50", "12"),
            ("1,234,567.00", "1,234"),
            ("1 234.56", "1"),
            ("12345", "234"),
        ] {
            assert!(
                !appears_as_token(haystack, needle),
                "{needle} in {haystack}"
            );
        }
    }

    #[test]
    fn neighbouring_numbers_do_not_hide_a_token() {
        for (haystack, needle) in [
            ("x,-10.00,80.00", "10.00"),
            ("2024-01-02,10.00,80.00", "80.00"),
            ("10.00,80.00", "10.00"),
            ("2023 284.67", "284.67"),
            ("£47.18  £3,418.27", "£47.18"),
            ("Total: 1,234.56.", "1,234.56"),
            ("02  284.67", "284.67"),
        ] {
            assert!(appears_as_token(haystack, needle), "{needle} in {haystack}");
        }
    }

    #[test]
    fn a_number_after_another_numbers_decimals_is_its_own_token() {
        for (haystack, needle) in [
            ("-0.07 254.93 0.01", "254.93"),
            ("147.02 254.9286 Market", "254.9286"),
            ("0,07 254,93", "254,93"),
            ("EUR 1.83 -0.48 125.00", "125.00"),
        ] {
            assert!(appears_as_token(haystack, needle), "{needle} in {haystack}");
        }
        for (haystack, needle) in [
            ("12 345 678,90", "678,90"),
            ("7 254.93", "254.93"),
            ("1.234,567", "567"),
        ] {
            assert!(
                !appears_as_token(haystack, needle),
                "{needle} in {haystack}"
            );
        }
    }

    #[test]
    fn numeric_values_are_read_only_from_numbers() {
        assert_eq!(numeric_value("-31.62"), Some(dec!(-31.62)));
        assert_eq!(numeric_value("(12.50)"), Some(dec!(-12.50)));
        assert_eq!(numeric_value("1.234,56"), Some(dec!(1234.56)));
        assert_eq!(numeric_value("£3,418.27"), Some(dec!(3418.27)));
        assert_eq!(numeric_value("742.18"), Some(dec!(742.18)));
        assert_eq!(numeric_value("1,234"), None);
        assert_eq!(numeric_value("2024-01-02"), None);
        assert_eq!(numeric_value("Coffee 3"), None);
        assert_eq!(numeric_value(""), None);
    }

    #[test]
    fn amount_shapes_cover_currencies_of_any_precision() {
        for text in [
            "Coffee 3.50",
            "¥1,234",
            "500円",
            "€ 7",
            "KWD 12.345",
            "BTC 0.00012345",
            "1.234.567",
        ] {
            assert!(contains_amount_shape(text), "{text}");
        }
        for text in ["Page 3 of 34", "Account 12345678", "Card ****1234"] {
            assert!(!contains_amount_shape(text), "{text}");
        }
    }
}
