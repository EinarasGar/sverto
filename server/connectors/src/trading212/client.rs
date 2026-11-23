use crate::models::balance::{ProviderAssetBalance, ProviderBalance, ProviderCashBalance};
use crate::models::sync::{FetchedPage, RateLimited, RawPage, SyncCursor};
use crate::models::transaction::ProviderTransaction;
use crate::port::Connector;
use crate::Result;
use base64::Engine;
use observability::{create_http_client, TracedHttpClient};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trading212Env {
    Demo,
    Live,
}

const MAX_RATE_LIMIT_WAITS: u32 = 5;
const MAX_RATE_LIMIT_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

pub struct Trading212Client {
    http: TracedHttpClient,
    base_url: String,
    api_key_id: String,
    api_secret: String,
    waits_for_rate_limit: bool,
}

impl Trading212Client {
    pub fn new(api_key_id: String, api_secret: String, env: Trading212Env) -> Self {
        let base_url = match env {
            Trading212Env::Demo => "https://demo.trading212.com".to_string(),
            Trading212Env::Live => "https://live.trading212.com".to_string(),
        };
        Self {
            http: create_http_client(),
            base_url,
            api_key_id,
            api_secret,
            waits_for_rate_limit: false,
        }
    }

    pub fn waiting_for_rate_limits(mut self) -> Self {
        self.waits_for_rate_limit = true;
        self
    }

    fn basic_auth(&self) -> String {
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.api_key_id, self.api_secret));
        format!("Basic {}", creds)
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response> {
        let mut waits = 0;
        loop {
            let resp = self
                .http
                .get(url)
                .header("Authorization", self.basic_auth())
                .send()
                .await?;
            if resp.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
                return crate::util::ensure_success(resp).await;
            }
            if !self.waits_for_rate_limit {
                return Err(RateLimited.into());
            }
            let wait = (waits < MAX_RATE_LIMIT_WAITS)
                .then(|| rate_limit_wait(resp.headers(), time::OffsetDateTime::now_utc()))
                .flatten();
            let Some(wait) = wait else {
                return crate::util::ensure_success(resp).await;
            };
            waits += 1;
            tracing::info!(
                wait_secs = wait.as_secs(),
                waits,
                "trading212 rate limit reached; waiting for it to reset"
            );
            tokio::time::sleep(wait).await;
        }
    }
}

fn rate_limit_wait(
    headers: &reqwest::header::HeaderMap,
    now: time::OffsetDateTime,
) -> Option<std::time::Duration> {
    let header = |name: &str| headers.get(name)?.to_str().ok()?.trim().parse::<i64>().ok();
    let seconds = match (header("x-ratelimit-reset"), header("retry-after")) {
        (Some(reset), _) => reset - now.unix_timestamp() + 1,
        (None, Some(seconds)) => seconds,
        (None, None) => return None,
    };
    let wait = std::time::Duration::from_secs(u64::try_from(seconds.max(1)).ok()?);
    (wait <= MAX_RATE_LIMIT_WAIT).then_some(wait)
}

#[async_trait::async_trait]
impl Connector for Trading212Client {
    async fn fetch_page(
        &self,
        from: Option<time::OffsetDateTime>,
        cursor: Option<SyncCursor>,
    ) -> Result<FetchedPage> {
        let (stream, path) = decode_cursor(cursor.as_ref());
        let url = match &path {
            Some(p) if p.starts_with('/') => format!("{}{}", self.base_url, p),
            Some(p) => format!("{}{}?{}", self.base_url, stream_endpoint(&stream), p),
            None => format!("{}{}?limit=50", self.base_url, stream_endpoint(&stream)),
        };

        let body: serde_json::Value = self.get(&url).await?.json().await?;

        let items: Vec<serde_json::Value> = body
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let next_page_path = body.get("nextPagePath").and_then(|v| v.as_str());
        let stream_exhausted = page_covers_from(&stream, &items, from)
            || next_page_path.is_none()
            || items.is_empty()
            || next_page_path == path.as_deref();

        let next_cursor = if stream_exhausted {
            next_stream(&stream).map(|next| encode_cursor(next, None))
        } else {
            next_page_path.map(|p| encode_cursor(&stream, Some(p)))
        };

        Ok(FetchedPage {
            stream,
            payload: serde_json::Value::Array(items),
            next_cursor,
        })
    }

    async fn fetch_balance(&self) -> Result<ProviderBalance> {
        let portfolio_url = format!("{}/api/v0/equity/portfolio", self.base_url);
        let portfolio_body: serde_json::Value = self.get(&portfolio_url).await?.json().await?;
        let quantities = parse_portfolio_quantities(&portfolio_body);

        let cash_url = format!("{}/api/v0/equity/account/cash", self.base_url);
        let cash_body: serde_json::Value = self.get(&cash_url).await?.json().await?;
        let cash = parse_cash_balance(&cash_body);

        Ok(ProviderBalance { quantities, cash })
    }

    fn map_pages(&self, pages: &[RawPage]) -> Vec<ProviderTransaction> {
        crate::provider::map_pages(crate::provider::ProviderKind::Trading212, pages)
    }
}

const STREAMS: [&str; 3] = ["transactions", "orders", "dividends"];

fn stream_endpoint(stream: &str) -> &'static str {
    match stream {
        "orders" => "/api/v0/equity/history/orders",
        "dividends" => "/api/v0/equity/history/dividends",
        _ => "/api/v0/equity/history/transactions",
    }
}

fn item_date(stream: &str, item: &serde_json::Value) -> Option<time::OffsetDateTime> {
    let raw = match stream {
        "orders" => item
            .get("fill")
            .and_then(|f| f.get("filledAt"))
            .or_else(|| item.get("order").and_then(|o| o.get("createdAt"))),
        "dividends" => item.get("paidOn"),
        _ => item.get("dateTime"),
    };
    raw.and_then(|v| v.as_str()).and_then(|s| {
        time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
    })
}

fn next_stream(stream: &str) -> Option<&'static str> {
    let idx = STREAMS.iter().position(|s| *s == stream)?;
    STREAMS.get(idx + 1).copied()
}

fn decode_cursor(cursor: Option<&SyncCursor>) -> (String, Option<String>) {
    let Some(cursor) = cursor else {
        return (STREAMS[0].to_string(), None);
    };
    let value = cursor.as_value();
    if let Some(s) = value.as_str() {
        return (STREAMS[0].to_string(), Some(s.to_string()));
    }
    let stream = value
        .get("stream")
        .and_then(|v| v.as_str())
        .unwrap_or(STREAMS[0])
        .to_string();
    let path = value
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    (stream, path)
}

fn encode_cursor(stream: &str, path: Option<&str>) -> SyncCursor {
    SyncCursor::new(serde_json::json!({ "stream": stream, "path": path }))
}

fn page_covers_from(
    stream: &str,
    items: &[serde_json::Value],
    from: Option<time::OffsetDateTime>,
) -> bool {
    let Some(from) = from else { return false };
    items
        .iter()
        .any(|item| item_date(stream, item).is_some_and(|ts| ts < from))
}

fn parse_portfolio_quantities(body: &serde_json::Value) -> Vec<ProviderAssetBalance> {
    body.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let asset_identifier = item.get("ticker")?.as_str()?.to_string();
                    let quantity = crate::util::parse_decimal(item.get("quantity")?)?;
                    Some(ProviderAssetBalance {
                        asset_identifier,
                        quantity,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_cash_balance(body: &serde_json::Value) -> Vec<ProviderCashBalance> {
    let amount = body
        .get("free")
        .and_then(crate::util::parse_decimal)
        .or_else(|| body.get("total").and_then(crate::util::parse_decimal));

    match amount {
        Some(amount) => {
            let currency = body
                .get("currency")
                .and_then(|v| v.as_str())
                .unwrap_or("GBP")
                .to_string();
            vec![ProviderCashBalance { currency, amount }]
        }
        None => vec![],
    }
}
