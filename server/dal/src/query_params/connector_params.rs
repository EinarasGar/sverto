use sqlx::types::Uuid;

#[derive(Debug)]
pub struct GetConnectorConnectionsParams {
    pub user_id: Uuid,
    pub search_type: GetConnectorConnectionsParamsSearchType,
}

#[derive(Debug)]
pub enum GetConnectorConnectionsParamsSearchType {
    ById(Uuid),
    All,
}

impl GetConnectorConnectionsParams {
    pub fn by_id(user_id: Uuid, id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetConnectorConnectionsParamsSearchType::ById(id),
        }
    }
    pub fn all(user_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetConnectorConnectionsParamsSearchType::All,
        }
    }
}

#[derive(Debug)]
pub struct GetRawPagesParams {
    pub search_type: GetRawPagesParamsSearchType,
    pub after_page_id: Option<Uuid>,
}

#[derive(Debug)]
pub enum GetRawPagesParamsSearchType {
    ByProviderAccountRef(Uuid),
    ByProviderAccountRefs(Vec<Uuid>),
    ByExternalAccount {
        connection_id: Uuid,
        external_account_id: String,
    },
}

impl GetRawPagesParams {
    pub fn by_provider_account_ref(
        provider_account_ref: Uuid,
        after_page_id: Option<Uuid>,
    ) -> Self {
        Self {
            search_type: GetRawPagesParamsSearchType::ByProviderAccountRef(provider_account_ref),
            after_page_id,
        }
    }
    pub fn by_provider_account_refs(provider_account_refs: Vec<Uuid>) -> Self {
        Self {
            search_type: GetRawPagesParamsSearchType::ByProviderAccountRefs(provider_account_refs),
            after_page_id: None,
        }
    }
    pub fn by_external_account(connection_id: Uuid, external_account_id: String) -> Self {
        Self {
            search_type: GetRawPagesParamsSearchType::ByExternalAccount {
                connection_id,
                external_account_id,
            },
            after_page_id: None,
        }
    }
}

#[derive(Debug)]
pub struct GetConnectorBindingsParams {
    pub user_id: Uuid,
    pub search_type: GetConnectorBindingsParamsSearchType,
}

#[derive(Debug)]
pub enum GetConnectorBindingsParamsSearchType {
    ById(Uuid),
    ByIds(Vec<Uuid>),
    ByConnection(Uuid),
    All,
}

impl GetConnectorBindingsParams {
    pub fn by_id(user_id: Uuid, id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetConnectorBindingsParamsSearchType::ById(id),
        }
    }
    pub fn by_ids(user_id: Uuid, ids: Vec<Uuid>) -> Self {
        Self {
            user_id,
            search_type: GetConnectorBindingsParamsSearchType::ByIds(ids),
        }
    }
    pub fn by_connection(user_id: Uuid, connection_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetConnectorBindingsParamsSearchType::ByConnection(connection_id),
        }
    }
    pub fn all(user_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetConnectorBindingsParamsSearchType::All,
        }
    }
}

#[derive(Debug)]
pub struct GetHeldTransactionsParams {
    pub user_id: Uuid,
    pub search_type: GetHeldTransactionsParamsSearchType,
}

#[derive(Debug)]
pub enum GetHeldTransactionsParamsSearchType {
    All,
    ById(Uuid),
    ByExternalIds {
        binding_id: Uuid,
        external_ids: Vec<String>,
    },
}

impl GetHeldTransactionsParams {
    pub fn all(user_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetHeldTransactionsParamsSearchType::All,
        }
    }
    pub fn by_id(user_id: Uuid, id: Uuid) -> Self {
        Self {
            user_id,
            search_type: GetHeldTransactionsParamsSearchType::ById(id),
        }
    }
    pub fn by_external_ids(user_id: Uuid, binding_id: Uuid, external_ids: Vec<String>) -> Self {
        Self {
            user_id,
            search_type: GetHeldTransactionsParamsSearchType::ByExternalIds {
                binding_id,
                external_ids,
            },
        }
    }
}

#[derive(Debug)]
pub struct DeleteHeldTransactionsParams {
    pub user_id: Uuid,
    pub search_type: DeleteHeldTransactionsParamsSearchType,
}

#[derive(Debug)]
pub enum DeleteHeldTransactionsParamsSearchType {
    ById(Uuid),
    ByBinding(Uuid),
}

impl DeleteHeldTransactionsParams {
    pub fn by_id(user_id: Uuid, id: Uuid) -> Self {
        Self {
            user_id,
            search_type: DeleteHeldTransactionsParamsSearchType::ById(id),
        }
    }
    pub fn by_binding(user_id: Uuid, binding_id: Uuid) -> Self {
        Self {
            user_id,
            search_type: DeleteHeldTransactionsParamsSearchType::ByBinding(binding_id),
        }
    }
}

#[derive(Debug)]
pub enum GetConnectorTransactionsParams {
    ByExternalIds {
        binding_id: Uuid,
        external_ids: Vec<String>,
    },
    ByTransactionIds {
        user_id: Uuid,
        transaction_ids: Vec<Uuid>,
    },
}

impl GetConnectorTransactionsParams {
    pub fn by_external_ids(binding_id: Uuid, external_ids: Vec<String>) -> Self {
        Self::ByExternalIds {
            binding_id,
            external_ids,
        }
    }
    pub fn by_transaction_ids(user_id: Uuid, transaction_ids: Vec<Uuid>) -> Self {
        Self::ByTransactionIds {
            user_id,
            transaction_ids,
        }
    }
}
