use std::collections::HashMap;

use super::infra::SharedInfra;
use super::{ok_or_err, user_id};
use crate::error::ApiError;

#[derive(serde::Serialize)]
struct FileCreateBody<'a> {
    mime_type: &'a str,
    original_name: &'a str,
    size_bytes: u64,
}

#[derive(serde::Deserialize)]
struct ServerFileRecord {
    id: String,
    upload_metadata: UploadMetadata,
}

#[derive(serde::Deserialize)]
struct UploadMetadata {
    upload_url: String,
    upload_headers: HashMap<String, String>,
    upload_method: String,
}

#[derive(serde::Deserialize)]
struct ServerFile {
    original_name: String,
}

pub async fn upload_file(
    infra: &SharedInfra,
    data: &[u8],
    mime_type: &str,
    file_name: &str,
    auth_token: Option<&str>,
) -> Result<String, ApiError> {
    let uid = user_id(infra)?;
    let create_body = serde_json::to_string(&FileCreateBody {
        mime_type,
        original_name: file_name,
        size_bytes: data.len() as u64,
    })
    .map_err(|e| ApiError::Parse {
        reason: e.to_string(),
    })?;

    let create_resp = infra
        .post(&format!("/api/users/{uid}/files"), &create_body, auth_token)
        .await?;
    ok_or_err(create_resp.status, &create_resp.body)?;
    let file_record: ServerFileRecord =
        serde_json::from_str(&create_resp.body).map_err(|e| ApiError::Parse {
            reason: e.to_string(),
        })?;

    let method = reqwest::Method::from_bytes(file_record.upload_metadata.upload_method.as_bytes())
        .unwrap_or(reqwest::Method::PUT);

    let mut upload_req = infra
        .http
        .request(method, &file_record.upload_metadata.upload_url);

    for (key, value) in &file_record.upload_metadata.upload_headers {
        upload_req = upload_req.header(key, value);
    }

    let upload_resp =
        upload_req
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| ApiError::Network {
                reason: e.to_string(),
            })?;

    if !upload_resp.status().is_success() {
        return Err(ApiError::Server {
            reason: format!("S3 upload failed: {}", upload_resp.status()),
            status: upload_resp.status().as_u16(),
        });
    }

    let confirm_resp = infra
        .post(
            &format!("/api/users/{uid}/files/{}/confirm", file_record.id),
            "{}",
            auth_token,
        )
        .await?;
    ok_or_err(confirm_resp.status, &confirm_resp.body)?;

    Ok(file_record.id)
}

pub async fn file_name(
    infra: &SharedInfra,
    file_id: &str,
    auth_token: Option<&str>,
) -> Result<String, ApiError> {
    let uid = user_id(infra)?;
    let resp = infra
        .get(&format!("/api/users/{uid}/files/{file_id}"), auth_token)
        .await?;
    ok_or_err(resp.status, &resp.body)?;
    let file: ServerFile = serde_json::from_str(&resp.body).map_err(|e| ApiError::Parse {
        reason: e.to_string(),
    })?;
    Ok(file.original_name)
}
