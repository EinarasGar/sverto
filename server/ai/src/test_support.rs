use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use rig::agent::AgentBuilder;
use schemars::JsonSchema;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::action_provider::AiActionProvider;
use crate::conversation_provider::{ConversationProvider, ConversationSource};
use crate::models::action::*;
use crate::models::agent::{ModelAgent, StructuredAgent};
use crate::models::chat::Base64Attachment;
use crate::models::error::AiError;
use crate::models::history::HistoryWrite;
use crate::rate_limit_provider::{RateLimitError, RateLimitProvider};

pub(crate) const MODEL: &str = "gemini-3.8-flash";

pub(crate) enum Reply {
    Json(Value),
    Sse(Value),
    Status(u16, Value),
}

pub(crate) struct FakeModel {
    pub url: String,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeModel {
    pub(crate) async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        tokio::spawn(async move {
            for reply in replies {
                let Ok(Ok((socket, _))) =
                    tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                        .await
                else {
                    return;
                };
                serve(socket, reply, &recorded).await;
            }
        });
        Self { url, requests }
    }

    pub(crate) fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn gemini(&self) -> rig::providers::gemini::Gemini {
        rig::providers::gemini::GeminiConfig::new("test-key")
            .with_base_url(&self.url)
            .client()
    }

    pub(crate) fn structured<T: JsonSchema>(
        &self,
        configure: impl FnOnce(AgentBuilder) -> AgentBuilder,
    ) -> StructuredAgent<T> {
        StructuredAgent::new(
            configure(AgentBuilder::new(self.gemini().completion(MODEL))),
            MODEL.to_string(),
        )
    }

    pub(crate) fn agent(&self, configure: impl FnOnce(AgentBuilder) -> AgentBuilder) -> ModelAgent {
        ModelAgent {
            agent: configure(AgentBuilder::new(self.gemini().completion(MODEL))).build(),
            model: MODEL.to_string(),
        }
    }
}

async fn serve(mut socket: tokio::net::TcpStream, reply: Reply, recorded: &Mutex<Vec<Value>>) {
    let mut received = Vec::new();
    let mut buffer = [0u8; 65536];
    let body_start = loop {
        let read = socket.read(&mut buffer).await.unwrap();
        assert!(read > 0, "the client closed the connection mid-request");
        received.extend_from_slice(&buffer[..read]);
        if let Some(at) = received.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let headers = String::from_utf8_lossy(&received[..body_start]).to_lowercase();
    let length: usize = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .map(|value| value.trim().parse().unwrap())
        .unwrap();
    while received.len() < body_start + length {
        let read = socket.read(&mut buffer).await.unwrap();
        assert!(read > 0, "the client closed the connection mid-request");
        received.extend_from_slice(&buffer[..read]);
    }
    recorded
        .lock()
        .unwrap()
        .push(serde_json::from_slice(&received[body_start..body_start + length]).unwrap());
    let (status, content_type, body) = match reply {
        Reply::Json(value) => (200, "application/json", value.to_string()),
        Reply::Sse(value) => (200, "text/event-stream", format!("data: {value}\r\n\r\n")),
        Reply::Status(status, value) => (status, "application/json", value.to_string()),
    };
    socket
        .write_all(
            format!(
                "HTTP/1.1 {status} X\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}

pub(crate) fn gemini_parts(request: &Value) -> Vec<Value> {
    request["contents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|content| content["parts"].as_array().unwrap().clone())
        .collect()
}

#[derive(Default)]
pub(crate) struct MemoryConversation {
    history: Mutex<Vec<u8>>,
    files: HashMap<Uuid, Base64Attachment>,
    pub turn_errors: Mutex<Vec<AiError>>,
    pub refuse_writes: AtomicBool,
}

impl MemoryConversation {
    pub(crate) fn with_file(mut self, id: Uuid, media_type: &str, bytes: &[u8]) -> Self {
        self.files.insert(
            id,
            Base64Attachment {
                media_type: media_type.to_string(),
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
        );
        self
    }

    pub(crate) fn history(&self) -> Vec<u8> {
        self.history.lock().unwrap().clone()
    }
}

impl ConversationProvider for MemoryConversation {
    async fn read_history(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.history())
    }

    async fn append_history(
        &self,
        expected_len: u64,
        bytes: &[u8],
    ) -> anyhow::Result<HistoryWrite> {
        let mut history = self.history.lock().unwrap();
        if history.len() as u64 != expected_len || self.refuse_writes.load(Ordering::SeqCst) {
            return Ok(HistoryWrite::Changed);
        }
        history.extend_from_slice(bytes);
        Ok(HistoryWrite::Written)
    }

    async fn fetch_attachments(&self, file_ids: &[Uuid]) -> anyhow::Result<Vec<Base64Attachment>> {
        file_ids
            .iter()
            .map(|id| {
                self.files
                    .get(id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("no file {id}"))
            })
            .collect()
    }

    async fn record_turn_error(&self, error: &AiError) -> anyhow::Result<()> {
        self.turn_errors.lock().unwrap().push(error.clone());
        Ok(())
    }

    async fn clear_turn_error(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct CountingLimit {
    pub admitted: AtomicUsize,
    charged: Mutex<Vec<(u64, u64)>>,
    pub released: AtomicUsize,
}

impl CountingLimit {
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        (
            self.admitted.load(Ordering::SeqCst),
            self.charged().len(),
            self.released.load(Ordering::SeqCst),
        )
    }

    pub(crate) fn charged(&self) -> Vec<(u64, u64)> {
        self.charged.lock().unwrap().clone()
    }
}

impl RateLimitProvider for CountingLimit {
    async fn pre_check(&self, _: u64) -> Result<(), RateLimitError> {
        self.admitted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn record_usage(&self, input_tokens: u64, output_tokens: u64) {
        self.charged
            .lock()
            .unwrap()
            .push((input_tokens, output_tokens));
    }

    async fn release(&self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

pub(crate) struct NoLimit;

impl RateLimitProvider for NoLimit {
    async fn pre_check(&self, _: u64) -> Result<(), RateLimitError> {
        Ok(())
    }

    async fn record_usage(&self, _: u64, _: u64) {}

    async fn release(&self) {}
}

#[derive(Default)]
pub(crate) struct FeeActions {
    pub fees: Mutex<Vec<String>>,
}

macro_rules! unexpected_actions {
    ($($name:ident($params:ident) -> $result:ident;)*) => {
        $(
            async fn $name(&self, _: $params) -> anyhow::Result<$result> {
                unreachable!("the test records fees only")
            }
        )*
    };
}

impl AiActionProvider for FeeActions {
    unexpected_actions! {
        create_transaction(CreateTransactionParams) -> CreateTransactionResult;
        group_transactions(GroupTransactionsParams) -> GroupTransactionsResult;
        create_custom_asset(CreateCustomAssetParams) -> CreateCustomAssetResult;
        record_asset_trade(RecordAssetTradeParams) -> RecordAssetTradeResult;
        record_transfer(RecordTransferParams) -> RecordTransferResult;
        record_cash_transfer(RecordCashTransferParams) -> RecordCashTransferResult;
        record_asset_transfer(RecordAssetTransferParams) -> RecordAssetTransferResult;
        record_asset_swap(RecordAssetSwapParams) -> RecordAssetSwapResult;
        update_asset_valuation(UpdateAssetValuationParams) -> UpdateAssetValuationResult;
        record_dividend(RecordDividendParams) -> RecordDividendResult;
        update_transaction(UpdateTransactionParams) -> UpdateTransactionResult;
        delete_transaction(DeleteTransactionParams) -> DeleteTransactionResult;
    }

    async fn record_fee(&self, params: RecordFeeParams) -> anyhow::Result<RecordFeeResult> {
        self.fees.lock().unwrap().push(params.amount.to_string());
        Ok(RecordFeeResult {
            transaction_id: Uuid::nil(),
            message: format!("Recorded a fee of {}", params.amount),
        })
    }
}

#[derive(Default)]
pub(crate) struct MemorySource {
    opened: AtomicUsize,
    stopped: bool,
}

impl MemorySource {
    pub(crate) fn stopped() -> Self {
        Self {
            stopped: true,
            ..Self::default()
        }
    }

    pub(crate) fn opened(&self) -> usize {
        self.opened.load(Ordering::SeqCst)
    }
}

impl ConversationSource for MemorySource {
    type Conversation = MemoryConversation;

    async fn open(&self) -> anyhow::Result<Option<Arc<MemoryConversation>>> {
        if self.stopped {
            return Ok(None);
        }
        self.opened.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Arc::new(MemoryConversation::default())))
    }
}
