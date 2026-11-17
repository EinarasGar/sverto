use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionProvider {
    Gemini,
    Anthropic,
}

impl FromStr for CompletionProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gemini" => Ok(Self::Gemini),
            "anthropic" => Ok(Self::Anthropic),
            _ => Err(anyhow::anyhow!("Unknown AI provider: {s}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddingProvider {
    Gemini,
}

impl FromStr for EmbeddingProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gemini" => Ok(Self::Gemini),
            _ => Err(anyhow::anyhow!("Unknown AI embedding provider: {s}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemOneProvider {
    TypeSafe,
}

impl FromStr for SystemOneProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "typesafe" => Ok(Self::TypeSafe),
            _ => Err(anyhow::anyhow!("Unknown System One provider: {s}")),
        }
    }
}
