use std::sync::Mutex;

use super::infra::SharedInfra;
use crate::api::account_transactions::{extract_account_transactions, scoped_transactions_path};
use crate::models::{ScopedTransactionsState, TransactionListScope};

const PAGE_SIZE: u32 = 20;

#[uniffi::export(callback_interface)]
pub trait ScopedTransactionsObserver: Send + Sync {
    fn on_scoped_transactions_changed(&self, state: ScopedTransactionsState);
}

pub struct ScopedTransactionsModule {
    state: ScopedTransactionsState,
    observer: Option<Box<dyn ScopedTransactionsObserver>>,
    scope: Option<TransactionListScope>,
    next_start: u32,
}

impl Default for ScopedTransactionsModule {
    fn default() -> Self {
        Self::new()
    }
}

fn empty_state() -> ScopedTransactionsState {
    ScopedTransactionsState {
        is_loading: false,
        is_loading_more: false,
        error: None,
        items: vec![],
        has_more: false,
    }
}

impl ScopedTransactionsModule {
    pub fn new() -> Self {
        Self {
            state: empty_state(),
            observer: None,
            scope: None,
            next_start: 0,
        }
    }

    pub fn set_observer(&mut self, observer: Box<dyn ScopedTransactionsObserver>) {
        self.observer = Some(observer);
        self.notify();
    }

    pub fn clear_observer(&mut self) {
        self.observer = None;
        self.state = empty_state();
        self.scope = None;
        self.next_start = 0;
    }

    pub fn clear_state(&mut self) {
        self.state = empty_state();
        self.scope = None;
        self.next_start = 0;
        self.notify();
    }

    fn notify(&self) {
        if let Some(ref obs) = self.observer {
            obs.on_scoped_transactions_changed(self.state.clone());
        }
    }
}

pub async fn load_scoped_transactions(
    infra: &SharedInfra,
    module: &Mutex<ScopedTransactionsModule>,
    scope: TransactionListScope,
    auth_token: Option<&str>,
) {
    let Some(user_id) = infra.user_id() else {
        return;
    };
    let base = scoped_transactions_path(&user_id, &scope);

    {
        let mut m = module.lock().unwrap();
        m.scope = Some(scope.clone());
        m.next_start = 0;
        m.state.is_loading = true;
        m.state.items = vec![];
        m.notify();
    }

    let path = format!("{base}?count={PAGE_SIZE}&start=0");
    let result = match infra.get(&path, auth_token).await {
        Ok(resp) => extract_account_transactions(&resp.body),
        Err(e) => Err(e.to_string()),
    };

    let mut m = module.lock().unwrap();
    if m.scope.as_ref() != Some(&scope) {
        return;
    }
    match result {
        Ok(page) => {
            m.state.items = page.items;
            m.state.has_more = page.has_more;
            m.next_start = PAGE_SIZE;
            m.state.error = None;
        }
        Err(e) => m.state.error = Some(e),
    }
    m.state.is_loading = false;
    m.notify();
}

pub async fn load_more_scoped_transactions(
    infra: &SharedInfra,
    module: &Mutex<ScopedTransactionsModule>,
    auth_token: Option<&str>,
) {
    let (user_id, scope, start) = {
        let m = module.lock().unwrap();
        if !m.state.has_more || m.state.is_loading_more {
            return;
        }
        let Some(id) = infra.user_id() else {
            return;
        };
        let Some(scope) = m.scope.clone() else {
            return;
        };
        (id, scope, m.next_start)
    };

    {
        let mut m = module.lock().unwrap();
        m.state.is_loading_more = true;
        m.notify();
    }

    let path = format!(
        "{}?count={PAGE_SIZE}&start={start}",
        scoped_transactions_path(&user_id, &scope)
    );
    let result = match infra.get(&path, auth_token).await {
        Ok(resp) => extract_account_transactions(&resp.body),
        Err(e) => Err(e.to_string()),
    };

    let mut m = module.lock().unwrap();
    if m.scope.as_ref() != Some(&scope) {
        return;
    }
    match result {
        Ok(page) => {
            m.state.items.extend(page.items);
            m.state.has_more = page.has_more;
            m.next_start = start + PAGE_SIZE;
        }
        Err(e) => m.state.error = Some(e),
    }
    m.state.is_loading_more = false;
    m.notify();
}

pub async fn refresh_scoped_transactions(
    infra: &SharedInfra,
    module: &Mutex<ScopedTransactionsModule>,
    auth_token: Option<&str>,
) {
    let Some(scope) = module.lock().unwrap().scope.clone() else {
        return;
    };
    let Some(user_id) = infra.user_id() else {
        return;
    };

    infra.evict_memory_cache_prefix(&scoped_transactions_path(&user_id, &scope));

    load_scoped_transactions(infra, module, scope, auth_token).await;
}
