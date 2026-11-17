mod attachments;
mod commit;
mod store;
mod transcript;

use std::collections::HashSet;

use rig::conversation::{Aside, AsideId, Author, Conversation, MessageId};

pub(crate) use attachments::{referenced_files, resolve, user_message};
pub(crate) use commit::CommitHook;
pub(crate) use store::{load, save};
pub(crate) use transcript::render_tool_result;
pub use transcript::{preview, transcript};

use crate::models::history::ToolApprovalAside;

const TOOL_APPROVAL: &str = "myra.tool_approval";

pub(crate) fn tool_approval(
    tool_call_id: String,
    approved: bool,
    after: Option<MessageId>,
) -> Aside {
    let value = serde_json::to_value(ToolApprovalAside {
        tool_call_id,
        approved,
    })
    .expect("a tool approval serializes to JSON");
    Aside::new(TOOL_APPROVAL, Author::user(), value).after(after)
}

pub(crate) fn decided_calls(conversation: &Conversation) -> HashSet<String> {
    tool_approvals(conversation)
        .into_iter()
        .map(|(_, _, approval)| approval.tool_call_id)
        .collect()
}

fn tool_approvals(
    conversation: &Conversation,
) -> Vec<(AsideId, Option<MessageId>, ToolApprovalAside)> {
    conversation
        .asides()
        .filter(|(_, aside)| aside.kind == TOOL_APPROVAL)
        .filter_map(|(id, aside)| {
            let approval = serde_json::from_value(aside.value.clone()).ok()?;
            Some((id, aside.after, approval))
        })
        .collect()
}
