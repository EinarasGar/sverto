use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Delimited,
    Spreadsheet,
    Pdf,
    Text,
    Image,
}

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Delimited => "delimited",
            Self::Spreadsheet => "spreadsheet",
            Self::Pdf => "pdf",
            Self::Text => "text",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceLocation {
    Lines { first: u32, last: u32 },
    Row { sheet: String, row: u32 },
    Page { page: u32, line: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRecord {
    pub location: SourceLocation,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
}

impl SourceRecord {
    pub fn page(&self) -> Option<u32> {
        match self.location {
            SourceLocation::Page { page, .. } => Some(page),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSource {
    pub kind: SourceKind,
    pub records: Vec<SourceRecord>,
    pub notes: Vec<String>,
    pub pages_without_text: Vec<u32>,
}
