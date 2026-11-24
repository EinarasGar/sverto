use crate::errors::FieldError;
use crate::view_models::ai::conversations::SendMessageRequestViewModel;
use crate::view_models::ai::quick_upload::MergeQuickUploadRequestViewModel;
use crate::view_models::transactions::validation::Validatable;

impl Validatable for SendMessageRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        let has_message = self
            .message
            .as_ref()
            .map(|m| !m.is_empty())
            .unwrap_or(false);
        let has_files = !self.file_ids.is_empty();
        let has_approval = !self.tool_approvals.is_empty();

        if !has_message && !has_files && !has_approval {
            return Err(vec![FieldError {
                field: "message".to_string(),
                message: "Either message, file_ids, or tool_approvals must be provided."
                    .to_string(),
            }]);
        }

        Ok(())
    }
}

impl Validatable for MergeQuickUploadRequestViewModel {
    fn validate(&self) -> Result<(), Vec<FieldError>> {
        if self.lines.is_empty() {
            return Err(vec![FieldError {
                field: "lines".to_string(),
                message: "A receipt needs at least one line.".to_string(),
            }]);
        }
        if self.lines.len() > 1 && self.description.is_none() {
            return Err(vec![FieldError {
                field: "description".to_string(),
                message: "A receipt with several lines needs a description.".to_string(),
            }]);
        }
        Ok(())
    }
}
