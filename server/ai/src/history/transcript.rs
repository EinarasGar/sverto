use rig::message::{AssistantContent, Message, ToolResult, ToolResultContent, UserContent};
use time::OffsetDateTime;
use uuid::Uuid;

use super::attachments::file_id;
use super::store::parse;
use super::tool_approvals;
use crate::models::error::AiError;
use crate::models::history::{TranscriptEntry, TranscriptItem};

const PREVIEW_WORDS: usize = 8;
const PREVIEW_CHARS: usize = 60;
const TOOL_FAILED: &str = "Tool execution failed";

pub fn transcript(history: &[u8]) -> Result<Vec<TranscriptItem>, AiError> {
    let conversation = parse(history)?;
    let approvals = tool_approvals(&conversation);

    let mut items = Vec::new();
    for entry in conversation.entries() {
        let id = entry.id;
        let created_at = made_at(entry.made);
        let item = |id: String, entry: TranscriptEntry| TranscriptItem {
            id,
            entry,
            file_ids: Vec::new(),
            created_at,
        };
        match entry.message {
            Message::User { content } => {
                let file_ids: Vec<Uuid> = content.iter().filter_map(file_id).collect();
                if let Some(text) = user_text(content).filter(|_| !is_tool_results(content)) {
                    items.push(TranscriptItem {
                        file_ids,
                        ..item(id.to_string(), TranscriptEntry::User { content: text })
                    });
                }
                for (part, content) in content.iter().enumerate() {
                    if let UserContent::ToolResult(result) = content {
                        items.push(item(
                            format!("{id}.{part}"),
                            TranscriptEntry::ToolResult {
                                tool_call_id: result.call.to_string(),
                                content: render_tool_result(result),
                            },
                        ));
                    }
                }
            }
            Message::Assistant(reply) => {
                for (part, content) in reply.content.iter().enumerate() {
                    let entry = match content {
                        AssistantContent::Text(text) if !text.text.is_empty() => {
                            TranscriptEntry::Assistant {
                                content: text.text.clone(),
                            }
                        }
                        AssistantContent::ToolCall(call) => TranscriptEntry::AssistantToolCall {
                            tool_call_id: call.id.to_string(),
                            name: call.function.name.to_string(),
                            args: call.function.raw_arguments(),
                        },
                        _ => continue,
                    };
                    items.push(item(format!("{id}.{part}"), entry));
                }
            }
            Message::System { .. } => {}
        }
        for (aside, _, approval) in approvals.iter().filter(|(_, after, _)| *after == Some(id)) {
            items.push(item(
                aside.to_string(),
                TranscriptEntry::ToolApproval {
                    tool_call_id: approval.tool_call_id.clone(),
                    approved: approval.approved,
                },
            ));
        }
    }
    Ok(items)
}

pub fn preview(history: &[u8]) -> Result<Option<String>, AiError> {
    let conversation = parse(history)?;
    let first = conversation
        .messages()
        .iter()
        .find_map(|message| match &**message {
            Message::User { content } if !is_tool_results(content) => Some(content),
            _ => None,
        });
    Ok(first
        .and_then(|content| user_text(content))
        .and_then(|text| {
            let words: Vec<&str> = text.split_whitespace().take(PREVIEW_WORDS).collect();
            (!words.is_empty()).then(|| words.join(" ").chars().take(PREVIEW_CHARS).collect())
        }))
}

pub(crate) fn render_tool_result(result: &ToolResult) -> String {
    let output = result
        .content
        .iter()
        .filter_map(|content| match content {
            ToolResultContent::Text(text) => Some(text.text.clone()),
            ToolResultContent::Json { value } => Some(value.to_string()),
            ToolResultContent::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if result.is_error {
        format!("{TOOL_FAILED}: {output}")
    } else {
        output
    }
}

fn user_text(content: &[UserContent]) -> Option<String> {
    let text: Vec<&str> = content
        .iter()
        .filter_map(|part| match part {
            UserContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    let has_files = content.iter().any(|part| file_id(part).is_some());
    (!text.is_empty() || has_files).then(|| text.join("\n"))
}

fn is_tool_results(content: &[UserContent]) -> bool {
    content
        .iter()
        .any(|part| matches!(part, UserContent::ToolResult(_)))
}

fn made_at(millis: u64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rig::conversation::{pairing, Conversation};
    use rig::memory::{MemoryStore, Store};
    use rig::message::{AssistantMessage, ToolName};
    use serde_json::json;

    use super::*;
    use crate::history::store::LOG;
    use crate::history::{tool_approval, user_message};
    use crate::models::chat::Base64Attachment;

    const FILE: Uuid = Uuid::from_u128(0x0b6f0b7e_5f1c_4f1e_9d3a_2a7e3c1d9f10);

    fn saved(mut conversation: Conversation) -> Vec<u8> {
        let store = MemoryStore::default();
        rig::memory::save(&store, LOG, &mut conversation).unwrap();
        store.read_log(LOG).unwrap()
    }

    fn sample() -> Conversation {
        let files = HashMap::from([(
            FILE,
            Base64Attachment {
                media_type: "image/png".into(),
                data: "UE5H".into(),
            },
        )]);
        let fee = ToolName::new("record_fee").unwrap();
        let mut conversation = Conversation::new();
        conversation.push(user_message(
            "Record the fees on this receipt".into(),
            &[FILE],
            &files,
        ));
        conversation.push(Message::Assistant(AssistantMessage::new(vec![
            AssistantContent::reasoning("Two fees."),
            AssistantContent::text("Recording them."),
            AssistantContent::tool_call("c1", fee.clone(), json!({"amount": 1.5})),
            AssistantContent::tool_call("c2", fee, json!({"amount": 2.5})),
        ])));
        let calls = conversation.pending_calls();
        let after = conversation.id(1);
        conversation.add_aside(tool_approval("c1".into(), true, after));
        conversation.add_aside(tool_approval("c2".into(), false, after));
        let Message::User { content: declined } =
            pairing::close_pending_with([&calls[1]], "User declined this action.")
        else {
            unreachable!()
        };
        let mut results =
            vec![UserContent::ToolResult(calls[0].result(vec![
                ToolResultContent::json(json!({"recorded": 1.5})),
            ]))];
        results.extend(declined);
        conversation.push(Message::User { content: results });
        conversation.push(Message::assistant("Done."));
        conversation
    }

    fn entries(items: Vec<TranscriptItem>) -> Vec<(String, TranscriptEntry, Vec<Uuid>)> {
        items
            .into_iter()
            .map(|item| (item.id, item.entry, item.file_ids))
            .collect()
    }

    #[test]
    fn the_transcript_lists_what_the_clients_show_in_order() {
        let items = transcript(&saved(sample())).unwrap();

        assert_eq!(
            entries(items),
            vec![
                (
                    "m0".into(),
                    TranscriptEntry::User {
                        content: "Record the fees on this receipt".into()
                    },
                    vec![FILE]
                ),
                (
                    "m1.1".into(),
                    TranscriptEntry::Assistant {
                        content: "Recording them.".into()
                    },
                    vec![]
                ),
                (
                    "m1.2".into(),
                    TranscriptEntry::AssistantToolCall {
                        tool_call_id: "c1".into(),
                        name: "record_fee".into(),
                        args: r#"{"amount":1.5}"#.into()
                    },
                    vec![]
                ),
                (
                    "m1.3".into(),
                    TranscriptEntry::AssistantToolCall {
                        tool_call_id: "c2".into(),
                        name: "record_fee".into(),
                        args: r#"{"amount":2.5}"#.into()
                    },
                    vec![]
                ),
                (
                    "a0".into(),
                    TranscriptEntry::ToolApproval {
                        tool_call_id: "c1".into(),
                        approved: true
                    },
                    vec![]
                ),
                (
                    "a1".into(),
                    TranscriptEntry::ToolApproval {
                        tool_call_id: "c2".into(),
                        approved: false
                    },
                    vec![]
                ),
                (
                    "m2.0".into(),
                    TranscriptEntry::ToolResult {
                        tool_call_id: "c1".into(),
                        content: r#"{"recorded":1.5}"#.into()
                    },
                    vec![]
                ),
                (
                    "m2.1".into(),
                    TranscriptEntry::ToolResult {
                        tool_call_id: "c2".into(),
                        content: "Tool execution failed: User declined this action.".into()
                    },
                    vec![]
                ),
                (
                    "m3.0".into(),
                    TranscriptEntry::Assistant {
                        content: "Done.".into()
                    },
                    vec![]
                ),
            ]
        );
    }

    #[test]
    fn a_first_message_with_no_text_has_no_preview() {
        let files = HashMap::from([(
            FILE,
            Base64Attachment {
                media_type: "image/png".into(),
                data: "UE5H".into(),
            },
        )]);
        let mut conversation = Conversation::new();
        conversation.push(user_message(String::new(), &[FILE], &files));

        assert_eq!(preview(&saved(conversation)).unwrap(), None);
        assert_eq!(preview(&[]).unwrap(), None);
    }

    #[test]
    fn the_preview_reads_from_the_start_of_a_long_history() {
        let mut conversation = sample();
        conversation.push(Message::assistant("x".repeat(20_000)));
        let history = saved(conversation);

        assert_eq!(
            preview(&history[..16_384]).unwrap().as_deref(),
            Some("Record the fees on this receipt")
        );
    }
}
