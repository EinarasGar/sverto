use std::fmt;

#[derive(Debug, Clone)]
pub struct BusinessValidationErrorDto {
    pub errors: Vec<BusinessFieldErrorDto>,
}

#[derive(Debug, Clone)]
pub struct BusinessFieldErrorDto {
    pub field: String,
    pub message: String,
}

impl fmt::Display for BusinessValidationErrorDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors: Vec<String> = self
            .errors
            .iter()
            .map(|error| format!("{}: {}", error.field, error.message))
            .collect();
        write!(f, "Validation failed: {}", errors.join("; "))
    }
}

impl std::error::Error for BusinessValidationErrorDto {}
