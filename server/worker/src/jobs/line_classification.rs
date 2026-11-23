use std::sync::Arc;

use ai::config::{AiConfig, ImportClassifier, SystemOneConfig};
use ai::conversation_provider::ConversationSource;
use ai::models::classification::{
    ClassificationCategory, ClassificationLine, ClassificationOutcome, ClassifiedKind,
    LineClassification,
};
use ai::workflows::line_classification::{llm_classify, system_one_classify};
use business::dtos::categories::category_dto::CategoryDto;
use business::dtos::imports::import_classification_dto::ImportLineTypeDto;
use business::providers::user_rate_limiter::UserRateLimiter;
use business::rate_limiting::rate_limiter::RateLimiter;
use business::service_collection::ServiceProviders;
use uuid::Uuid;

pub(crate) async fn classify<S: ConversationSource>(
    providers: &ServiceProviders,
    user_id: Uuid,
    conversations: &S,
    lines: &[ClassificationLine],
    categories: &[ClassificationCategory],
) -> anyhow::Result<ClassificationOutcome> {
    match ImportClassifier::try_from_env()? {
        ImportClassifier::Llm => {
            let rate_limit = Arc::new(
                UserRateLimiter::new(
                    RateLimiter::new(providers.redis.clone(), providers.db.clone()),
                    user_id,
                )
                .waiting_for_slot(),
            );
            llm_classify(
                &AiConfig::try_from_env()?,
                conversations,
                rate_limit,
                lines,
                categories,
            )
            .await
        }
        ImportClassifier::SystemOne => {
            system_one_classify(
                &SystemOneConfig::try_from_env()?,
                conversations,
                lines,
                categories,
            )
            .await
        }
    }
}

pub(crate) fn offered_categories(categories: &[CategoryDto]) -> Vec<ClassificationCategory> {
    categories
        .iter()
        .filter(|category| !category.is_system)
        .map(|category| ClassificationCategory {
            id: category.id,
            type_name: category.category_type_name.clone(),
            name: category.category.clone(),
        })
        .collect()
}

pub(crate) fn typing(
    classification: &LineClassification,
    money_in: bool,
) -> (ImportLineTypeDto, Option<i32>) {
    let line_type = type_of(classification.kind, money_in);
    let category_id = classification
        .category_id
        .filter(|_| classification.confident && line_type == ImportLineTypeDto::Regular);
    (line_type, category_id)
}

fn type_of(kind: ClassifiedKind, money_in: bool) -> ImportLineTypeDto {
    match (kind, money_in) {
        (ClassifiedKind::Expense | ClassifiedKind::Refund, _) => ImportLineTypeDto::Regular,
        (ClassifiedKind::BankFee, true) => ImportLineTypeDto::Regular,
        (ClassifiedKind::BankFee | ClassifiedKind::Interest, false) => {
            ImportLineTypeDto::AccountFees
        }
        (
            ClassifiedKind::TransferIn | ClassifiedKind::TransferOut | ClassifiedKind::Interest,
            true,
        ) => ImportLineTypeDto::CashTransferIn,
        (ClassifiedKind::TransferIn | ClassifiedKind::TransferOut, false) => {
            ImportLineTypeDto::CashTransferOut
        }
        (ClassifiedKind::CurrencyExchange, _) => ImportLineTypeDto::AssetTrade,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROCERIES: i32 = 25;

    fn classified(kind: ClassifiedKind, confident: bool) -> LineClassification {
        LineClassification {
            kind,
            category_id: Some(GROCERIES),
            confidence: if confident { 1.0 } else { 0.3 },
            confident,
        }
    }

    #[test]
    fn only_a_confident_payment_keeps_its_category() {
        assert_eq!(
            typing(&classified(ClassifiedKind::Expense, true), false),
            (ImportLineTypeDto::Regular, Some(GROCERIES))
        );
        assert_eq!(
            typing(&classified(ClassifiedKind::BankFee, true), true),
            (ImportLineTypeDto::Regular, Some(GROCERIES))
        );
        assert_eq!(
            typing(&classified(ClassifiedKind::Expense, false), false),
            (ImportLineTypeDto::Regular, None)
        );
        assert_eq!(
            typing(&classified(ClassifiedKind::TransferIn, true), true),
            (ImportLineTypeDto::CashTransferIn, None)
        );
        assert_eq!(
            typing(&classified(ClassifiedKind::BankFee, true), false),
            (ImportLineTypeDto::AccountFees, None)
        );
    }
}
