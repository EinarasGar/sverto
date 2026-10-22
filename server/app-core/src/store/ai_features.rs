use super::infra::SharedInfra;
use crate::error::ApiError;

pub async fn set_ai_features_enabled(
    infra: &SharedInfra,
    enabled: bool,
    auth_token: Option<&str>,
) -> Result<(), ApiError> {
    let user_id = infra.user_id().ok_or_else(|| ApiError::Parse {
        reason: "no user_id".into(),
    })?;
    let body = serde_json::json!({ "enabled": enabled }).to_string();
    let path = format!("/api/users/{user_id}/ai-features");
    let resp = infra.post(&path, &body, auth_token).await?;
    if resp.status >= 400 {
        return Err(crate::error::server_error(resp.status, &resp.body));
    }
    infra.set_ai_features_enabled(enabled);
    Ok(())
}
