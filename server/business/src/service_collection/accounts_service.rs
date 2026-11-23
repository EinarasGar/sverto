use std::collections::{HashMap, HashSet};

#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::{
    models::account_models::{
        Account, AccountCreationModel, AccountIdentifierInsert, AccountIdentifierRow,
        AccountLiquidityTypeModel, AccountTypeModel, AccountUpdateModel, AccountWithMetadata,
    },
    queries::{account_identifier_queries, account_queries},
    query_params::get_accounts_params::GetAccountsParams,
};
use itertools::Itertools;
use mockall::automock;
use uuid::Uuid;

use crate::dtos::accounts::{
    account_amendment_dto::AccountAmendmentDto,
    account_dto::AccountDto,
    account_identifier_dto::{AccountIdentifierDto, AccountIdentifierKind},
    account_liquidity_type_dto::AccountLiquidityTypeDto,
    account_type_dto::AccountTypeDto,
    full_account_dto::FullAccountDto,
};

pub struct AccountsService {
    db: MyraDb,
}

#[automock]
impl AccountsService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            db: providers.db.clone(),
        }
    }

    async fn check_identifier_conflicts(
        &self,
        user_id: Uuid,
        exclude_account_id: Option<Uuid>,
        identifiers: &[AccountIdentifierDto],
    ) -> anyhow::Result<()> {
        if identifiers.is_empty() {
            return Ok(());
        }
        let values: Vec<String> = identifiers.iter().map(|i| i.value.clone()).collect();
        let query = account_identifier_queries::find_identifiers_by_value(
            user_id,
            exclude_account_id,
            values,
        );
        let rows = self.db.fetch_all::<AccountIdentifierRow>(query).await?;
        let existing: HashSet<(&str, &str)> = rows
            .iter()
            .map(|r| (r.kind.as_str(), r.value.as_str()))
            .collect();
        if let Some(conflict) = identifiers
            .iter()
            .find(|i| existing.contains(&(i.kind.as_str(), i.value.as_str())))
        {
            return Err(crate::dtos::conflict_error_dto::BusinessConflictError {
                message: format!("{} is already linked to another account.", conflict.value),
            }
            .into());
        }
        Ok(())
    }

    async fn insert_identifiers(
        &self,
        user_id: Uuid,
        identifiers: Vec<(Uuid, AccountIdentifierDto)>,
    ) -> anyhow::Result<()> {
        if identifiers.is_empty() {
            return Ok(());
        }
        let rows = identifiers
            .into_iter()
            .map(|(account_id, identifier)| AccountIdentifierInsert {
                account_id,
                kind: identifier.kind.as_str().to_string(),
                value: identifier.value,
            })
            .collect();
        let query = account_identifier_queries::insert_account_identifiers(user_id, rows);
        self.db.execute(query).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn accounts_by_iban(
        &self,
        user_id: Uuid,
        ibans: Vec<String>,
    ) -> anyhow::Result<HashMap<String, Uuid>> {
        let query = account_identifier_queries::find_identifiers_by_value(user_id, None, ibans);
        Ok(self
            .db
            .fetch_all::<AccountIdentifierRow>(query)
            .await?
            .into_iter()
            .filter(|row| {
                AccountIdentifierKind::from_db_str(&row.kind) == Some(AccountIdentifierKind::Iban)
            })
            .map(|row| (row.value, row.account_id))
            .collect())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn add_ibans(&self, user_id: Uuid, ibans: Vec<(Uuid, String)>) -> anyhow::Result<()> {
        self.insert_identifiers(
            user_id,
            ibans
                .into_iter()
                .map(|(account_id, value)| {
                    (
                        account_id,
                        AccountIdentifierDto {
                            kind: AccountIdentifierKind::Iban,
                            value,
                        },
                    )
                })
                .collect(),
        )
        .await
    }

    async fn load_identifiers(
        &self,
        account_ids: Vec<Uuid>,
    ) -> anyhow::Result<HashMap<Uuid, Vec<AccountIdentifierDto>>> {
        if account_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let query = account_identifier_queries::get_identifiers_for_accounts(account_ids);
        let rows = self.db.fetch_all::<AccountIdentifierRow>(query).await?;
        let mut map: HashMap<Uuid, Vec<AccountIdentifierDto>> = HashMap::new();
        for row in rows {
            if let Some(kind) = AccountIdentifierKind::from_db_str(&row.kind) {
                map.entry(row.account_id)
                    .or_default()
                    .push(AccountIdentifierDto {
                        kind,
                        value: row.value,
                    });
            }
        }
        Ok(map)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, account_id = %id))]
    pub async fn get_account_with_metadata(
        &self,
        user_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<FullAccountDto> {
        let query =
            account_queries::get_accounts(GetAccountsParams::by_id_with_metadata(user_id, id));
        let model = self.db.fetch_optional::<AccountWithMetadata>(query).await?;

        let Some(model) = model else {
            return Err(anyhow::anyhow!("Account not found"));
        };
        let mut dto: FullAccountDto = model.into();
        let mut map = self.load_identifiers(vec![dto.id]).await?;
        dto.identifiers = map.remove(&dto.id).unwrap_or_default();
        Ok(dto)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn get_accounts(&self, id: HashSet<Uuid>) -> anyhow::Result<Vec<AccountDto>> {
        let query = account_queries::get_accounts(GetAccountsParams::by_ids(id));
        let model = self.db.fetch_all::<Account>(query).await?;
        let ret = model.into_iter().map_into().collect();
        Ok(ret)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn get_user_accounts_with_metadata(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<FullAccountDto>> {
        let query =
            account_queries::get_accounts(GetAccountsParams::by_user_id_with_metadata(user_id));
        let models = self.db.fetch_all::<AccountWithMetadata>(query).await?;
        let mut dtos: Vec<FullAccountDto> = models.into_iter().map_into().collect();
        let ids: Vec<Uuid> = dtos.iter().map(|d| d.id).collect();
        let mut map = self.load_identifiers(ids).await?;
        for dto in &mut dtos {
            dto.identifiers = map.remove(&dto.id).unwrap_or_default();
        }
        Ok(dtos)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, account_id = %account_id))]
    pub async fn update_user_account(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        amendment: AccountAmendmentDto,
    ) -> anyhow::Result<()> {
        self.db.start_transaction().await?;
        self.update_user_account_inner(user_id, account_id, amendment)
            .await?;
        self.db.commit_transaction().await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn add_user_account(
        &self,
        user_id: Uuid,
        amendment: AccountAmendmentDto,
    ) -> anyhow::Result<Uuid> {
        self.db.start_transaction().await?;
        let new_id = self.add_user_account_inner(user_id, amendment).await?;
        self.db.commit_transaction().await?;
        Ok(new_id)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, account_id = %account_id))]
    pub async fn deactivate_user_account(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<()> {
        self.db.start_transaction().await?;
        let rows = self
            .db
            .execute_with_rows_affected(account_queries::deactivate_account(user_id, account_id))
            .await?;
        if rows == 0 {
            return Err(crate::dtos::not_found_error_dto::BusinessNotFoundError {
                message: "Account not found.".to_string(),
            }
            .into());
        }
        self.db
            .execute(account_identifier_queries::delete_account_identifiers(
                account_id,
            ))
            .await?;
        self.db.commit_transaction().await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn get_account_types(&self) -> anyhow::Result<Vec<AccountTypeDto>> {
        let query = account_queries::get_account_types();
        let models = self.db.fetch_all::<AccountTypeModel>(query).await?;
        let return_models = models.into_iter().map_into().collect();
        Ok(return_models)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn get_account_liquidity_types(
        &self,
    ) -> anyhow::Result<Vec<AccountLiquidityTypeDto>> {
        let query = account_queries::get_account_liquidity_types();
        let models = self
            .db
            .fetch_all::<AccountLiquidityTypeModel>(query)
            .await?;
        let return_models = models.into_iter().map_into().collect();
        Ok(return_models)
    }
}

impl AccountsService {
    pub(crate) async fn update_user_account_inner(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        amendment: AccountAmendmentDto,
    ) -> anyhow::Result<()> {
        self.check_identifier_conflicts(user_id, Some(account_id), &amendment.identifiers)
            .await?;

        let account_name = amendment.account_name.clone();
        let model = AccountUpdateModel {
            account_id,
            user_id,
            account_name: amendment.account_name,
            account_type: amendment.account_type,
            liquidity_type: amendment.account_liquidity_type,
            ownership_share: amendment.ownership_share,
        };

        let rows = self
            .db
            .execute_with_rows_affected(account_queries::update_account(model))
            .await
            .map_err(|e| {
                if e.as_database_error()
                    .is_some_and(|d| d.is_unique_violation())
                {
                    anyhow::Error::new(crate::dtos::conflict_error_dto::BusinessConflictError {
                        message: format!("An account named {} already exists.", account_name),
                    })
                } else {
                    anyhow::Error::new(e)
                }
            })?;
        if rows == 0 {
            return Err(crate::dtos::not_found_error_dto::BusinessNotFoundError {
                message: "Account not found.".to_string(),
            }
            .into());
        }
        self.db
            .execute(account_identifier_queries::delete_account_identifiers(
                account_id,
            ))
            .await?;
        self.insert_identifiers(
            user_id,
            amendment
                .identifiers
                .into_iter()
                .map(|identifier| (account_id, identifier))
                .collect(),
        )
        .await
    }

    pub(crate) async fn add_user_account_inner(
        &self,
        user_id: Uuid,
        amendment: AccountAmendmentDto,
    ) -> anyhow::Result<Uuid> {
        self.check_identifier_conflicts(user_id, None, &amendment.identifiers)
            .await?;

        let model = AccountCreationModel {
            user_id,
            account_name: amendment.account_name.clone(),
            account_type: amendment.account_type,
            liquidity_type: amendment.account_liquidity_type,
            ownership_share: amendment.ownership_share,
        };

        let new_id: Uuid = self
            .db
            .fetch_one_scalar(account_queries::insert_account(model))
            .await
            .map_err(|e| {
                if e.as_database_error()
                    .is_some_and(|d| d.is_unique_violation())
                {
                    anyhow::Error::new(crate::dtos::conflict_error_dto::BusinessConflictError {
                        message: format!(
                            "An account named {} already exists.",
                            amendment.account_name
                        ),
                    })
                } else {
                    anyhow::Error::new(e)
                }
            })?;
        self.insert_identifiers(
            user_id,
            amendment
                .identifiers
                .into_iter()
                .map(|identifier| (new_id, identifier))
                .collect(),
        )
        .await?;
        Ok(new_id)
    }
}
