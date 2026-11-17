use dal::models::import_models::{ImportDateOrder, ImportFileModel, ImportFileStatus};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFileStatusDto {
    Pending,
    Processing,
    Parsed,
    Failed,
}

impl From<ImportFileStatus> for ImportFileStatusDto {
    fn from(value: ImportFileStatus) -> Self {
        match value {
            ImportFileStatus::Pending => Self::Pending,
            ImportFileStatus::Processing => Self::Processing,
            ImportFileStatus::Parsed => Self::Parsed,
            ImportFileStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDateOrderDto {
    DayFirst,
    MonthFirst,
}

impl From<ImportDateOrder> for ImportDateOrderDto {
    fn from(value: ImportDateOrder) -> Self {
        match value {
            ImportDateOrder::DayFirst => Self::DayFirst,
            ImportDateOrder::MonthFirst => Self::MonthFirst,
        }
    }
}

pub struct ImportFileDto {
    pub file_id: Uuid,
    pub status: ImportFileStatusDto,
    pub error_message: Option<String>,
    pub paused_until: Option<OffsetDateTime>,
    pub date_order: Option<ImportDateOrderDto>,
    pub created_at: OffsetDateTime,
    pub recorded_transactions: i64,
}

pub struct ImportProcessingFileDto {
    pub conversation_id: Uuid,
    pub date_order: Option<ImportDateOrderDto>,
}

impl ImportFileDto {
    pub fn new(model: ImportFileModel, recorded_transactions: i64) -> anyhow::Result<Self> {
        Ok(Self {
            status: model.file_status()?.into(),
            date_order: model.file_date_order()?.map(Into::into),
            file_id: model.file_id,
            error_message: model.error_message,
            paused_until: model.paused_until,
            created_at: model.created_at,
            recorded_transactions,
        })
    }
}
