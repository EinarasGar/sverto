use crate::config::AiConfig;
use crate::models::error::AiError;
use crate::provider::embedding_model;

pub const EMBEDDING_DIMS: usize = 1536;

pub type EmbeddingModel = rig::DynModel<rig::operation::Embedding>;

#[tracing::instrument(skip_all, level = "debug", fields(text_len = text.len(), otel.kind = "client"))]
pub async fn embed_query(model: &EmbeddingModel, text: &str) -> Result<Vec<f64>, AiError> {
    let embedding = model.embed_text(text).await.map_err(AiError::from)?;
    Ok(embedding.vec)
}

#[tracing::instrument(skip_all, level = "debug", fields(text_len = text.len(), otel.kind = "client"))]
pub async fn embed_text(config: &AiConfig, text: &str) -> Result<Vec<f64>, AiError> {
    embed_query(&embedding_model(config), text).await
}
