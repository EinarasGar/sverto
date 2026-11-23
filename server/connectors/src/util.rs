use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

use crate::Result;

pub(crate) fn parse_decimal(value: &serde_json::Value) -> Option<Decimal> {
    value
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| value.as_f64().and_then(Decimal::from_f64))
}

pub(crate) async fn ensure_success(resp: reqwest::Response) -> Result<reqwest::Response> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("<body read failed: {e}>"));
        anyhow::bail!("provider returned {}: {}", status, body_text);
    }
    Ok(resp)
}

pub(crate) fn normalized_iban(raw: &str) -> Option<String> {
    let iban = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let bytes = iban.as_bytes();
    let shaped = (15..=34).contains(&bytes.len())
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..4].iter().all(u8::is_ascii_digit)
        && bytes
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    shaped.then_some(iban)
}
