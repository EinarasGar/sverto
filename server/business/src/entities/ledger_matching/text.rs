use std::collections::BTreeSet;

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

pub fn normalise(text: &str) -> String {
    let letters: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphabetic() { c } else { ' ' })
        .collect();
    letters
        .split_whitespace()
        .filter(|token| token.chars().count() > 1)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn similarity(a: &str, b: &str) -> Option<f64> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let partial = if a.chars().count().min(b.chars().count()) >= 4 {
        partial_ratio(a, b)
    } else {
        0.0
    };
    Some(token_set_ratio(a, b).max(partial))
}

fn ratio(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut previous = vec![0usize; b.len() + 1];
    for x in &a {
        let mut current = vec![0usize; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            current[j + 1] = if x == y {
                previous[j] + 1
            } else {
                previous[j + 1].max(current[j])
            };
        }
        previous = current;
    }
    200.0 * previous[b.len()] as f64 / (a.len() + b.len()) as f64
}

fn token_set_ratio(a: &str, b: &str) -> f64 {
    let a: BTreeSet<&str> = a.split(' ').collect();
    let b: BTreeSet<&str> = b.split(' ').collect();
    let join = |tokens: Vec<&str>| tokens.join(" ");
    let common = join(a.intersection(&b).copied().collect());
    let only_a = join(a.difference(&b).copied().collect());
    let only_b = join(b.difference(&a).copied().collect());
    if !common.is_empty() && (only_a.is_empty() || only_b.is_empty()) {
        return 100.0;
    }
    let with = |rest: &str| {
        if common.is_empty() {
            rest.to_string()
        } else {
            format!("{common} {rest}")
        }
    };
    let (full_a, full_b) = (with(&only_a), with(&only_b));
    ratio(&full_a, &full_b)
        .max(ratio(&common, &full_a))
        .max(ratio(&common, &full_b))
}

fn partial_ratio(a: &str, b: &str) -> f64 {
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    if long.contains(short) {
        return 100.0;
    }
    let long: Vec<char> = long.chars().collect();
    let width = short.chars().count();
    long.windows(width)
        .map(|window| ratio(short, &window.iter().collect::<String>()))
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalising_drops_accents_digits_and_single_letters() {
        assert_eq!(normalise("Šiaulių TESCO 3021, A"), "siauliu tesco");
    }

    #[test]
    fn extra_words_on_one_side_still_match() {
        assert_eq!(similarity("tesco stores", "tesco"), Some(100.0));
        assert!(similarity("tesco stores london", "tesco stores").unwrap() >= 90.0);
    }

    #[test]
    fn unrelated_wording_scores_low_and_missing_wording_has_no_score() {
        assert!(similarity("shell garage", "tesco stores").unwrap() < 50.0);
        assert_eq!(similarity("", "tesco"), None);
    }
}
