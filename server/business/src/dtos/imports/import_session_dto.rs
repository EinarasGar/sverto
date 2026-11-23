use time::OffsetDateTime;

use super::import_ask_dto::ImportOpenAskDto;
use super::import_classification_dto::ImportClassificationDto;
use super::import_source_dto::ImportSourceDto;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSessionStatusDto {
    Collecting,
    Parsing,
    Reconciling,
    NeedsInput,
    ProposalReady,
    Completed,
}

pub struct ImportSessionDto {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub accounts: Vec<Uuid>,
    pub status: ImportSessionStatusDto,
    pub classification: ImportClassificationDto,
    pub created_at: OffsetDateTime,
}

#[derive(Default)]
pub struct ImportSessionCountsDto {
    pub add: i64,
    pub amend: i64,
    pub delete: i64,
    pub skip: i64,
    pub pending: i64,
    pub unstageable: i64,
    pub staged_available: bool,
}

pub struct ImportSessionDetailDto {
    pub session: ImportSessionDto,
    pub sources: Vec<ImportSourceDto>,
    pub counts: ImportSessionCountsDto,
    pub open_asks: Vec<ImportOpenAskDto>,
}
