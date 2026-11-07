use serde_json::Value;

#[derive(Debug)]
pub(crate) enum FlexReply {
    Accepted { reference_code: String },
    Failed { code: Option<u32>, message: String },
    Statement(Vec<Value>),
}
