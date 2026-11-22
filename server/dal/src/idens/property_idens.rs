use sea_query::Iden;

pub enum PropertyIden {
    Table,
    Id,
    AccountId,
    AssetId,
    Kind,
}

impl Iden for PropertyIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "property",
            Self::Id => "id",
            Self::AccountId => "account_id",
            Self::AssetId => "asset_id",
            Self::Kind => "kind",
        }
    }
}

pub enum PropertyLoanIden {
    Table,
    AccountId,
    PropertyId,
}

impl Iden for PropertyLoanIden {
    fn unquoted(&self) -> &str {
        match self {
            Self::Table => "property_loan",
            Self::AccountId => "account_id",
            Self::PropertyId => "property_id",
        }
    }
}
