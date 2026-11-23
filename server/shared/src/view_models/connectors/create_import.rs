#[cfg(feature = "backend")]
use business::dtos::connectors::{
    ConnectorImportAccountDto, ConnectorImportFromDto, CreateConnectorImportDto,
};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::ToSchema;
use uuid::Uuid;

time::serde::format_description!(iso_date, Date, "[year]-[month]-[day]");

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateConnectorImportRequestViewModel {
    pub accounts: Vec<ConnectorImportAccountViewModel>,
    pub import_from: ConnectorImportFrom,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ConnectorImportAccountViewModel {
    pub provider_account_id: String,
    pub sverto_account_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectorImportFrom {
    All,
    AfterLastEntry,
    Date {
        #[serde(with = "iso_date")]
        date: Date,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateConnectorImportResponseViewModel {
    pub import_session_id: Uuid,
}

#[cfg(feature = "backend")]
impl From<CreateConnectorImportRequestViewModel> for CreateConnectorImportDto {
    fn from(value: CreateConnectorImportRequestViewModel) -> Self {
        Self {
            accounts: value
                .accounts
                .into_iter()
                .map(|account| ConnectorImportAccountDto {
                    provider_account_id: account.provider_account_id,
                    sverto_account_id: account.sverto_account_id,
                })
                .collect(),
            import_from: value.import_from.into(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<ConnectorImportFrom> for ConnectorImportFromDto {
    fn from(value: ConnectorImportFrom) -> Self {
        match value {
            ConnectorImportFrom::All => Self::All,
            ConnectorImportFrom::AfterLastEntry => Self::AfterLastEntry,
            ConnectorImportFrom::Date { date } => Self::Date(date),
        }
    }
}
