use std::time::Duration;

use serde_json::Value;
use time::{Date, OffsetDateTime};
use url::Url;

use crate::ibkr::mapper::{instrument_identifier, parse_flex_date, FLEX_DATE};
use crate::ibkr::models::FlexReply;
use crate::ibkr::statement::{parse_reply, SECTION_KEY};
use crate::models::balance::{ProviderAssetBalance, ProviderBalance, ProviderCashBalance};
use crate::models::sync::{FetchedPage, RawPage, SyncCursor};
use crate::models::transaction::ProviderTransaction;
use crate::port::{Connector, ProviderCapabilities};
use crate::Result;

const SEND_REQUEST_URL: &str =
    "https://ndcdyn.interactivebrokers.com/AccountManagement/FlexWebService/SendRequest";
/// Pinned: the reply's `gdcdyn` host may not resolve, and the token mustn't follow it.
const GET_STATEMENT_URL: &str =
    "https://ndcdyn.interactivebrokers.com/AccountManagement/FlexWebService/GetStatement";
const FLEX_VERSION: &str = "3";
const USER_AGENT: &str = "Sverto/1.0";
const STREAM: &str = "statement";
/// Flex accepts at most 365 days per request (`fd`..`td` inclusive).
const MAX_WINDOW_DAYS: i64 = 364;
const MAX_HISTORY_DAYS: i64 = 5 * 365;
const MAX_ATTEMPTS: u32 = 5;
/// Doubles per attempt: repeated requests count towards IBKR's lockout (code 1025).
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(6);
const RETRYABLE_CODES: [u32; 5] = [1001, 1009, 1018, 1019, 1021];
const STATEMENT_NOT_AVAILABLE: u32 = 1003;

pub struct IbkrFlexClient {
    // Untraced: the token is a query parameter and spans record full URLs.
    http: reqwest::Client,
    query_id: String,
    token: String,
    account_opened: tokio::sync::OnceCell<Date>,
}

impl IbkrFlexClient {
    pub fn new(query_id: String, token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            query_id,
            token,
            account_opened: tokio::sync::OnceCell::new(),
        }
    }

    async fn fetch_statement(&self, period: Option<(Date, Date)>) -> Result<Option<Vec<Value>>> {
        let mut params = vec![
            ("t", self.token.clone()),
            ("q", self.query_id.clone()),
            ("v", FLEX_VERSION.to_string()),
        ];
        if let Some((from, to)) = period {
            params.push(("fd", from.format(FLEX_DATE)?));
            params.push(("td", to.format(FLEX_DATE)?));
        }

        let mut accepted = None;
        for attempt in 0..MAX_ATTEMPTS {
            let reply = parse_reply(&self.get(SEND_REQUEST_URL, &params).await?)?;
            log_reply("SendRequest", period, attempt, &reply);
            match reply {
                FlexReply::Accepted { reference_code } => {
                    accepted = Some(reference_code);
                    break;
                }
                FlexReply::Failed {
                    code: Some(STATEMENT_NOT_AVAILABLE),
                    ..
                } => return Ok(None),
                FlexReply::Failed { code, message } => ensure_retryable(code, &message)?,
                FlexReply::Statement(_) => {
                    anyhow::bail!("IBKR flex SendRequest returned a statement")
                }
            }
            if attempt + 1 < MAX_ATTEMPTS {
                tokio::time::sleep(retry_delay(attempt)).await;
            }
        }
        let Some(reference_code) = accepted else {
            anyhow::bail!("IBKR flex request not accepted after {MAX_ATTEMPTS} attempts");
        };

        let statement_params = [
            ("t", self.token.clone()),
            ("q", reference_code),
            ("v", FLEX_VERSION.to_string()),
        ];
        for attempt in 0..MAX_ATTEMPTS {
            tokio::time::sleep(retry_delay(attempt)).await;
            let reply = parse_reply(&self.get(GET_STATEMENT_URL, &statement_params).await?)?;
            log_reply("GetStatement", period, attempt, &reply);
            match reply {
                FlexReply::Statement(rows) => return Ok(Some(rows)),
                FlexReply::Failed {
                    code: Some(STATEMENT_NOT_AVAILABLE),
                    ..
                } => return Ok(None),
                FlexReply::Failed { code, message } => ensure_retryable(code, &message)?,
                FlexReply::Accepted { .. } => {
                    anyhow::bail!("IBKR flex GetStatement returned an unexpected envelope")
                }
            }
        }
        anyhow::bail!("IBKR flex statement not ready after {MAX_ATTEMPTS} attempts")
    }

    async fn account_opened(&self) -> Result<Date> {
        self.account_opened
            .get_or_try_init(|| async {
                let rows = self
                    .fetch_statement(None)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("IBKR flex statement is not available"))?;
                parse_account_opened(&rows).ok_or_else(|| {
                    anyhow::anyhow!(
                        "IBKR flex query has no account opening date; add the Account Information section"
                    )
                })
            })
            .await
            .copied()
    }

    async fn get(&self, url: &str, params: &[(&str, String)]) -> Result<String> {
        let url = Url::parse_with_params(url, params)?;
        let resp = self
            .http
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        let resp = crate::util::ensure_success(resp).await?;
        Ok(resp.text().await.map_err(reqwest::Error::without_url)?)
    }
}

/// Never logs the token or the reference code.
fn log_reply(step: &str, period: Option<(Date, Date)>, attempt: u32, reply: &FlexReply) {
    let period = period.map(|(from, to)| format!("{from}..{to}"));
    match reply {
        FlexReply::Accepted { .. } => {
            tracing::info!(step, ?period, attempt, "IBKR flex request accepted");
        }
        FlexReply::Statement(rows) => {
            tracing::info!(
                step,
                ?period,
                attempt,
                rows = rows.len(),
                "IBKR flex statement received"
            );
        }
        FlexReply::Failed { code, message }
            if code.is_some_and(|c| RETRYABLE_CODES.contains(&c)) =>
        {
            tracing::info!(step, ?period, attempt, ?code, %message, "IBKR flex statement not ready");
        }
        FlexReply::Failed { code, message } => {
            tracing::warn!(step, ?period, attempt, ?code, %message, "IBKR flex request failed");
        }
    }
}

fn retry_delay(attempt: u32) -> Duration {
    INITIAL_RETRY_DELAY * 2u32.pow(attempt)
}

fn ensure_retryable(code: Option<u32>, message: &str) -> Result<()> {
    match code {
        Some(code) if RETRYABLE_CODES.contains(&code) => Ok(()),
        Some(code) => anyhow::bail!("IBKR flex request failed ({code}): {message}"),
        None => anyhow::bail!("IBKR flex request failed: {message}"),
    }
}

#[async_trait::async_trait]
impl Connector for IbkrFlexClient {
    async fn fetch_page(
        &self,
        from: Option<OffsetDateTime>,
        cursor: Option<SyncCursor>,
    ) -> Result<FetchedPage> {
        // Flex data is end-of-day.
        let last_day = OffsetDateTime::now_utc().date() - time::Duration::days(1);
        let mut window_start = cursor
            .as_ref()
            .and_then(|c| c.as_value().get("from")?.as_str().map(str::to_string))
            .map(|s| Date::parse(&s, FLEX_DATE))
            .transpose()?
            .or_else(|| from.map(|f| f.date()))
            .unwrap_or(last_day - time::Duration::days(MAX_HISTORY_DAYS))
            .min(last_day);
        // A multi-window walk would start before the account opened, which Flex refuses (1003).
        if cursor.is_none() && window_start + time::Duration::days(MAX_WINDOW_DAYS) < last_day {
            match self.account_opened().await {
                Ok(opened) => window_start = window_start.max(opened.min(last_day)),
                Err(e) => tracing::warn!(
                    error = %e,
                    "IBKR flex account opening date unavailable; walking from the history start"
                ),
            }
        }
        let window_end = (window_start + time::Duration::days(MAX_WINDOW_DAYS)).min(last_day);

        let mut next_start = window_end + time::Duration::days(1);
        let statement = match self
            .fetch_statement(Some((window_start, window_end)))
            .await?
        {
            Some(rows) => rows,
            // Flex refuses periods before the account opened; any other unavailability is an error.
            None => {
                let opened = self.account_opened().await?;
                if opened > window_end {
                    next_start = opened;
                    Vec::new()
                } else if opened > window_start {
                    self.fetch_statement(Some((opened, window_end)))
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "IBKR flex statement is not available for {opened}..{window_end}"
                            )
                        })?
                } else {
                    anyhow::bail!(
                        "IBKR flex statement is not available for {window_start}..{window_end}"
                    )
                }
            }
        };
        let rows = statement
            .into_iter()
            .filter(|row| {
                matches!(
                    row.get(SECTION_KEY).and_then(|v| v.as_str()),
                    Some("Trade" | "CashTransaction")
                )
            })
            .collect();

        let next_cursor = if next_start <= last_day {
            Some(SyncCursor::new(
                serde_json::json!({ "from": next_start.format(FLEX_DATE)? }),
            ))
        } else {
            None
        };

        Ok(FetchedPage {
            stream: STREAM.to_string(),
            payload: Value::Array(rows),
            next_cursor,
        })
    }

    async fn fetch_balance(&self) -> Result<ProviderBalance> {
        let rows = self
            .fetch_statement(None)
            .await?
            .ok_or_else(|| anyhow::anyhow!("IBKR flex statement is not available"))?;
        Ok(ProviderBalance {
            quantities: parse_positions(&rows),
            cash: parse_cash(&rows),
        })
    }

    fn map_pages(&self, pages: &[RawPage]) -> Vec<ProviderTransaction> {
        crate::provider::map_pages(crate::provider::ProviderKind::Ibkr, pages)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            max_history: Some(time::Duration::days(MAX_HISTORY_DAYS)),
        }
    }
}

fn section<'a>(rows: &'a [Value], name: &'a str) -> impl Iterator<Item = &'a Value> {
    rows.iter()
        .filter(move |row| row.get(SECTION_KEY).and_then(|v| v.as_str()) == Some(name))
}

fn parse_positions(rows: &[Value]) -> Vec<ProviderAssetBalance> {
    section(rows, "OpenPosition")
        .filter(|row| row.get("levelOfDetail").and_then(|v| v.as_str()) != Some("LOT"))
        .filter_map(|row| {
            Some(ProviderAssetBalance {
                asset_identifier: instrument_identifier(row)?,
                quantity: crate::util::parse_decimal(row.get("position")?)?,
            })
        })
        .collect()
}

fn parse_account_opened(rows: &[Value]) -> Option<Date> {
    section(rows, "AccountInformation")
        .find_map(|row| parse_flex_date(row.get("dateOpened")?.as_str()?))
        .map(|opened| opened.date())
}

fn parse_cash(rows: &[Value]) -> Vec<ProviderCashBalance> {
    section(rows, "CashReportCurrency")
        .filter_map(|row| {
            let currency = row.get("currency")?.as_str()?;
            if currency == "BASE_SUMMARY" {
                return None;
            }
            Some(ProviderCashBalance {
                currency: currency.to_string(),
                amount: crate::util::parse_decimal(row.get("endingCash")?)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_balances_from_statement_rows() {
        let rows = vec![
            json!({ "_section": "OpenPosition", "symbol": "AAPL", "listingExchange": "NASDAQ", "position": "10", "levelOfDetail": "SUMMARY" }),
            json!({ "_section": "OpenPosition", "symbol": "AAPL", "listingExchange": "NASDAQ", "position": "4", "levelOfDetail": "LOT" }),
            json!({ "_section": "CashReportCurrency", "currency": "BASE_SUMMARY", "endingCash": "999" }),
            json!({ "_section": "CashReportCurrency", "currency": "USD", "endingCash": "120.50" }),
        ];

        let positions = parse_positions(&rows);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].asset_identifier, "AAPL.NASDAQ");
        assert_eq!(positions[0].quantity.to_string(), "10");

        let cash = parse_cash(&rows);
        assert_eq!(cash.len(), 1);
        assert_eq!(cash[0].currency, "USD");
        assert_eq!(cash[0].amount.to_string(), "120.50");
    }

    #[test]
    fn retry_delay_doubles_from_six_seconds() {
        let delays: Vec<u64> = (0..MAX_ATTEMPTS)
            .map(|attempt| retry_delay(attempt).as_secs())
            .collect();
        assert_eq!(delays, [6, 12, 24, 48, 96]);
    }

    #[test]
    fn parses_account_opening_date() {
        let rows = vec![
            json!({ "_section": "Trade", "dateOpened": "20200101" }),
            json!({ "_section": "AccountInformation", "accountId": "U1234567", "dateOpened": "20220314" }),
        ];
        assert_eq!(
            parse_account_opened(&rows),
            Some(Date::from_calendar_date(2022, time::Month::March, 14).unwrap())
        );
        assert_eq!(parse_account_opened(&rows[..1]), None);
    }
}
