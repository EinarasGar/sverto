#[derive(Debug, thiserror::Error)]
#[error("AI features are disabled for this account")]
pub struct BusinessAiFeaturesDisabledError;
