use time::OffsetDateTime;

use super::create_property::CreatePropertyRequestViewModel;
use crate::errors::FieldError;
use crate::view_models::transactions::validation::Validatable;

impl Validatable for CreatePropertyRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        match &self.purchase {
            Some(purchase) if purchase.date > OffsetDateTime::now_utc() => Err(vec![FieldError {
                field: "purchase.date".to_string(),
                message: "Must not be in the future.".to_string(),
            }]),
            _ => Ok(()),
        }
    }
}
