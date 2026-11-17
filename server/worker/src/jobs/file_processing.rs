//! Validates an uploaded file (MIME check, size check), generates a
//! thumbnail for images, and marks the file as ready.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use business::jobs::FileProcessingJob;
use business::service_collection::ServiceProviders;
use dal::file_provider::FileProvider;
use dal::models::file_models::FileStatus;
use dal::queries::file_queries;
use infer::MatcherType;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::jobs::WorkerJob;

#[async_trait]
impl WorkerJob for FileProcessingJob {
    const NAME: &'static str = "file_processing";

    async fn run(&self, providers: &ServiceProviders) -> anyhow::Result<()> {
        handle(providers, self.file_id, self.user_id).await
    }
}

const MAX_THUMBNAIL_SIZE: u64 = 10_485_760;
const MIME_DETECTION_BYTES: u64 = 8192;
const MIN_DETECTABLE_FILE_SIZE: u64 = 8;
const MAX_CONCURRENT_THUMBNAILS: usize = 4;

static THUMBNAIL_SEMAPHORE: once_cell::sync::Lazy<Semaphore> =
    once_cell::sync::Lazy::new(|| Semaphore::new(MAX_CONCURRENT_THUMBNAILS));

#[tracing::instrument(level = "info", skip_all, fields(file_id = %file_id, user_id = %user_id))]
async fn handle(providers: &ServiceProviders, file_id: Uuid, user_id: Uuid) -> Result<()> {
    let query = file_queries::get_files(
        dal::query_params::get_files_params::GetFilesParams::by_id(user_id, file_id),
    );
    let file = providers
        .db
        .fetch_optional::<dal::models::file_models::FileModel>(query)
        .await?
        .ok_or_else(|| anyhow!("File {} not found", file_id))?;

    let result = process_file(
        &providers.file_provider,
        file_id,
        &file.storage_key,
        &file.mime_type,
        file.size_bytes,
    )
    .await;

    match result {
        Ok(thumbnail_key) => {
            let update_query =
                file_queries::update_file_ready(file_id, user_id, thumbnail_key.clone());
            let rows = providers
                .db
                .execute_with_rows_affected(update_query)
                .await?;
            if rows == 0 {
                if let Some(ref thumb_key) = thumbnail_key {
                    if let Err(e) = providers.file_provider.delete(thumb_key).await {
                        tracing::warn!(file_id = %file_id, error = %e, "Failed to clean up orphaned thumbnail");
                    }
                }
                tracing::warn!(file_id = %file_id, "update_file_ready affected 0 rows");
            }
        }
        Err(e) => {
            let fail_query = file_queries::update_file_status(file_id, user_id, FileStatus::Failed);
            if let Err(db_err) = providers.db.execute(fail_query).await {
                tracing::error!(
                    file_id = %file_id,
                    error = ?db_err,
                    error.type = "db_status_update",
                    "failed to mark file failed"
                );
            }
            return Err(e);
        }
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(file_id = %file_id, declared_mime = %declared_mime))]
async fn process_file(
    file_provider: &Arc<dyn FileProvider>,
    file_id: Uuid,
    storage_key: &str,
    declared_mime: &str,
    declared_size: i64,
) -> Result<Option<String>> {
    let expected_size =
        u64::try_from(declared_size).map_err(|_| anyhow!("Invalid stored file size"))?;

    if expected_size < MIN_DETECTABLE_FILE_SIZE {
        return Err(anyhow!(
            "File too small for MIME detection ({} bytes, minimum {} required)",
            expected_size,
            MIN_DETECTABLE_FILE_SIZE
        ));
    }

    let declared_base_mime = declared_mime.split(';').next().unwrap_or("").trim();

    let header_bytes = file_provider
        .download_range(
            storage_key,
            0,
            MIME_DETECTION_BYTES.min(expected_size).saturating_sub(1),
        )
        .await?;

    let detected = infer::get(&header_bytes);
    let text = detected.is_none_or(|kind| kind.matcher_type() == MatcherType::Text);
    let container = detected.is_some_and(|kind| {
        kind.matcher_type() == MatcherType::Doc
            || matches!(
                kind.mime_type(),
                "application/zip" | "application/x-ole-storage"
            )
    });
    let must_match =
        declared_base_mime.starts_with("image/") || declared_base_mime == "application/pdf";
    let label_holds = detected.is_some_and(|kind| kind.mime_type() == declared_base_mime)
        || (!must_match && (text || (container && !declared_base_mime.starts_with("text/"))));
    if !label_holds {
        return Err(anyhow!(
            "MIME type mismatch: declared {} but detected {}",
            declared_base_mime,
            detected.map_or("no known signature", |kind| kind.mime_type())
        ));
    }

    let actual_size = file_provider.head_object(storage_key).await?;
    if actual_size != expected_size {
        return Err(anyhow!(
            "Size mismatch: declared {} but actual {}",
            declared_size,
            actual_size
        ));
    }

    let thumbnail_key =
        if declared_base_mime.starts_with("image/") && expected_size <= MAX_THUMBNAIL_SIZE {
            let _permit = THUMBNAIL_SEMAPHORE
                .acquire()
                .await
                .map_err(|_| anyhow!("Thumbnail semaphore closed"))?;
            let bytes = file_provider.download(storage_key).await?;
            match generate_thumbnail(&bytes) {
                Ok(thumb_bytes) => {
                    let thumb_key = format!("thumbnails/{}.webp", file_id);
                    file_provider
                        .upload(&thumb_key, &thumb_bytes, "image/webp")
                        .await?;
                    Some(thumb_key)
                }
                Err(e) => {
                    tracing::warn!(file_id = %file_id, error = %e, "Failed to generate thumbnail");
                    None
                }
            }
        } else {
            None
        };

    Ok(thumbnail_key)
}

fn generate_thumbnail(bytes: &[u8]) -> Result<Vec<u8>> {
    use image::ImageReader;
    use std::io::Cursor;

    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    let thumbnail = img.thumbnail(256, 256);

    let mut output = Vec::new();
    thumbnail.write_to(&mut Cursor::new(&mut output), image::ImageFormat::WebP)?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use dal::file_provider::PresignedUpload;

    use super::*;

    const OLE_WORKBOOK: &[u8] = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00\x00\x00\x00\x00\x00\x00";
    const PNG: &[u8] = b"\x89PNG\r\n\x1A\n\x00\x00\x00\rIHDR";
    const PDF: &[u8] = b"%PDF-1.7\n1 0 obj\n";

    struct Stored(Vec<u8>);

    #[async_trait]
    impl FileProvider for Stored {
        async fn generate_presigned_upload(
            &self,
            _key: &str,
            _content_type: &str,
            _content_length: u64,
            _expires_in_seconds: u32,
        ) -> Result<PresignedUpload> {
            Err(anyhow!("not used"))
        }

        async fn generate_presigned_download(
            &self,
            _key: &str,
            _expires_in_seconds: u32,
            _download_filename: Option<&str>,
        ) -> Result<String> {
            Err(anyhow!("not used"))
        }

        async fn download(&self, _key: &str) -> Result<Vec<u8>> {
            Ok(self.0.clone())
        }

        async fn download_range(&self, _key: &str, start: u64, end: u64) -> Result<Vec<u8>> {
            Ok(self.0[start as usize..=(end as usize).min(self.0.len() - 1)].to_vec())
        }

        async fn head_object(&self, _key: &str) -> Result<u64> {
            Ok(self.0.len() as u64)
        }

        async fn upload(&self, _key: &str, _content: &[u8], _content_type: &str) -> Result<()> {
            Ok(())
        }

        async fn delete(&self, _key: &str) -> Result<()> {
            Ok(())
        }
    }

    async fn accepted(content: &[u8], declared: &str) -> bool {
        let provider: Arc<dyn FileProvider> = Arc::new(Stored(content.to_vec()));
        process_file(
            &provider,
            Uuid::nil(),
            "key",
            declared,
            content.len() as i64,
        )
        .await
        .is_ok()
    }

    fn zip_starting_with(entry: &str, content: &[u8]) -> Vec<u8> {
        let mut bytes = b"PK\x03\x04\x14\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
        bytes.extend([0u8; 4]);
        bytes.extend((content.len() as u32).to_le_bytes());
        bytes.extend((content.len() as u32).to_le_bytes());
        bytes.extend((entry.len() as u16).to_le_bytes());
        bytes.extend([0u8; 2]);
        bytes.extend(entry.as_bytes());
        bytes.extend(content);
        bytes
    }

    #[tokio::test]
    async fn text_is_accepted_whatever_the_browser_called_it() {
        for (content, declared) in [
            (
                &b"Date,Amount\n2024-01-02,-3.50\n"[..],
                "application/vnd.ms-excel",
            ),
            (
                b"OFXHEADER:100\nDATA:OFXSGML\n\n<OFX>\n<STMTTRN>\n",
                "application/x-ofx",
            ),
            (
                b":20:STMT\n:25:DE4370054306\n:60F:C240101EUR1,00\n",
                "application/octet-stream",
            ),
            (b"!Type:Bank\nD01/02/2024\nT-3.50\n^\n", "application/x-qif"),
            (
                b"<?xml version=\"1.0\"?>\n<Document><Ntry/></Document>",
                "application/xml",
            ),
            (b"{\"transactions\": []}", "application/json"),
            (b"Date,Amount\n2024-01-02,-3.50\n", "text/csv"),
        ] {
            assert!(accepted(content, declared).await, "{declared}");
        }
    }

    #[tokio::test]
    async fn workbook_containers_are_accepted_under_any_document_label() {
        let xlsx = zip_starting_with("xl/workbook.xml", b"<workbook/>");
        let ods = zip_starting_with(
            "mimetype",
            b"application/vnd.oasis.opendocument.spreadsheet",
        );
        let bare_zip = zip_starting_with("sheet.xml", b"<sheet/>");
        for (content, declared) in [
            (OLE_WORKBOOK, "application/vnd.ms-excel"),
            (OLE_WORKBOOK, "application/octet-stream"),
            (
                &xlsx[..],
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            (&xlsx[..], "application/vnd.ms-excel"),
            (&xlsx[..], "application/octet-stream"),
            (&ods[..], "application/vnd.oasis.opendocument.spreadsheet"),
            (&ods[..], "application/octet-stream"),
            (
                &bare_zip[..],
                "application/vnd.oasis.opendocument.spreadsheet",
            ),
        ] {
            assert!(accepted(content, declared).await, "{declared}");
        }
    }

    #[tokio::test]
    async fn a_file_declared_as_an_image_or_a_pdf_must_be_one() {
        let xlsx = zip_starting_with("xl/workbook.xml", b"<workbook/>");
        for (content, declared) in [
            (&b"<svg onload=\"alert(1)\"/>"[..], "image/png"),
            (b"Date,Amount\n2024-01-02,-3.50\n", "application/pdf"),
            (&xlsx[..], "image/jpeg"),
            (OLE_WORKBOOK, "application/pdf"),
            (PDF, "image/png"),
        ] {
            assert!(!accepted(content, declared).await, "{declared}");
        }
        assert!(accepted(PNG, "image/png").await);
        assert!(accepted(PDF, "application/pdf").await);
    }

    #[tokio::test]
    async fn media_and_executables_cannot_hide_behind_another_label() {
        let mut elf = b"\x7FELF\x02\x01\x01".to_vec();
        elf.resize(64, 0);
        for (content, declared) in [
            (PNG, "text/csv"),
            (PNG, "application/octet-stream"),
            (PDF, "text/plain"),
            (&elf[..], "text/plain"),
            (
                b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xFF\xFF\x00\x00",
                "application/vnd.ms-excel",
            ),
        ] {
            assert!(!accepted(content, declared).await, "{declared}");
        }
    }
}
