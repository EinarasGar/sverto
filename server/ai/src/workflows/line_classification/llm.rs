use std::collections::HashSet;
use std::sync::Arc;

use rig::agent::AgentBuilder;
use rig::completion::{Effort, OnUnsupported};

use super::{category_label, fits, grouping, kind_name, KINDS};
use crate::config::AiConfig;
use crate::conversation::Conversation;
use crate::conversation_provider::{ConversationProvider, ConversationSource};
use crate::models::agent::StructuredAgent;
use crate::models::chat::{ChatTurn, Persistence, StructuredOutput, TokenUsage};
use crate::models::classification::{
    ClassificationCategory, ClassificationGroup, ClassificationLine, ClassificationOutcome,
    ClassificationRun, CompactClassification, GroupClassification, LineClassification,
    MoneyDirection,
};
use crate::models::error::AiError;
use crate::provider::completion_model;
use crate::rate_limit_provider::RateLimitProvider;

const BATCH_SIZE: usize = 40;
const MAX_OUTPUT_TOKENS: u64 = 16384;
const MAX_CONFIDENCE: u32 = 9;
const CONFIDENT_FROM: u32 = 7;

#[tracing::instrument(level = "debug", skip_all, fields(lines = lines.len()))]
pub async fn llm_classify<S, R>(
    config: &AiConfig,
    conversations: &S,
    rate_limit: Arc<R>,
    lines: &[ClassificationLine],
    categories: &[ClassificationCategory],
) -> anyhow::Result<ClassificationOutcome>
where
    S: ConversationSource,
    R: RateLimitProvider,
{
    let preamble = system_prompt(categories);
    classify(
        || {
            StructuredAgent::new(
                AgentBuilder::new(completion_model(config))
                    .preamble(&preamble)
                    .max_tokens(MAX_OUTPUT_TOKENS)
                    .reasoning(Effort::Low)
                    .temperature(0.0)
                    .on_unsupported(OnUnsupported::Ignore),
                config.model.clone(),
            )
        },
        conversations,
        rate_limit,
        lines,
        categories,
    )
    .await
}

async fn classify<S, R>(
    agent: impl Fn() -> StructuredAgent<CompactClassification>,
    conversations: &S,
    rate_limit: Arc<R>,
    lines: &[ClassificationLine],
    categories: &[ClassificationCategory],
) -> anyhow::Result<ClassificationOutcome>
where
    S: ConversationSource,
    R: RateLimitProvider,
{
    let (groups, line_groups) = grouping::group(lines);
    let mut answers = Vec::with_capacity(groups.len());
    let mut usage = TokenUsage::default();
    for batch in groups.chunks(BATCH_SIZE) {
        let Some(conversation) = conversations.open().await? else {
            return Ok(ClassificationOutcome::Stopped);
        };
        let (batch_answers, batch_usage) =
            run(agent(), conversation, rate_limit.clone(), batch, categories).await?;
        answers.extend(batch_answers);
        usage.input += batch_usage.input;
        usage.output += batch_usage.output;
    }
    Ok(ClassificationOutcome::Classified(ClassificationRun {
        lines: grouping::per_line(&line_groups, answers),
        groups: groups.len(),
        usage,
    }))
}

async fn run<C, R>(
    agent: StructuredAgent<CompactClassification>,
    conversation: Arc<C>,
    rate_limit: Arc<R>,
    groups: &[ClassificationGroup],
    categories: &[ClassificationCategory],
) -> Result<(Vec<GroupClassification>, TokenUsage), AiError>
where
    C: ConversationProvider,
    R: RateLimitProvider,
{
    let message = serde_json::to_string_pretty(groups)
        .map_err(|error| AiError::unknown(error.to_string()))?;
    let StructuredOutput {
        value: answer,
        usage,
    } = Conversation::new(conversation, rate_limit)
        .run_structured(
            agent,
            ChatTurn::Message {
                message,
                file_ids: Vec::new(),
            },
            Persistence::Persist,
        )
        .await?;
    let indices: HashSet<u32> = groups.iter().map(|group| group.index).collect();
    let offered: HashSet<i32> = categories.iter().map(|category| category.id).collect();
    let mut answered = HashSet::new();
    let answers = answer
        .rows
        .into_iter()
        .filter_map(|row| {
            let [index, kind, category_id, confidence] = row[..] else {
                return None;
            };
            let (kind, _) = KINDS.get(kind as usize)?;
            let category_id = i32::try_from(category_id)
                .ok()
                .filter(|id| offered.contains(id));
            if !indices.contains(&index) || confidence > MAX_CONFIDENCE {
                return None;
            }
            answered.insert(index).then_some(GroupClassification {
                index,
                classification: LineClassification {
                    kind: *kind,
                    category_id,
                    confidence: f64::from(confidence) / f64::from(MAX_CONFIDENCE),
                    confident: confidence >= CONFIDENT_FROM,
                },
            })
        })
        .collect();
    Ok((answers, usage))
}

fn system_prompt(categories: &[ClassificationCategory]) -> String {
    let kinds: Vec<String> = KINDS
        .iter()
        .map(|(kind, text)| format!("- {}: {text}", kind_name(*kind)))
        .collect();
    let codes: Vec<String> = KINDS
        .iter()
        .enumerate()
        .map(|(number, (kind, _))| format!("{number} = {}", kind_name(*kind)))
        .collect();
    let categories: Vec<String> = categories
        .iter()
        .map(|category| format!("{}: {}", category.id, category_label(category)))
        .collect();
    let kinds_of = |direction: MoneyDirection| -> String {
        KINDS
            .iter()
            .filter(|(kind, _)| fits(*kind, direction))
            .map(|(kind, _)| kind_name(*kind))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "You classify bank-statement transactions for a personal finance app. Each item is a group of statement lines \
that share the same description and direction. Descriptions are the bank's own text and can be in any language.\n\n\
For every item decide:\n\
1. kind, one of:\n{}\n\
   The item's direction limits the kind. For \"money out\": {}. For \"money in\": {}.\n\
2. category_id: the spending category this transaction would belong to if it were a purchase or payment. Always answer, whatever the kind; use 0 only when no category could fit.\n\
3. confidence: 0 (a guess) to 9 (certain), for your answers together.\n\n\
Categories (id: type › name):\n{}\n\n\
Answer every item exactly once, keyed by its index, with one row per item: [index, kind, category_id, confidence], where kind is a number: {}.",
        kinds.join("\n"),
        kinds_of(MoneyDirection::Out),
        kinds_of(MoneyDirection::In),
        categories.join("\n"),
        codes.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::models::classification::ClassifiedKind;
    use crate::test_support::{
        gemini_parts, CountingLimit, FakeModel, MemoryConversation, MemorySource, Reply,
    };

    fn group(index: u32, description: &str) -> ClassificationGroup {
        ClassificationGroup {
            index,
            description: description.to_string(),
            direction: MoneyDirection::Out,
            typical_amount: "12.40 GBP".to_string(),
            times_seen: 3,
            account: "Current".to_string(),
            includes_a_fee: false,
        }
    }

    fn categories() -> Vec<ClassificationCategory> {
        vec![
            ClassificationCategory {
                id: 25,
                type_name: "Food & Dining".to_string(),
                name: "Groceries".to_string(),
            },
            ClassificationCategory {
                id: 900,
                type_name: "Hobbies".to_string(),
                name: "Climbing".to_string(),
            },
        ]
    }

    fn answer(rows: Value) -> Reply {
        Reply::Json(json!({
            "candidates": [{"content": {"role": "model", "parts": [
                {"text": json!({ "rows": rows }).to_string()}
            ]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 900, "candidatesTokenCount": 40, "totalTokenCount": 940}
        }))
    }

    async fn classify(
        rows: Value,
        groups: &[ClassificationGroup],
    ) -> (Vec<GroupClassification>, TokenUsage, FakeModel) {
        let model = FakeModel::start(vec![answer(rows)]).await;
        let agent = model.structured(|builder| builder.preamble(system_prompt(&categories())));
        let (answers, usage) = run(
            agent,
            Arc::new(MemoryConversation::default()),
            Arc::new(CountingLimit::default()),
            groups,
            &categories(),
        )
        .await
        .unwrap();
        (answers, usage, model)
    }

    #[tokio::test]
    async fn each_row_becomes_a_classification_of_its_group() {
        let (answers, usage, model) = classify(
            json!([[0, 0, 25, 9], [1, 3, 0, 5]]),
            &[group(0, "TESCO STORES"), group(1, "ACME LTD SALARY")],
        )
        .await;

        assert_eq!(
            answers,
            vec![
                GroupClassification {
                    index: 0,
                    classification: LineClassification {
                        kind: ClassifiedKind::Expense,
                        category_id: Some(25),
                        confidence: 1.0,
                        confident: true,
                    },
                },
                GroupClassification {
                    index: 1,
                    classification: LineClassification {
                        kind: ClassifiedKind::TransferIn,
                        category_id: None,
                        confidence: 5.0 / 9.0,
                        confident: false,
                    },
                },
            ]
        );
        assert_eq!((usage.input, usage.output), (900, 40));
        let sent: Vec<Value> = serde_json::from_str(
            gemini_parts(&model.requests()[0])[0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(sent[1]["description"], "ACME LTD SALARY");
        assert_eq!(sent[1]["direction"], "money out");
    }

    #[tokio::test]
    async fn malformed_rows_are_dropped() {
        let (answers, _, _) = classify(
            json!([
                [0, 0, 25],
                [1, 7, 25, 9],
                [3, 0, 25, 9],
                [0, 0, 25, 10],
                [4, 1, 900, 8],
                [4, 0, 25, 9]
            ]),
            &[group(0, "A"), group(1, "B"), group(4, "E")],
        )
        .await;

        assert_eq!(
            answers,
            vec![GroupClassification {
                index: 4,
                classification: LineClassification {
                    kind: ClassifiedKind::Refund,
                    category_id: Some(900),
                    confidence: 8.0 / 9.0,
                    confident: true,
                },
            }]
        );
    }

    fn line(description: Option<&str>, amount: &str) -> ClassificationLine {
        ClassificationLine {
            description: description.map(str::to_string),
            amount: amount.parse().unwrap(),
            currency: "GBP".to_string(),
            account: "Current".to_string(),
            has_fee: false,
        }
    }

    #[tokio::test]
    async fn each_line_gets_its_merchant_answer_from_one_batch() {
        let model = FakeModel::start(vec![answer(json!([[0, 0, 25, 9]]))]).await;
        let source = MemorySource::default();
        let lines = [
            line(Some("TESCO 1120"), "-3.10"),
            line(None, "-9.99"),
            line(Some("tesco 1120"), "-12.40"),
        ];

        let outcome = super::classify(
            || model.structured(|builder| builder),
            &source,
            Arc::new(CountingLimit::default()),
            &lines,
            &categories(),
        )
        .await
        .unwrap();

        let ClassificationOutcome::Classified(run) = outcome else {
            panic!("an open source should let the run finish");
        };

        let groceries = Some(LineClassification {
            kind: ClassifiedKind::Expense,
            category_id: Some(25),
            confidence: 1.0,
            confident: true,
        });
        assert_eq!(run.lines, vec![groceries.clone(), None, groceries]);
        assert_eq!(run.groups, 1);
        assert_eq!(source.opened(), 1);
    }

    #[tokio::test]
    async fn a_source_that_stops_ends_the_run_before_any_model_call() {
        let model = FakeModel::start(Vec::new()).await;
        let source = MemorySource::stopped();

        let outcome = super::classify(
            || model.structured(|builder| builder),
            &source,
            Arc::new(CountingLimit::default()),
            &[line(Some("TESCO"), "-3")],
            &categories(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, ClassificationOutcome::Stopped));
        assert!(model.requests().is_empty());
    }

    #[test]
    fn the_rows_schema_has_no_fixed_length_arrays() {
        let schema = serde_json::to_value(schemars::schema_for!(CompactClassification)).unwrap();

        assert!(!schema.to_string().contains("minItems"));
    }
}
