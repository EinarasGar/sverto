use sea_query::Iden;

pub enum ImportSessionsIden {
    Table,
    Id,
    UserId,
    ConversationId,
    Lifecycle,
    CreatedAt,
}

impl Iden for ImportSessionsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_sessions",
            Self::Id => "id",
            Self::UserId => "user_id",
            Self::ConversationId => "conversation_id",
            Self::Lifecycle => "lifecycle",
            Self::CreatedAt => "created_at",
        }
    }
}

pub enum ImportFilesIden {
    Table,
    SessionId,
    FileId,
    ConversationId,
    Status,
    ErrorMessage,
    PausedUntil,
    CoverageFrom,
    CoverageTo,
    DateOrder,
    CreatedAt,
}

impl Iden for ImportFilesIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_files",
            Self::SessionId => "session_id",
            Self::FileId => "file_id",
            Self::ConversationId => "conversation_id",
            Self::Status => "status",
            Self::ErrorMessage => "error_message",
            Self::PausedUntil => "paused_until",
            Self::CoverageFrom => "coverage_from",
            Self::CoverageTo => "coverage_to",
            Self::DateOrder => "date_order",
            Self::CreatedAt => "created_at",
        }
    }
}

pub enum ImportAccountMappingsIden {
    Table,
    SessionId,
    FileId,
    AccountRef,
    AccountId,
}

impl Iden for ImportAccountMappingsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_account_mappings",
            Self::SessionId => "session_id",
            Self::FileId => "file_id",
            Self::AccountRef => "account_ref",
            Self::AccountId => "account_id",
        }
    }
}

pub enum ImportAssetMappingsIden {
    Table,
    SessionId,
    AssetRef,
    AssetId,
}

impl Iden for ImportAssetMappingsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_asset_mappings",
            Self::SessionId => "session_id",
            Self::AssetRef => "asset_ref",
            Self::AssetId => "asset_id",
        }
    }
}

#[derive(Clone, Copy)]
pub enum ImportParsedLinesIden {
    Table,
    Id,
    SessionId,
    FileId,
    TransactionKey,
    AccountRef,
    DateTransacted,
    Description,
    RelatedAssetRef,
    ExternalId,
}

impl Iden for ImportParsedLinesIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_parsed_lines",
            Self::Id => "id",
            Self::SessionId => "session_id",
            Self::FileId => "file_id",
            Self::TransactionKey => "transaction_key",
            Self::AccountRef => "account_ref",
            Self::DateTransacted => "date_transacted",
            Self::Description => "description",
            Self::RelatedAssetRef => "related_asset_ref",
            Self::ExternalId => "external_id",
        }
    }
}

pub enum ImportStagedDecisionsIden {
    Table,
    Id,
    SessionId,
    AskKey,
    Verb,
    TransactionId,
}

impl Iden for ImportStagedDecisionsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_staged_decisions",
            Self::Id => "id",
            Self::SessionId => "session_id",
            Self::AskKey => "ask_key",
            Self::Verb => "verb",
            Self::TransactionId => "transaction_id",
        }
    }
}

pub enum ImportParsedLegsIden {
    Table,
    Id,
    ParsedLineId,
    AssetRef,
    Quantity,
    FeeType,
}

impl Iden for ImportParsedLegsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_parsed_legs",
            Self::Id => "id",
            Self::ParsedLineId => "parsed_line_id",
            Self::AssetRef => "asset_ref",
            Self::Quantity => "quantity",
            Self::FeeType => "fee_type",
        }
    }
}

pub enum ImportAssertionsIden {
    Table,
    SessionId,
    FileId,
    AccountRef,
    AssetRef,
    AccountId,
    AssetId,
    Kind,
    AsOfDate,
    Quantity,
}

impl Iden for ImportAssertionsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_assertions",
            Self::SessionId => "session_id",
            Self::FileId => "file_id",
            Self::AccountRef => "account_ref",
            Self::AssetRef => "asset_ref",
            Self::AccountId => "account_id",
            Self::AssetId => "asset_id",
            Self::Kind => "kind",
            Self::AsOfDate => "as_of_date",
            Self::Quantity => "quantity",
        }
    }
}

pub enum ImportClassificationRunsIden {
    Table,
    Id,
    SessionId,
    CreatedAt,
    ClassifiedAt,
    ErrorMessage,
}

impl Iden for ImportClassificationRunsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_classification_runs",
            Self::Id => "id",
            Self::SessionId => "session_id",
            Self::CreatedAt => "created_at",
            Self::ClassifiedAt => "classified_at",
            Self::ErrorMessage => "error_message",
        }
    }
}

pub enum ImportLineClassificationsIden {
    Table,
    RunId,
    ParsedLineId,
    TypeId,
    CategoryId,
}

impl Iden for ImportLineClassificationsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_line_classifications",
            Self::RunId => "run_id",
            Self::ParsedLineId => "parsed_line_id",
            Self::TypeId => "type_id",
            Self::CategoryId => "category_id",
        }
    }
}

pub enum ImportClassificationConversationsIden {
    Table,
    RunId,
    ConversationId,
}

impl Iden for ImportClassificationConversationsIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "import_classification_conversations",
            Self::RunId => "run_id",
            Self::ConversationId => "conversation_id",
        }
    }
}
