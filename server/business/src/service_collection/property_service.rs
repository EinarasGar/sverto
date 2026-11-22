#[mockall_double::double]
use dal::database_context::MyraDb;
use dal::models::property_models::{InsertPropertyModel, PropertyModel};
use dal::queries::property_queries;
use dal::query_params::get_properties_params::GetPropertiesParams;
use itertools::Itertools;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use super::accounts_service::AccountsService;
use super::asset_rates_service::AssetRatesService;
use super::asset_service::AssetsService;
use super::transaction_management_service::TransactionManagementService;
use crate::dtos::accounts::account_amendment_dto::AccountAmendmentDto;
use crate::dtos::add_custom_asset_dto::AddCustomAssetDto;
use crate::dtos::asset_pair_rate_insert_dto::AssetPairRateInsertDto;
use crate::dtos::assets::update_asset_dto::UpdateAssetDto;
use crate::dtos::conflict_error_dto::BusinessConflictError;
use crate::dtos::entry_dto::EntryDto;
use crate::dtos::not_found_error_dto::BusinessNotFoundError;
use crate::dtos::properties::create_property_dto::CreatePropertyDto;
use crate::dtos::properties::property_dto::PropertyDto;
use crate::dtos::properties::update_property_dto::UpdatePropertyDto;
use crate::dtos::transaction_dto::{
    AssetTransferInMetadataDto, TransactionDto, TransactionTypeDto, TransactionVisibilityDto,
};
use crate::dtos::validation_error_dto::{BusinessFieldErrorDto, BusinessValidationErrorDto};

const LIQUID: i32 = 1;

pub struct PropertyService {
    db: MyraDb,
    assets_service: AssetsService,
    accounts_service: AccountsService,
    transaction_service: TransactionManagementService,
    asset_rates_service: AssetRatesService,
}

impl PropertyService {
    pub fn new(providers: &super::ServiceProviders) -> Self {
        Self {
            assets_service: AssetsService::new(providers),
            accounts_service: AccountsService::new(providers),
            transaction_service: TransactionManagementService::new(providers),
            asset_rates_service: AssetRatesService::new(providers),
            db: providers.db.clone(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn get_properties(&self, user_id: Uuid) -> anyhow::Result<Vec<PropertyDto>> {
        let models = self
            .db
            .fetch_all::<PropertyModel>(property_queries::get_properties(GetPropertiesParams::all(
                user_id,
            )))
            .await?;
        models.into_iter().map(PropertyDto::try_from).collect()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, property_id = %property_id))]
    pub async fn get_property(
        &self,
        user_id: Uuid,
        property_id: Uuid,
    ) -> anyhow::Result<PropertyDto> {
        let model = self
            .db
            .fetch_optional::<PropertyModel>(property_queries::get_properties(
                GetPropertiesParams::by_id(user_id, property_id),
            ))
            .await?
            .ok_or_else(|| BusinessNotFoundError {
                message: "Property not found.".to_string(),
            })?;
        model.try_into()
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id))]
    pub async fn create_property(
        &self,
        user_id: Uuid,
        dto: CreatePropertyDto,
    ) -> anyhow::Result<PropertyDto> {
        if !self
            .assets_service
            .is_visible_currency(user_id, dto.currency_asset_id)
            .await?
        {
            return Err(BusinessValidationErrorDto {
                errors: vec![BusinessFieldErrorDto {
                    field: "currency_asset_id".to_string(),
                    message: "Must be a currency.".to_string(),
                }],
            }
            .into());
        }

        let now = OffsetDateTime::now_utc();
        // Postgres keeps microseconds; the binary bind truncates and the COPY text rounds, so
        // an unaligned `now` can put the first rate after the transfer it must price.
        let now = now
            .replace_nanosecond(now.nanosecond() / 1_000 * 1_000)
            .expect("a whole number of microseconds is a valid nanosecond");
        let acquired_at = dto.purchase.as_ref().map_or(now, |purchase| purchase.date);
        let loan_account_ids: Vec<Uuid> = dto.loan_account_ids.into_iter().unique().collect();

        self.db.start_transaction().await?;
        let asset = self
            .assets_service
            .add_custom_asset_inner(AddCustomAssetDto {
                ticker: format!(
                    "{}-{}",
                    dto.kind.ticker_prefix(),
                    &Uuid::new_v4().simple().to_string()[..8].to_uppercase()
                ),
                name: dto.name.clone(),
                asset_type: dto.kind.asset_type_id(),
                base_pair_id: dto.currency_asset_id,
                user_id,
            })
            .await?;
        let account_id = self
            .accounts_service
            .add_user_account_inner(
                user_id,
                AccountAmendmentDto {
                    account_type: dto.kind.account_type_id(),
                    account_name: dto.name.clone(),
                    account_liquidity_type: LIQUID,
                    ownership_share: dto.ownership_share,
                    identifiers: vec![],
                },
            )
            .await?;
        self.transaction_service
            .add_individual_transaction_inner(
                user_id,
                TransactionDto {
                    transaction_id: None,
                    date: acquired_at,
                    visibility: TransactionVisibilityDto::Default,
                    fee_entries: vec![],
                    transaction_type: TransactionTypeDto::AssetTransferIn(
                        AssetTransferInMetadataDto {
                            entry: EntryDto {
                                entry_id: None,
                                asset_id: asset.new_asset_id,
                                quantity: Decimal::ONE,
                                account_id,
                            },
                        },
                    ),
                },
            )
            .await?;
        let property_id: Uuid = self
            .db
            .fetch_one_scalar(property_queries::insert_property(InsertPropertyModel {
                account_id,
                asset_id: asset.new_asset_id,
                kind: dto.kind.into(),
            }))
            .await?;
        self.link_loans(user_id, property_id, loan_account_ids.clone())
            .await?;
        self.db.commit_transaction().await?;

        let pair_id = asset
            .new_asset_pair_id
            .expect("a custom asset is created with its base pair");
        let mut rates: Vec<AssetPairRateInsertDto> = dto
            .purchase
            .map(|purchase| AssetPairRateInsertDto {
                pair_id,
                rate: purchase.price,
                date: purchase.date,
            })
            .into_iter()
            .collect();
        rates.push(AssetPairRateInsertDto {
            pair_id,
            rate: dto.value,
            date: now,
        });
        self.asset_rates_service.insert_pair_many(rates).await?;

        Ok(PropertyDto {
            id: property_id,
            kind: dto.kind,
            name: dto.name,
            account_id,
            asset_id: asset.new_asset_id,
            currency_asset_id: dto.currency_asset_id,
            ownership_share: dto.ownership_share,
            loan_account_ids,
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, property_id = %property_id))]
    pub async fn update_property(
        &self,
        user_id: Uuid,
        property_id: Uuid,
        dto: UpdatePropertyDto,
    ) -> anyhow::Result<()> {
        let property = self.get_property(user_id, property_id).await?;
        let account = self
            .accounts_service
            .get_account_with_metadata(user_id, property.account_id)
            .await?;
        let asset = self.assets_service.get_asset(property.asset_id).await?;

        self.db.start_transaction().await?;
        self.accounts_service
            .update_user_account_inner(
                user_id,
                property.account_id,
                AccountAmendmentDto {
                    account_type: account.account_type.id,
                    account_name: dto.name.clone(),
                    account_liquidity_type: account.liquidity_type.id,
                    ownership_share: dto.ownership_share,
                    identifiers: account.identifiers,
                },
            )
            .await?;
        self.assets_service
            .update_asset(UpdateAssetDto {
                asset_id: property.asset_id,
                ticker: asset.ticker,
                name: dto.name,
                asset_type: property.kind.asset_type_id(),
                base_pair_id: property.currency_asset_id,
                user_id,
            })
            .await?;
        self.db.commit_transaction().await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, property_id = %property_id, account_id = %account_id))]
    pub async fn link_loan(
        &self,
        user_id: Uuid,
        property_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<()> {
        self.link_loans(user_id, property_id, vec![account_id])
            .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(user_id = %user_id, property_id = %property_id, account_id = %account_id))]
    pub async fn unlink_loan(
        &self,
        user_id: Uuid,
        property_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<()> {
        let removed = self
            .db
            .execute_with_rows_affected(property_queries::delete_property_loan(
                user_id,
                property_id,
                account_id,
            ))
            .await?;
        if removed == 0 {
            return Err(BusinessNotFoundError {
                message: "Linked loan not found.".to_string(),
            }
            .into());
        }
        Ok(())
    }

    async fn link_loans(
        &self,
        user_id: Uuid,
        property_id: Uuid,
        account_ids: Vec<Uuid>,
    ) -> anyhow::Result<()> {
        if account_ids.is_empty() {
            return Ok(());
        }
        let requested = account_ids.len() as u64;
        let linked = self
            .db
            .execute_with_rows_affected(property_queries::insert_property_loans(
                user_id,
                property_id,
                account_ids,
            ))
            .await
            .map_err(|e| {
                if e.as_database_error()
                    .is_some_and(|d| d.is_unique_violation())
                {
                    anyhow::Error::new(BusinessConflictError {
                        message: "This loan is already linked to a property.".to_string(),
                    })
                } else {
                    anyhow::Error::new(e)
                }
            })?;
        if linked != requested {
            return Err(BusinessNotFoundError {
                message: "Loan account not found.".to_string(),
            }
            .into());
        }
        Ok(())
    }
}
