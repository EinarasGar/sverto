use std::env;

use crate::models::provider::{CompletionProvider, EmbeddingProvider, SystemOneProvider};

#[derive(Clone)]
pub struct AiConfig {
    pub provider: CompletionProvider,
    pub api_key: String,
    pub model: String,
    pub embedding_provider: EmbeddingProvider,
    pub embedding_api_key: String,
    pub embedding_model: String,
}

impl AiConfig {
    pub fn try_from_env() -> anyhow::Result<Self> {
        Ok(Self {
            provider: required("AI_PROVIDER")?.parse()?,
            api_key: required("AI_API_KEY")?,
            model: required("AI_MODEL")?,
            embedding_provider: required("AI_EMBEDDING_PROVIDER")?.parse()?,
            embedding_api_key: required("AI_EMBEDDING_API_KEY")?,
            embedding_model: required("AI_EMBEDDING_MODEL")?,
        })
    }
}

#[derive(Clone)]
pub struct SystemOneConfig {
    pub provider: SystemOneProvider,
    pub api_key: String,
    pub model: String,
}

impl SystemOneConfig {
    pub fn try_from_env() -> anyhow::Result<Self> {
        Ok(Self {
            provider: required("SYSTEM_ONE_PROVIDER")?.parse()?,
            api_key: required("SYSTEM_ONE_KEY")?,
            model: required("SYSTEM_ONE_MODEL")?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportClassifier {
    Llm,
    SystemOne,
}

impl ImportClassifier {
    pub fn try_from_env() -> anyhow::Result<Self> {
        match required("IMPORT_CLASSIFIER")?.as_str() {
            "llm" => Ok(Self::Llm),
            "system_one" => Ok(Self::SystemOne),
            other => Err(anyhow::anyhow!(
                "IMPORT_CLASSIFIER must be llm or system_one, not {other}"
            )),
        }
    }
}

fn required(name: &str) -> anyhow::Result<String> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        Ok(_) => Err(anyhow::anyhow!("{name} is empty")),
        Err(_) => Err(anyhow::anyhow!("{name} not set")),
    }
}
