//! Tool-call approval gate for the chat workflow. Builds a parallel toolset
//! containing only the gated tools (used to replay them after the user has
//! mid-stream so the UI can ask the user before they execute.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rig::agent::{
    AgentHook, CompletionCallAction, CompletionCallEvent, DispatchAction, DispatchEvent,
    HookContext,
};
use rig::tool::Tool;

use crate::action_provider::AiActionProvider;
use crate::models::chat::ToolRequestPayload;
use crate::tools::create_custom_asset::CreateCustomAssetTool;
use crate::tools::create_transaction::CreateTransactionTool;
use crate::tools::delete_transaction::DeleteTransactionTool;
use crate::tools::record_asset_swap::RecordAssetSwapTool;
use crate::tools::record_asset_trade::RecordAssetTradeTool;
use crate::tools::record_asset_transfer::RecordAssetTransferTool;
use crate::tools::record_cash_transfer::RecordCashTransferTool;
use crate::tools::record_dividend::RecordDividendTool;
use crate::tools::record_fee::RecordFeeTool;
use crate::tools::record_transfer::RecordTransferTool;
use crate::tools::update_asset_valuation::UpdateAssetValuationTool;
use crate::tools::update_transaction::UpdateTransactionTool;

const PENDING_APPROVAL_RESULT: &str = "Tool call requires user approval. Awaiting response.";

pub(crate) struct GatedToolSet {
    pub toolset: rig::tool::ToolSet,
    pub gated_names: HashSet<String>,
}

pub(crate) fn build_gated_toolset<A: AiActionProvider>(actions: Arc<A>) -> GatedToolSet {
    let mut toolset = rig::tool::ToolSet::default();
    toolset.add_tool(CreateTransactionTool::new(actions.clone()));
    toolset.add_tool(CreateCustomAssetTool::new(actions.clone()));
    toolset.add_tool(RecordAssetTradeTool::new(actions.clone()));
    toolset.add_tool(RecordTransferTool::new(actions.clone()));
    toolset.add_tool(RecordCashTransferTool::new(actions.clone()));
    toolset.add_tool(RecordAssetTransferTool::new(actions.clone()));
    toolset.add_tool(RecordAssetSwapTool::new(actions.clone()));
    toolset.add_tool(UpdateAssetValuationTool::new(actions.clone()));
    toolset.add_tool(RecordDividendTool::new(actions.clone()));
    toolset.add_tool(RecordFeeTool::new(actions.clone()));
    toolset.add_tool(UpdateTransactionTool::new(actions.clone()));
    toolset.add_tool(DeleteTransactionTool::new(actions));

    let gated_names: HashSet<String> = [
        CreateTransactionTool::<A>::NAME,
        CreateCustomAssetTool::<A>::NAME,
        RecordAssetTradeTool::<A>::NAME,
        RecordTransferTool::<A>::NAME,
        RecordCashTransferTool::<A>::NAME,
        RecordAssetTransferTool::<A>::NAME,
        RecordAssetSwapTool::<A>::NAME,
        UpdateAssetValuationTool::<A>::NAME,
        RecordDividendTool::<A>::NAME,
        RecordFeeTool::<A>::NAME,
        UpdateTransactionTool::<A>::NAME,
        DeleteTransactionTool::<A>::NAME,
    ]
    .iter()
    .map(|s: &&str| s.to_string())
    .collect();

    GatedToolSet {
        toolset,
        gated_names,
    }
}

#[derive(Clone)]
pub(crate) struct ApprovalHook {
    gated_names: HashSet<String>,
    pending: Arc<Mutex<Vec<ToolRequestPayload>>>,
}

impl ApprovalHook {
    pub(crate) fn new(gated_names: HashSet<String>) -> Self {
        Self {
            gated_names,
            pending: Arc::default(),
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending().is_empty()
    }

    pub(crate) fn is_pending(&self, tool_call_id: &str) -> bool {
        self.pending()
            .iter()
            .any(|request| request.tool_call_id == tool_call_id)
    }

    pub(crate) fn take_pending(&self) -> Vec<ToolRequestPayload> {
        std::mem::take(&mut *self.pending())
    }

    fn pending(&self) -> MutexGuard<'_, Vec<ToolRequestPayload>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AgentHook for ApprovalHook {
    // Once a gated call has been captured, stop before the next model turn so
    // itself lets every gated call in the same turn be captured first.
    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        _event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        if self.has_pending() {
            CompletionCallAction::stop(PENDING_APPROVAL_RESULT)
        } else {
            CompletionCallAction::continue_run()
        }
    }

    async fn on_dispatch(&self, _ctx: &HookContext, event: DispatchEvent<'_>) -> DispatchAction {
        let Some(tool_name) = event.tool_name() else {
            return DispatchAction::proceed();
        };
        if !self.gated_names.contains(tool_name) {
            return DispatchAction::proceed();
        }
        self.pending().push(ToolRequestPayload {
            tool_call_id: event.call_id.map(ToString::to_string).unwrap_or_default(),
            name: tool_name.to_string(),
            args: event.tool_args().unwrap_or("{}").to_string(),
        });
        DispatchAction::skip(PENDING_APPROVAL_RESULT)
    }
}
