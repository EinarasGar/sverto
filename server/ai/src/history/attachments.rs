use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use rig::message::{
    Document, DocumentData, DocumentMediaType, DocumentSourceKind, ImageMediaType, Message,
    MimeType, UserContent,
};
use uuid::Uuid;

use crate::models::chat::Base64Attachment;

const FILE_REFERENCE: &str = "myra-file:";

pub(crate) fn user_message(
    text: String,
    file_ids: &[Uuid],
    files: &HashMap<Uuid, Base64Attachment>,
) -> Message {
    let mut content = vec![UserContent::text(text)];
    content.extend(
        file_ids
            .iter()
            .filter_map(|id| files.get(id).map(|file| reference(*id, &file.media_type))),
    );
    Message::User { content }
}

fn reference(file_id: Uuid, media_type: &str) -> UserContent {
    let url = format!("{FILE_REFERENCE}{file_id}");
    match ImageMediaType::from_mime_type(media_type) {
        Some(image) => UserContent::image_url(url, Some(image), None),
        None => UserContent::Document(Document {
            data: DocumentData::File(DocumentSourceKind::Url(url)),
            media_type: DocumentMediaType::from_mime_type(media_type),
            additional_params: None,
        }),
    }
}

pub(crate) fn file_id(part: &UserContent) -> Option<Uuid> {
    let url = match part {
        UserContent::Image(image) => match &image.data {
            DocumentSourceKind::Url(url) => url,
            _ => return None,
        },
        UserContent::Document(document) => match &document.data {
            DocumentData::File(DocumentSourceKind::Url(url)) => url,
            _ => return None,
        },
        _ => return None,
    };
    url.strip_prefix(FILE_REFERENCE)?.parse().ok()
}

pub(crate) fn referenced_files(messages: &[Arc<Message>]) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for message in messages {
        if let Message::User { content } = &**message {
            for id in content.iter().filter_map(file_id) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

pub(crate) fn resolve(
    messages: &[Arc<Message>],
    files: &HashMap<Uuid, Base64Attachment>,
) -> Vec<Message> {
    messages
        .iter()
        .map(|message| match &**message {
            Message::User { content } if content.iter().any(|part| file_id(part).is_some()) => {
                Message::User {
                    content: content
                        .iter()
                        .filter_map(|part| match file_id(part) {
                            Some(id) => resolved(part, files.get(&id)?),
                            None => Some(part.clone()),
                        })
                        .collect(),
                }
            }
            other => other.clone(),
        })
        .collect()
}

fn resolved(reference: &UserContent, file: &Base64Attachment) -> Option<UserContent> {
    match reference {
        UserContent::Image(image) => Some(UserContent::image_base64(
            file.data.clone(),
            image.media_type.clone(),
            None,
        )),
        UserContent::Document(document) => match document.media_type.clone() {
            Some(DocumentMediaType::PDF) => Some(UserContent::document_base64(
                file.data.clone(),
                Some(DocumentMediaType::PDF),
            )),
            Some(text_type) => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&file.data)
                    .ok()?;
                Some(UserContent::document_text(
                    String::from_utf8_lossy(&bytes).into_owned(),
                    Some(text_type),
                ))
            }
            None => {
                tracing::warn!(
                    media_type = %file.media_type,
                    "Skipping an attachment that is unsupported"
                );
                None
            }
        },
        other => Some(other.clone()),
    }
}
