use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use dal::{
    database_connection::MyraDbConnection, database_context::MyraDb,
    enums::account_types::DatabaseAccountTypes, models::account_models::AccountTypeModel,
    queries::account_queries,
};
use once_cell::sync::Lazy;

use crate::loader::{Loader, LoaderError};

use super::{DynamicEnum, DynamicEnumError};

static ACCOUNT_TYPES: Lazy<Mutex<Option<HashMap<DatabaseAccountTypes, i32>>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Debug)]
pub struct AccountTypes;

#[async_trait]
impl Loader for AccountTypes {
    #[tracing::instrument(level = "debug", skip_all, fields(otel.kind = "client"))]
    async fn load(&self) -> Result<(), LoaderError> {
        let Ok(connection) = MyraDbConnection::new().await else {
            return Err(LoaderError::DatabaseConnectionError);
        };
        let db = MyraDb::new(connection);
        let Ok(models) = db
            .fetch_all::<AccountTypeModel>(account_queries::get_account_types())
            .await
        else {
            return Err(LoaderError::DatabaseQueryError);
        };

        let map = models
            .into_iter()
            .filter_map(|model| {
                DatabaseAccountTypes::from_name(&model.account_type_name)
                    .map(|account_type| (account_type, model.id))
            })
            .collect();
        Self::set_static_map(Some(map));
        Ok(())
    }

    fn get_expected_len(&self) -> usize {
        DatabaseAccountTypes::len()
    }
    fn get_loaded_len(&self) -> usize {
        let map = AccountTypes::get_static_map().lock().unwrap();
        map.as_ref().map_or(0, |m| m.len())
    }
}

impl DynamicEnum<DatabaseAccountTypes, i32> for AccountTypes {
    fn get_static_map() -> &'static Lazy<Mutex<Option<HashMap<DatabaseAccountTypes, i32>>>> {
        &ACCOUNT_TYPES
    }

    fn set_static_map(map: Option<HashMap<DatabaseAccountTypes, i32>>) {
        let mut static_map = ACCOUNT_TYPES.lock().unwrap();
        *static_map = map;
    }
}

impl AccountTypes {
    pub fn liability_ids() -> Result<Vec<i32>, DynamicEnumError> {
        [
            DatabaseAccountTypes::Credit,
            DatabaseAccountTypes::Mortgage,
            DatabaseAccountTypes::Loan,
        ]
        .into_iter()
        .map(Self::try_into_dynamic_enum)
        .collect()
    }
}
