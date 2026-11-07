use std::collections::HashMap;

use rust_decimal::Decimal;
use serde_json::Value;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

use crate::ibkr::statement::SECTION_KEY;
use crate::models::{
    MappedTransaction, ProviderTransaction, ProviderTransactionKind, SkippedTransaction,
};

pub(crate) const FLEX_DATE: &[BorrowedFormatItem<'static>] =
    format_description!("[year][month][day]");

const SUPPORTED_ASSET_CATEGORIES: [&str; 2] = ["STK", "FUND"];
const DIVIDEND_TYPES: [&str; 2] = ["Dividends", "Payment In Lieu Of Dividends"];
const CASH_TRANSFER_TYPES: [&str; 5] = [
    "Deposits/Withdrawals",
    "Broker Interest Received",
    "Broker Interest Paid",
    "Bond Interest Received",
    "Bond Interest Paid",
];
const ACCOUNT_FEE_TYPES: [&str; 2] = ["Other Fees", "Advisor Fees"];
const WITHHOLDING_TAX_TYPE: &str = "Withholding Tax";

fn skipped(external_id: impl Into<String>, reason: impl Into<String>) -> MappedTransaction {
    MappedTransaction::Skipped(SkippedTransaction {
        external_id: external_id.into(),
        reason: reason.into(),
    })
}

fn attr<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn decimal(row: &Value, key: &str) -> Option<Decimal> {
    row.get(key).and_then(crate::util::parse_decimal)
}

/// Time of day is dropped: Flex gives it in local time without an offset.
pub(crate) fn parse_flex_date(raw: &str) -> Option<OffsetDateTime> {
    let digits: String = raw
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .filter(char::is_ascii_digit)
        .collect();
    let date = Date::parse(&digits, FLEX_DATE).ok()?;
    Some(date.midnight().assume_utc())
}

pub(crate) fn instrument_identifier(row: &Value) -> Option<String> {
    let exchange = attr(row, "listingExchange");
    let symbol = attr(row, "symbol")?;
    // LSE lines carry a lowercase suffix (`BATSl`); share-class spaces become dashes.
    let symbol = match exchange {
        Some("LSE" | "LSEETF") => symbol.trim_end_matches(|c: char| c.is_ascii_lowercase()),
        _ => symbol,
    }
    .replace(' ', "-");
    let suffix = match exchange {
        Some("NASDAQ") => Some("NASDAQ"),
        Some("NYSE") => Some("NYSE"),
        Some("ARCA" | "NYSEARCA") => Some("NYSE ARCA"),
        Some("AMEX") => Some("NYSE MKT"),
        Some("BATS") => Some("BATS"),
        Some("PINK") => Some("PINK"),
        Some("LSE" | "LSEETF") => Some("LSE"),
        Some("AEB") => Some("AS"),
        Some("SBF") => Some("PA"),
        Some("ENEXT.BE") => Some("BR"),
        Some("BVL") => Some("LS"),
        Some("IBIS" | "IBIS2") => Some("XETRA"),
        Some("FWB" | "FWB2") => Some("F"),
        Some("EBS" | "SWX") => Some("SW"),
        Some("BM") => Some("MC"),
        Some("VSE") => Some("VI"),
        Some("SFB") => Some("ST"),
        Some("CPH") => Some("CO"),
        Some("HEX") => Some("HE"),
        Some("OSE") => Some("OL"),
        Some("TSE") => Some("TO"),
        Some("ASX") => Some("AU"),
        Some("MEXI") => Some("MX"),
        _ => None,
    };
    Some(match suffix {
        Some(suffix) => format!("{symbol}.{suffix}"),
        None => symbol,
    })
}

fn is_isin(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 12
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Falls back to the ISIN IBKR embeds in dividend descriptions.
pub(crate) fn instrument_isin(row: &Value) -> Option<String> {
    if let Some(isin) = attr(row, "isin").filter(|isin| is_isin(isin)) {
        return Some(isin.to_string());
    }
    let description = attr(row, "description")?;
    let (_, rest) = description.split_once('(')?;
    let (candidate, _) = rest.split_once(')')?;
    is_isin(candidate).then(|| candidate.to_string())
}

/// Dividend rights like `SHELL.DVR` on `CORPACT` stand in for the real stock.
fn is_corporate_action_instrument(row: &Value) -> bool {
    attr(row, "listingExchange") == Some("CORPACT") || attr(row, "subCategory") == Some("RIGHT")
}

fn corporate_action_root_symbol(row: &Value) -> Option<String> {
    let symbol = attr(row, "symbol")?;
    let root = symbol
        .split('.')
        .next()
        .unwrap_or(symbol)
        .trim_end_matches(|c: char| c.is_ascii_digit());
    (!root.is_empty()).then(|| root.replace(' ', "-"))
}

fn map_row(raw: &Value) -> Option<MappedTransaction> {
    match attr(raw, SECTION_KEY)? {
        "Trade" => Some(map_trade(raw)),
        "CashTransaction" => Some(map_cash_transaction(raw)),
        _ => None,
    }
}

/// Withholding tax rows are folded into the dividend sharing their `actionID` and currency.
pub fn map_rows<'a>(rows: impl IntoIterator<Item = &'a Value>) -> Vec<MappedTransaction> {
    // Keyed by transactionID: overlapping statement windows repeat rows.
    let mut withholding: HashMap<(&str, &str), HashMap<&str, Decimal>> = HashMap::new();
    let mut mapped = Vec::new();
    for raw in rows {
        if let Some((key, transaction_id, amount)) = withholding_tax_row(raw) {
            withholding
                .entry(key)
                .or_default()
                .insert(transaction_id, amount);
            continue;
        }
        if let Some(row) = map_row(raw) {
            mapped.push((attr(raw, "actionID").zip(attr(raw, "currency")), row));
        }
    }

    // An action can also hold reversals and re-bookings, so only one dividend takes the tax.
    let mut recipients: HashMap<(&str, &str), String> = HashMap::new();
    for (key, row) in &mapped {
        let (Some(key), MappedTransaction::Provider(tx)) = (key, row) else {
            continue;
        };
        if matches!(tx.kind, ProviderTransactionKind::Dividend { .. })
            && tx.amount.is_sign_positive()
            && withholding.contains_key(key)
        {
            recipients
                .entry(*key)
                .and_modify(|id| {
                    if tx.external_id < *id {
                        id.clone_from(&tx.external_id);
                    }
                })
                .or_insert_with(|| tx.external_id.clone());
        }
    }
    for (key, row) in &mut mapped {
        let (Some(key), MappedTransaction::Provider(tx)) = (key, row) else {
            continue;
        };
        if recipients.get(key) == Some(&tx.external_id) {
            tx.kind = ProviderTransactionKind::Dividend {
                withholding_tax: withholding[key].values().sum(),
            };
        }
    }

    let unmatched = withholding
        .into_iter()
        .filter(|(key, _)| !recipients.contains_key(key))
        .flat_map(|(_, rows)| rows.into_keys())
        .map(|transaction_id| {
            skipped(
                format!("cash:{transaction_id}"),
                "withholding tax without a matching dividend",
            )
        });
    mapped
        .into_iter()
        .map(|(_, row)| row)
        .chain(unmatched)
        .collect()
}

fn withholding_tax_row(raw: &Value) -> Option<((&str, &str), &str, Decimal)> {
    if attr(raw, SECTION_KEY)? != "CashTransaction"
        || attr(raw, "type")? != WITHHOLDING_TAX_TYPE
        || attr(raw, "levelOfDetail").is_some_and(|level| level != "DETAIL")
    {
        return None;
    }
    Some((
        (attr(raw, "actionID")?, attr(raw, "currency")?),
        attr(raw, "transactionID")?,
        decimal(raw, "amount")?,
    ))
}

pub fn map_trade(raw: &Value) -> MappedTransaction {
    let Some(trade_id) = attr(raw, "tradeID") else {
        return skipped(
            "<missing tradeID>",
            "missing tradeID — cannot identify trade",
        );
    };
    let external_id = format!("trade:{trade_id}");

    if let Some(level) = attr(raw, "levelOfDetail") {
        if level != "EXECUTION" {
            return skipped(external_id, format!("not an execution row ({level})"));
        }
    }
    let category = attr(raw, "assetCategory").unwrap_or("unknown");
    if category == "CASH" {
        return skipped(external_id, "currency conversion — not imported");
    }
    if !SUPPORTED_ASSET_CATEGORIES.contains(&category) {
        return skipped(
            external_id,
            format!("asset category {category} not supported"),
        );
    }

    let Some(asset_identifier) = instrument_identifier(raw) else {
        return skipped(external_id, "missing symbol");
    };
    let Some(quantity) = decimal(raw, "quantity") else {
        return skipped(external_id, "missing or unparseable quantity");
    };
    if quantity.is_zero() {
        return skipped(external_id, "zero-quantity trade");
    }
    let Some(amount) = decimal(raw, "proceeds") else {
        return skipped(external_id, "missing or unparseable proceeds");
    };
    let Some(currency) = attr(raw, "currency") else {
        return skipped(external_id, "missing currency");
    };
    let commission = decimal(raw, "ibCommission").unwrap_or_default();
    if !commission.is_zero() && attr(raw, "ibCommissionCurrency").is_some_and(|c| c != currency) {
        return skipped(external_id, "commission charged in another currency");
    }
    let fee = commission + decimal(raw, "taxes").unwrap_or_default();
    let Some(date) = attr(raw, "dateTime")
        .or_else(|| attr(raw, "tradeDate"))
        .and_then(parse_flex_date)
    else {
        return skipped(external_id, "missing or unparseable trade date");
    };
    let side = if quantity.is_sign_positive() {
        "BUY"
    } else {
        "SELL"
    };

    MappedTransaction::Provider(ProviderTransaction {
        external_id,
        amount,
        currency: currency.to_string(),
        date,
        description: format!("{side} {} {asset_identifier}", quantity.abs()),
        asset_identifier: Some(asset_identifier),
        isin: instrument_isin(raw),
        quantity: Some(quantity),
        kind: ProviderTransactionKind::Trade { fee },
    })
}

pub fn map_cash_transaction(raw: &Value) -> MappedTransaction {
    let Some(transaction_id) = attr(raw, "transactionID") else {
        return skipped(
            "<missing transactionID>",
            "missing transactionID — cannot identify cash transaction",
        );
    };
    let external_id = format!("cash:{transaction_id}");

    if let Some(level) = attr(raw, "levelOfDetail") {
        if level != "DETAIL" {
            return skipped(external_id, format!("not a detail row ({level})"));
        }
    }

    let Some(amount) = decimal(raw, "amount") else {
        return skipped(external_id, "missing or unparseable amount");
    };
    let Some(currency) = attr(raw, "currency") else {
        return skipped(external_id, "missing currency");
    };
    let Some(date) = attr(raw, "dateTime")
        .or_else(|| attr(raw, "settleDate"))
        .or_else(|| attr(raw, "reportDate"))
        .and_then(parse_flex_date)
    else {
        return skipped(external_id, "missing or unparseable date");
    };
    let tx_type = attr(raw, "type").unwrap_or("unknown");
    let kind = if DIVIDEND_TYPES.contains(&tx_type) {
        ProviderTransactionKind::Dividend {
            withholding_tax: Decimal::ZERO,
        }
    } else if CASH_TRANSFER_TYPES.contains(&tx_type) {
        ProviderTransactionKind::CashTransfer
    } else if ACCOUNT_FEE_TYPES.contains(&tx_type) {
        ProviderTransactionKind::AccountFee
    } else if tx_type == WITHHOLDING_TAX_TYPE {
        return skipped(external_id, "withholding tax without a matching dividend");
    } else {
        return skipped(
            external_id,
            format!("cash transaction type {tx_type} not supported"),
        );
    };
    let (asset_identifier, isin) = if !DIVIDEND_TYPES.contains(&tx_type) {
        (None, None)
    } else if is_corporate_action_instrument(raw) {
        // The pseudo-instrument's ISIN is throwaway and never matches an asset.
        (corporate_action_root_symbol(raw), None)
    } else {
        (instrument_identifier(raw), instrument_isin(raw))
    };
    let description = attr(raw, "description").unwrap_or(tx_type).to_string();

    MappedTransaction::Provider(ProviderTransaction {
        external_id,
        amount,
        currency: currency.to_string(),
        date,
        description,
        asset_identifier,
        isin,
        quantity: None,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dec(value: &str) -> Decimal {
        value.parse().unwrap()
    }

    fn provider(mapped: MappedTransaction) -> ProviderTransaction {
        match mapped {
            MappedTransaction::Provider(p) => p,
            MappedTransaction::Skipped(s) => panic!("unexpectedly skipped: {}", s.reason),
        }
    }

    fn skip_reason(mapped: MappedTransaction) -> String {
        match mapped {
            MappedTransaction::Skipped(s) => s.reason,
            MappedTransaction::Provider(p) => panic!("unexpectedly mapped: {}", p.external_id),
        }
    }

    fn trade(overrides: Value) -> Value {
        let mut row = json!({
            "_section": "Trade",
            "currency": "USD",
            "assetCategory": "STK",
            "symbol": "AAPL",
            "listingExchange": "NASDAQ",
            "tradeID": "111",
            "dateTime": "20260105;093512",
            "quantity": "10",
            "proceeds": "-1900",
            "ibCommission": "-1",
            "ibCommissionCurrency": "USD",
            "taxes": "-0.25",
            "netCash": "-1901.25",
            "buySell": "BUY",
            "levelOfDetail": "EXECUTION",
        });
        for (k, v) in overrides.as_object().unwrap() {
            row[k] = v.clone();
        }
        row
    }

    #[test]
    fn maps_buy_trade() {
        let tx = provider(map_row(&trade(json!({}))).unwrap());
        assert_eq!(tx.external_id, "trade:111");
        assert_eq!(tx.amount, dec("-1900"));
        assert_eq!(
            tx.kind,
            ProviderTransactionKind::Trade { fee: dec("-1.25") }
        );
        assert_eq!(tx.quantity, Some(dec("10")));
        assert_eq!(tx.asset_identifier.as_deref(), Some("AAPL.NASDAQ"));
        assert_eq!(tx.currency, "USD");
        assert_eq!(
            tx.date.date(),
            Date::from_calendar_date(2026, time::Month::January, 5).unwrap()
        );
        assert_eq!(tx.description, "BUY 10 AAPL.NASDAQ");
    }

    #[test]
    fn maps_sell_trade_on_lse_etf() {
        let tx = provider(map_trade(&trade(json!({
            "symbol": "VWRP",
            "listingExchange": "LSEETF",
            "currency": "GBP",
            "quantity": "-3",
            "proceeds": "313.40",
            "ibCommission": "-1",
            "ibCommissionCurrency": "GBP",
            "taxes": "0",
            "netCash": "312.40",
            "buySell": "SELL",
            "dateTime": "2026-02-10;14:01:00",
        }))));
        assert_eq!(tx.quantity, Some(dec("-3")));
        assert_eq!(tx.amount, dec("313.40"));
        assert_eq!(tx.kind, ProviderTransactionKind::Trade { fee: dec("-1") });
        assert_eq!(tx.asset_identifier.as_deref(), Some("VWRP.LSE"));
        assert_eq!(
            tx.date.date(),
            Date::from_calendar_date(2026, time::Month::February, 10).unwrap()
        );
    }

    #[test]
    fn normalizes_instrument_identifiers_to_asset_tickers() {
        let cases = [
            ("BATSl", "LSE", "BATS.LSE"),
            ("HMEFz", "LSEETF", "HMEF.LSE"),
            ("PBR A", "NYSE", "PBR-A.NYSE"),
            ("SPY", "ARCA", "SPY.NYSE ARCA"),
            ("BABAN", "MEXI", "BABAN.MX"),
            ("PRX", "AEB", "PRX.AS"),
            ("NOVO B", "CPH", "NOVO-B.CO"),
            ("SHELL.DVR", "CORPACT", "SHELL.DVR"),
        ];
        for (symbol, exchange, expected) in cases {
            let row = json!({ "symbol": symbol, "listingExchange": exchange });
            assert_eq!(
                instrument_identifier(&row).as_deref(),
                Some(expected),
                "{symbol} on {exchange}"
            );
        }
    }

    #[test]
    fn skips_fx_and_unsupported_trades() {
        assert_eq!(
            skip_reason(map_trade(&trade(json!({ "assetCategory": "CASH" })))),
            "currency conversion — not imported"
        );
        assert_eq!(
            skip_reason(map_trade(&trade(json!({ "assetCategory": "OPT" })))),
            "asset category OPT not supported"
        );
        assert_eq!(
            skip_reason(map_trade(&trade(json!({ "levelOfDetail": "ORDER" })))),
            "not an execution row (ORDER)"
        );
        assert_eq!(
            skip_reason(map_trade(&trade(json!({ "ibCommissionCurrency": "EUR" })))),
            "commission charged in another currency"
        );
    }

    #[test]
    fn maps_deposit_as_cash_transfer() {
        let tx = provider(
            map_row(&json!({
                "_section": "CashTransaction",
                "currency": "EUR",
                "type": "Deposits/Withdrawals",
                "amount": "500",
                "description": "CASH RECEIPTS / ELECTRONIC FUND TRANSFERS",
                "transactionID": "222",
                "dateTime": "20260102",
                "levelOfDetail": "DETAIL",
            }))
            .unwrap(),
        );
        assert_eq!(tx.external_id, "cash:222");
        assert_eq!(tx.amount, dec("500"));
        assert_eq!(tx.kind, ProviderTransactionKind::CashTransfer);
        assert_eq!(tx.asset_identifier, None);
        assert_eq!(tx.quantity, None);
    }

    #[test]
    fn maps_dividend_with_origin_asset() {
        let tx = provider(map_cash_transaction(&json!({
            "currency": "USD",
            "type": "Dividends",
            "symbol": "MSFT",
            "listingExchange": "NASDAQ",
            "amount": "8.30",
            "description": "MSFT(US5949181045) Cash Dividend USD 0.83 per Share",
            "transactionID": "333",
            "dateTime": "20260312;202000",
        })));
        assert_eq!(tx.asset_identifier.as_deref(), Some("MSFT.NASDAQ"));
        assert_eq!(tx.isin.as_deref(), Some("US5949181045"));
        assert_eq!(tx.quantity, None);
    }

    #[test]
    fn maps_dividend_without_listing_exchange_with_isin_fallback() {
        let tx = provider(map_cash_transaction(&json!({
            "currency": "EUR",
            "type": "Dividends",
            "symbol": "ASML",
            "isin": "NL0010273215",
            "amount": "12.80",
            "description": "ASML(NL0010273215) Cash Dividend EUR 1.60 per Share (Ordinary Dividend)",
            "transactionID": "335",
            "dateTime": "20260430",
        })));
        assert_eq!(tx.asset_identifier.as_deref(), Some("ASML"));
        assert_eq!(tx.isin.as_deref(), Some("NL0010273215"));
    }

    #[test]
    fn reads_isin_from_attribute_or_description() {
        assert_eq!(
            instrument_isin(&json!({ "isin": "IE00BK5BQT80" })).as_deref(),
            Some("IE00BK5BQT80")
        );
        assert_eq!(
            instrument_isin(&json!({
                "isin": "",
                "description": "PBR A(US71654V1017) Cash Dividend USD 0.18 per Share",
            }))
            .as_deref(),
            Some("US71654V1017")
        );
        assert_eq!(
            instrument_isin(&json!({ "description": "CASH RECEIPTS (ELECTRONIC)" })),
            None
        );
    }

    fn corporate_action_dividend(symbol: &str, amount: &str) -> Value {
        json!({
            "currency": "EUR",
            "type": "Dividends",
            "symbol": symbol,
            "listingExchange": "CORPACT",
            "assetCategory": "STK",
            "subCategory": "RIGHT",
            "isin": "NL0015002AR9",
            "underlyingSymbol": symbol,
            "amount": amount,
            "description": format!("{symbol}(NL0015002AR9) EXPIRE DIVIDEND RIGHT (Ordinary Dividend)"),
            "transactionID": "336",
            "dateTime": "20260915",
        })
    }

    #[test]
    fn maps_corporate_action_dividend_to_root_symbol() {
        for symbol in [
            "SHELL.DVR",
            "SHELL.DRT",
            "SHELL.DRS",
            "SHELL.DVD",
            "SHELL.DDR",
            "SHELL.DvR",
            "SHELL1.DI",
            "SHELL.2DR",
        ] {
            let tx = provider(map_cash_transaction(&corporate_action_dividend(
                symbol, "0.33",
            )));
            assert_eq!(tx.asset_identifier.as_deref(), Some("SHELL"), "{symbol}");
            assert_eq!(tx.isin, None, "{symbol}");
            assert_eq!(tx.quantity, None, "{symbol}");
        }
    }

    #[test]
    fn corporate_action_root_symbol_uses_dashed_share_class() {
        let tx = provider(map_cash_transaction(&corporate_action_dividend(
            "PBR A.DVR",
            "0.33",
        )));
        assert_eq!(tx.asset_identifier.as_deref(), Some("PBR-A"));
    }

    #[test]
    fn maps_corporate_action_dividend_reversal_to_root_symbol() {
        let tx = provider(map_cash_transaction(&corporate_action_dividend(
            "SHELL.DVR",
            "-0.33",
        )));
        assert_eq!(tx.amount, dec("-0.33"));
        assert_eq!(tx.asset_identifier.as_deref(), Some("SHELL"));
    }

    #[test]
    fn regular_dividend_keeps_exchange_ticker_and_isin() {
        let tx = provider(map_cash_transaction(&json!({
            "currency": "USD",
            "type": "Dividends",
            "symbol": "MSFT",
            "listingExchange": "NASDAQ",
            "subCategory": "COMMON",
            "amount": "8.30",
            "description": "MSFT(US5949181045) Cash Dividend USD 0.83 per Share",
            "transactionID": "333",
            "dateTime": "20260312",
        })));
        assert_eq!(tx.asset_identifier.as_deref(), Some("MSFT.NASDAQ"));
        assert_eq!(tx.isin.as_deref(), Some("US5949181045"));
    }

    #[test]
    fn non_dividend_corporate_action_row_has_no_instrument() {
        let tx = provider(map_cash_transaction(&json!({
            "currency": "EUR",
            "type": "Other Fees",
            "symbol": "SHELL.DVR",
            "listingExchange": "CORPACT",
            "subCategory": "RIGHT",
            "amount": "-0.01",
            "transactionID": "337",
            "dateTime": "20260915",
        })));
        assert_eq!(tx.asset_identifier, None);
        assert_eq!(tx.kind, ProviderTransactionKind::AccountFee);
    }

    #[test]
    fn maps_broker_interest_as_cash_transfer() {
        let tx = provider(map_cash_transaction(&json!({
            "currency": "EUR",
            "type": "Broker Interest Paid",
            "amount": "-0.42",
            "transactionID": "338",
            "dateTime": "20260903",
        })));
        assert_eq!(tx.kind, ProviderTransactionKind::CashTransfer);
        assert_eq!(tx.amount, dec("-0.42"));
    }

    #[test]
    fn skips_unsupported_cash_transaction_types() {
        assert_eq!(
            skip_reason(map_cash_transaction(&json!({
                "currency": "EUR",
                "type": "Price Adjustments",
                "amount": "1",
                "transactionID": "339",
                "dateTime": "20260903",
            }))),
            "cash transaction type Price Adjustments not supported"
        );
    }

    fn cash_row(transaction_id: &str, tx_type: &str, amount: &str, action_id: &str) -> Value {
        json!({
            "_section": "CashTransaction",
            "levelOfDetail": "DETAIL",
            "currency": "USD",
            "type": tx_type,
            "symbol": "MSFT",
            "listingExchange": "NASDAQ",
            "amount": amount,
            "transactionID": transaction_id,
            "actionID": action_id,
            "dateTime": "20260312",
        })
    }

    fn mapped_rows(rows: &[Value]) -> (Vec<ProviderTransaction>, Vec<SkippedTransaction>) {
        let mut transactions = Vec::new();
        let mut skipped = Vec::new();
        for row in map_rows(rows) {
            match row {
                MappedTransaction::Provider(p) => transactions.push(p),
                MappedTransaction::Skipped(s) => skipped.push(s),
            }
        }
        (transactions, skipped)
    }

    #[test]
    fn folds_withholding_tax_into_its_dividend() {
        let (transactions, skipped) = mapped_rows(&[
            cash_row("333", "Dividends", "8.30", "A1"),
            cash_row("334", "Withholding Tax", "-1.25", "A1"),
            cash_row("335", "Withholding Tax", "-0.10", "A1"),
            // Overlapping statement windows repeat rows.
            cash_row("334", "Withholding Tax", "-1.25", "A1"),
        ]);
        assert!(skipped.is_empty());
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].external_id, "cash:333");
        assert_eq!(
            transactions[0].kind,
            ProviderTransactionKind::Dividend {
                withholding_tax: dec("-1.35")
            }
        );
    }

    #[test]
    fn withholding_tax_goes_to_one_positive_dividend_of_the_action() {
        let (transactions, _) = mapped_rows(&[
            cash_row("402", "Dividends", "8.30", "A2"),
            cash_row("401", "Dividends", "-8.30", "A2"),
            cash_row("400", "Dividends", "8.30", "A2"),
            cash_row("403", "Withholding Tax", "-1.25", "A2"),
        ]);
        let taxes: Vec<(&str, ProviderTransactionKind)> = transactions
            .iter()
            .map(|tx| (tx.external_id.as_str(), tx.kind))
            .collect();
        let no_tax = ProviderTransactionKind::Dividend {
            withholding_tax: Decimal::ZERO,
        };
        assert_eq!(
            taxes,
            vec![
                ("cash:402", no_tax),
                ("cash:401", no_tax),
                (
                    "cash:400",
                    ProviderTransactionKind::Dividend {
                        withholding_tax: dec("-1.25")
                    }
                ),
            ]
        );
    }

    #[test]
    fn skips_withholding_tax_without_a_matching_dividend() {
        let (transactions, skipped) = mapped_rows(&[
            cash_row("333", "Dividends", "8.30", "A1"),
            cash_row("334", "Withholding Tax", "-1.25", "B9"),
        ]);
        assert_eq!(
            transactions[0].kind,
            ProviderTransactionKind::Dividend {
                withholding_tax: Decimal::ZERO
            }
        );
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].external_id, "cash:334");
        assert_eq!(
            skipped[0].reason,
            "withholding tax without a matching dividend"
        );
    }

    #[test]
    fn ignores_non_transaction_sections() {
        assert!(map_row(&json!({ "_section": "OpenPosition" })).is_none());
    }
}
