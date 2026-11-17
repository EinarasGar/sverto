#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SourceError {
    #[error("This kind of file cannot be imported.")]
    Unsupported,
    #[error("This kind of image cannot be read.")]
    ImageFormat,
    #[error("The file is password-protected.")]
    Encrypted,
    #[error("The file is empty.")]
    Empty,
    #[error("The file could not be read: {0}")]
    Unreadable(String),
}
