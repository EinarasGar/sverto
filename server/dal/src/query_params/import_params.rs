use sqlx::types::Uuid;

pub struct GetImportSessionsParams {
    pub user_id: Uuid,
    pub search_type: GetImportSessionsSearchType,
}

pub enum GetImportSessionsSearchType {
    ById(Uuid),
    All,
}

impl GetImportSessionsParams {
    pub fn by_id(session_id: Uuid, user_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetImportSessionsSearchType::ById(session_id),
        }
    }

    pub fn all(user_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetImportSessionsSearchType::All,
        }
    }
}

pub struct GetImportFilesParams {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub search_type: GetImportFilesSearchType,
}

pub enum GetImportFilesSearchType {
    ById(Uuid),
    All,
}

impl GetImportFilesParams {
    pub fn by_id(session_id: Uuid, file_id: Uuid, user_id: Uuid) -> Self {
        Self {
            session_id,
            user_id,
            search_type: GetImportFilesSearchType::ById(file_id),
        }
    }

    pub fn all(session_id: Uuid, user_id: Uuid) -> Self {
        Self {
            session_id,
            user_id,
            search_type: GetImportFilesSearchType::All,
        }
    }
}

pub enum ClassificationRunOutcome {
    Classified,
    Failed(String),
}
