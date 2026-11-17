use crate::errors::FieldError;

use crate::view_models::imports::requests::{
    AttachImportFilesRequestViewModel, SetImportItemsVerbRequestViewModel,
};
use crate::view_models::imports::staged::ImportItemVerbOverride;
use crate::view_models::transactions::validation::Validatable;

const MAX_ATTACH_FILES: usize = 50;

impl Validatable for AttachImportFilesRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        if self.file_ids.is_empty() {
            return Err(vec![FieldError {
                field: "file_ids".to_string(),
                message: "file_ids must not be empty".to_string(),
            }]);
        }
        if self.file_ids.len() > MAX_ATTACH_FILES {
            return Err(vec![FieldError {
                field: "file_ids".to_string(),
                message: format!("at most {MAX_ATTACH_FILES} files per request"),
            }]);
        }
        Ok(())
    }
}

impl Validatable for SetImportItemsVerbRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        if self.candidate_id.is_some() && self.verb != ImportItemVerbOverride::Skip {
            return Err(vec![FieldError {
                field: "candidate_id".to_string(),
                message: "A suggested transaction can only be chosen with skip.".to_string(),
            }]);
        }
        Ok(())
    }
}
