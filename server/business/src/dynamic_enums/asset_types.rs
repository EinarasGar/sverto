use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use dal::{
    database_connection::MyraDbConnection, database_context::MyraDb,
    enums::asset_types::DatabaseAssetTypes, models::asset_models::AssetTypeModel,
    queries::asset_queries,
};
use once_cell::sync::Lazy;

use crate::loader::{Loader, LoaderError};

use super::DynamicEnum;

static ASSET_TYPES: Lazy<Mutex<Option<HashMap<DatabaseAssetTypes, i32>>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Debug)]
pub struct AssetTypes;

#[async_trait]
impl Loader for AssetTypes {
    #[tracing::instrument(level = "debug", skip_all, fields(otel.kind = "client"))]
    async fn load(&self) -> Result<(), LoaderError> {
        let Ok(connection) = MyraDbConnection::new().await else {
            return Err(LoaderError::DatabaseConnectionError);
        };
        let db = MyraDb::new(connection);
        let Ok(models) = db
            .fetch_all::<AssetTypeModel>(asset_queries::get_asset_types())
            .await
        else {
            return Err(LoaderError::DatabaseQueryError);
        };

        let map = models
            .into_iter()
            .filter_map(|model| {
                DatabaseAssetTypes::from_name(&model.asset_type_name)
                    .map(|asset_type| (asset_type, model.id))
            })
            .collect();
        Self::set_static_map(Some(map));
        Ok(())
    }

    fn get_expected_len(&self) -> usize {
        DatabaseAssetTypes::len()
    }
    fn get_loaded_len(&self) -> usize {
        let map = AssetTypes::get_static_map().lock().unwrap();
        map.as_ref().map_or(0, |m| m.len())
    }
}

impl DynamicEnum<DatabaseAssetTypes, i32> for AssetTypes {
    fn get_static_map() -> &'static Lazy<Mutex<Option<HashMap<DatabaseAssetTypes, i32>>>> {
        &ASSET_TYPES
    }

    fn set_static_map(map: Option<HashMap<DatabaseAssetTypes, i32>>) {
        let mut static_map = ASSET_TYPES.lock().unwrap();
        *static_map = map;
    }
}
