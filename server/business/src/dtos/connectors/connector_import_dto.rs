use time::Date;
use uuid::Uuid;

pub struct CreateConnectorImportDto {
    pub accounts: Vec<ConnectorImportAccountDto>,
    pub import_from: ConnectorImportFromDto,
}

pub struct ConnectorImportAccountDto {
    pub provider_account_id: String,
    pub sverto_account_id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorImportFromDto {
    All,
    AfterLastEntry,
    Date(Date),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStartDto {
    From(Date),
    AfterLastEntry(Date),
}

impl HistoryStartDto {
    pub fn day(self) -> Date {
        match self {
            Self::From(day) | Self::AfterLastEntry(day) => day,
        }
    }
}
