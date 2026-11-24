#[cfg(feature = "backend")]
use crate::view_models::transactions::value_types::IntoDecimal;
#[cfg(feature = "backend")]
use business::dtos::ai_quick_upload_dto::{
    MergeQuickUploadDto, MergeQuickUploadLineDto, QuickUploadDto,
};
use serde::{Deserialize, Serialize};
use time::{serde::timestamp, OffsetDateTime};
use uuid::Uuid;

use crate::view_models::{
    accounts::base_models::account::IdentifiableAccountViewModel,
    assets::base_models::asset::IdentifiableAssetViewModel,
    categories::base_models::category::IdentifiableCategoryViewModel,
    transactions::{
        base_models::{
            category_id::RequiredCategoryId, description::Description,
            metadata_lookup::MetadataLookupTables,
        },
        transaction_types::TransactionWithId,
        value_types::Amount,
    },
};

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateQuickUploadRequestViewModel {
    pub file_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct QuickUploadLookupTables {
    pub accounts: Vec<IdentifiableAccountViewModel>,
    pub assets: Vec<IdentifiableAssetViewModel>,
    pub categories: Vec<IdentifiableCategoryViewModel>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QuickUploadResponseViewModel {
    pub status: String,
    pub source_file_id: Uuid,
    pub proposal_type: Option<String>,
    pub proposal_data: Option<serde_json::Value>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub lookup_tables: QuickUploadLookupTables,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdentifiableQuickUploadResponseViewModel {
    pub id: Uuid,
    pub status: String,
    pub source_file_id: Uuid,
    pub proposal_type: Option<String>,
    pub proposal_data: Option<serde_json::Value>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QuickUploadMessageRequestViewModel {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QuickUploadMessageResponseViewModel {
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CompleteQuickUploadRequestViewModel {
    pub accepted: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GetQuickUploadMatchesResponseViewModel {
    pub matches: Vec<TransactionWithId>,
    pub lookup_tables: MetadataLookupTables,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MergeQuickUploadRequestViewModel {
    pub transaction_id: Uuid,
    pub description: Option<Description>,
    pub category_id: RequiredCategoryId,
    #[serde(with = "timestamp")]
    #[schema(value_type = i32)]
    pub date: OffsetDateTime,
    pub lines: Vec<MergeQuickUploadLineViewModel>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MergeQuickUploadLineViewModel {
    pub description: Option<Description>,
    pub category_id: RequiredCategoryId,
    pub amount: Amount,
}

#[cfg(feature = "backend")]
impl From<MergeQuickUploadRequestViewModel> for MergeQuickUploadDto {
    fn from(request: MergeQuickUploadRequestViewModel) -> Self {
        Self {
            transaction_id: request.transaction_id,
            description: request.description.map(Description::into_inner),
            category_id: request.category_id.0,
            date: request.date,
            lines: request
                .lines
                .into_iter()
                .map(|line| MergeQuickUploadLineDto {
                    description: line.description.map(Description::into_inner),
                    category_id: line.category_id.0,
                    amount: line.amount.into_decimal(),
                })
                .collect(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<QuickUploadDto> for QuickUploadResponseViewModel {
    fn from(dto: QuickUploadDto) -> Self {
        Self {
            status: dto.status.to_string(),
            source_file_id: dto.source_file_id,
            proposal_type: dto.proposal_type.map(|t| t.to_string()),
            proposal_data: dto.proposal_data,
            created_at: dto.created_at,
            updated_at: dto.updated_at,
            lookup_tables: QuickUploadLookupTables::default(),
        }
    }
}

#[cfg(feature = "backend")]
impl From<QuickUploadDto> for IdentifiableQuickUploadResponseViewModel {
    fn from(dto: QuickUploadDto) -> Self {
        Self {
            id: dto.id,
            status: dto.status.to_string(),
            source_file_id: dto.source_file_id,
            proposal_type: dto.proposal_type.map(|t| t.to_string()),
            proposal_data: dto.proposal_data,
            created_at: dto.created_at,
            updated_at: dto.updated_at,
        }
    }
}
