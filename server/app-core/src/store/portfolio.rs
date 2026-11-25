use std::sync::Mutex;

use shared::view_models::portfolio::get_networth_history::GetNetWorthHistoryResponseViewModel;

use super::infra::SharedInfra;
use crate::api::holdings::{extract_holdings, PortfolioHoldings};
use crate::models::{ChartPeriodData, ChartPoint, PortfolioState};

const RANGES: &[&str] = &["1d", "1w", "1m", "3m", "6m", "1y", "all"];
const LABELS: &[&str] = &["1D", "1W", "1M", "3M", "6M", "1Y", "ALL"];

#[uniffi::export(callback_interface)]
pub trait PortfolioObserver: Send + Sync {
    fn on_portfolio_changed(&self, state: PortfolioState);
}

pub struct PortfolioModule {
    state: PortfolioState,
    observer: Option<Box<dyn PortfolioObserver>>,
}

impl Default for PortfolioModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PortfolioModule {
    pub fn new() -> Self {
        Self {
            state: PortfolioState {
                is_loading: false,
                error: None,
                holdings: vec![],
                liabilities: vec![],
                holdings_total: 0.0,
                liabilities_total: 0.0,
                chart_data: vec![],
                base_ticker: String::new(),
            },
            observer: None,
        }
    }

    pub fn set_observer(&mut self, observer: Box<dyn PortfolioObserver>) {
        self.observer = Some(observer);
        self.notify();
    }

    pub fn clear_observer(&mut self) {
        self.observer = None;
    }

    pub fn clear_state(&mut self) {
        self.state = PortfolioState {
            is_loading: false,
            error: None,
            holdings: vec![],
            liabilities: vec![],
            holdings_total: 0.0,
            liabilities_total: 0.0,
            chart_data: vec![],
            base_ticker: String::new(),
        };
        self.notify();
    }

    fn set_holdings(&mut self, holdings: PortfolioHoldings) {
        self.state.holdings_total = holdings.holdings.iter().map(|h| h.value).sum();
        self.state.liabilities_total = holdings.liabilities.iter().map(|l| l.value).sum();
        self.state.holdings = holdings.holdings;
        self.state.liabilities = holdings.liabilities;
    }

    fn notify(&self) {
        if let Some(ref obs) = self.observer {
            obs.on_portfolio_changed(self.state.clone());
        }
    }
}

pub async fn load_portfolio(
    infra: &SharedInfra,
    module: &Mutex<PortfolioModule>,
    auth_token: Option<&str>,
) {
    let user_id = match infra.user_id() {
        Some(id) => id,
        None => return,
    };

    let holdings_url = format!(
        "{}/api/users/{user_id}/portfolio/holdings",
        infra.base_url()
    );
    let properties_url = format!("{}/api/users/{user_id}/properties", infra.base_url());
    let cached_properties = infra.persistent_cache.get(&properties_url);
    let cached_holdings: Option<PortfolioHoldings> = infra
        .persistent_cache
        .get(&holdings_url)
        .and_then(|body| extract_holdings(&body, cached_properties.as_deref()).ok());

    let mut cached_chart_data: Vec<ChartPeriodData> = Vec::new();
    for (i, range) in RANGES.iter().enumerate() {
        let chart_url = format!(
            "{}/api/users/{user_id}/portfolio/history?range={range}",
            infra.base_url()
        );
        if let Some(body) = infra.persistent_cache.get(&chart_url) {
            if let Ok(resp) = serde_json::from_str::<GetNetWorthHistoryResponseViewModel>(&body) {
                cached_chart_data.push(ChartPeriodData {
                    period: LABELS[i].to_string(),
                    points: resp
                        .sums
                        .into_iter()
                        .map(|p| ChartPoint {
                            timestamp: p.date,
                            value: p.rate,
                        })
                        .collect(),
                });
            }
        }
    }

    let had_cached = cached_holdings.is_some();
    {
        let mut m = module.lock().unwrap();
        if let Some(holdings) = cached_holdings {
            m.set_holdings(holdings);
        }
        if !cached_chart_data.is_empty() {
            m.state.chart_data = cached_chart_data;
        }
        m.state.is_loading = true;
        m.state.error = None;
        m.state.base_ticker = infra.default_asset_ticker().unwrap_or_default();
        m.notify();
    }

    fetch_fresh(infra, module, &user_id, auth_token, had_cached).await;
}

pub async fn refresh_portfolio(
    infra: &SharedInfra,
    module: &Mutex<PortfolioModule>,
    auth_token: Option<&str>,
) {
    let user_id = match infra.user_id() {
        Some(id) => id,
        None => return,
    };

    infra.evict_memory_cache_prefix(&format!("/api/users/{}/portfolio", user_id));
    infra.evict_memory_cache_prefix(&format!("/api/users/{}/properties", user_id));

    {
        let mut m = module.lock().unwrap();
        m.state.is_loading = true;
        m.state.error = None;
        m.notify();
    }

    fetch_fresh(infra, module, &user_id, auth_token, true).await;
}

async fn fetch_fresh(
    infra: &SharedInfra,
    module: &Mutex<PortfolioModule>,
    user_id: &str,
    auth_token: Option<&str>,
    had_cached: bool,
) {
    let holdings_path = format!("/api/users/{user_id}/portfolio/holdings");
    let properties_path = format!("/api/users/{user_id}/properties");

    let chart_paths: Vec<String> = RANGES
        .iter()
        .map(|range| format!("/api/users/{user_id}/portfolio/history?range={range}"))
        .collect();

    let chart_futures: Vec<_> = chart_paths
        .iter()
        .map(|path| infra.get(path, auth_token))
        .collect();

    let (holdings_result, properties_result, chart_results) = tokio::join!(
        infra.get(&holdings_path, auth_token),
        infra.get(&properties_path, auth_token),
        futures_util::future::join_all(chart_futures),
    );

    let properties_body = properties_result.ok().map(|resp| resp.body);
    let holdings: Option<PortfolioHoldings> = holdings_result
        .ok()
        .and_then(|resp| extract_holdings(&resp.body, properties_body.as_deref()).ok());

    let mut chart_data: Vec<ChartPeriodData> = Vec::new();
    let mut chart_error: Option<String> = None;

    for (i, result) in chart_results.into_iter().enumerate() {
        match result {
            Ok(resp) => {
                match serde_json::from_str::<GetNetWorthHistoryResponseViewModel>(&resp.body) {
                    Ok(parsed) => {
                        chart_data.push(ChartPeriodData {
                            period: LABELS[i].to_string(),
                            points: parsed
                                .sums
                                .into_iter()
                                .map(|p| ChartPoint {
                                    timestamp: p.date,
                                    value: p.rate,
                                })
                                .collect(),
                        });
                    }
                    Err(e) => {
                        chart_error = Some(e.to_string());
                    }
                }
            }
            Err(e) => {
                chart_error = Some(e.to_string());
            }
        }
    }

    let mut m = module.lock().unwrap();
    if let Some(h) = holdings {
        m.set_holdings(h);
        if !chart_data.is_empty() {
            m.state.chart_data = chart_data;
        }
        m.state.error = None;
    } else if had_cached {
        // Keep existing items but set error
        m.state.error = chart_error.or(Some("Failed to load portfolio".to_string()));
    } else {
        m.state.error = Some("Failed to load portfolio".to_string());
    }
    m.state.base_ticker = infra.default_asset_ticker().unwrap_or_default();
    m.state.is_loading = false;
    m.notify();
}
