use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use futures::{stream, StreamExt};
use rig::conversation::{Author, Entry};
use rig::message::Message;
use rig::typesafeai::types::Request;
use rig::typesafeai::Choice;
use serde_json::{json, Value};

use super::{category_label, grouping, kind_name, KINDS};
use crate::config::SystemOneConfig;
use crate::conversation_provider::{ConversationProvider, ConversationSource};
use crate::history;
use crate::models::chat::TokenUsage;
use crate::models::classification::{
    ClassificationCategory, ClassificationGroup, ClassificationLine, ClassificationOutcome,
    ClassificationRun, GroupClassification, LineClassification, SystemOneAnswers,
};
use crate::models::error::AiError;
use crate::provider::{system_one_model, SystemOneModel};

const CONCURRENCY: usize = 8;
const CALL_ATTEMPTS: u64 = 3;
const CONFIDENT_FROM: f64 = 0.9;
const MISCELLANEOUS_CATEGORY: i32 = 76;
const KIND_QUESTION: &str = "What kind of bank transaction is this group of statement lines? The description is the bank's own text and can be in any language.";
const CATEGORY_QUESTION: &str = "Which spending category would this transaction belong to if it were a purchase or payment? The description is the bank's own text and can be in any language.";

#[tracing::instrument(level = "debug", skip_all, fields(lines = lines.len(), model = %config.model))]
pub async fn system_one_classify<S>(
    config: &SystemOneConfig,
    conversations: &S,
    lines: &[ClassificationLine],
    categories: &[ClassificationCategory],
) -> anyhow::Result<ClassificationOutcome>
where
    S: ConversationSource,
{
    let (groups, line_groups) = grouping::group(lines);
    let (answers, usage) = if groups.is_empty() {
        (Vec::new(), TokenUsage::default())
    } else {
        let Some(conversation) = conversations.open().await? else {
            return Ok(ClassificationOutcome::Stopped);
        };
        run(system_one_model(config), conversation, &groups, categories).await?
    };
    Ok(ClassificationOutcome::Classified(ClassificationRun {
        lines: grouping::per_line(&line_groups, answers),
        groups: groups.len(),
        usage,
    }))
}

async fn run<C>(
    model: SystemOneModel,
    conversation: Arc<C>,
    groups: &[ClassificationGroup],
    categories: &[ClassificationCategory],
) -> Result<(Vec<GroupClassification>, TokenUsage), AiError>
where
    C: ConversationProvider,
{
    let category_keys = category_keys(categories);
    let questions = BTreeMap::from([
        (
            "kind".to_string(),
            Choice::<String>::try_from_iter(
                KIND_QUESTION,
                KINDS
                    .iter()
                    .map(|(kind, text)| (kind_name(*kind).to_string(), *text)),
            )?,
        ),
        (
            "category".to_string(),
            Choice::<String>::try_from_iter(
                CATEGORY_QUESTION,
                categories
                    .iter()
                    .zip(&category_keys)
                    .map(|(category, key)| (key.clone(), category_label(category))),
            )?,
        ),
    ]);
    let questions = serde_json::value::to_raw_value(&questions)
        .map_err(|error| AiError::unknown(error.to_string()))?;

    let states: Vec<(u32, Value)> = groups
        .iter()
        .map(|group| {
            let mut state = json!(group);
            if let Some(state) = state.as_object_mut() {
                state.remove("index");
            }
            (group.index, state)
        })
        .collect();
    let calls = states.into_iter().map(|(index, state)| {
        let (model, questions) = (&model, &questions);
        async move {
            let mut attempt = 1;
            let result = loop {
                let request = Request {
                    state: state.clone(),
                    questions: questions.clone(),
                };
                match model.call(request).await {
                    Err(_) if attempt < CALL_ATTEMPTS => {
                        tokio::time::sleep(Duration::from_secs(attempt)).await;
                        attempt += 1;
                    }
                    result => break result,
                }
            };
            (index, state, result)
        }
    });
    let answers: Vec<_> = stream::iter(calls).buffered(CONCURRENCY).collect().await;

    let mut history = history::load(&*conversation).await?;
    history.push(Message::user(questions.get().to_string()));
    let by_key: HashMap<&String, i32> = category_keys
        .iter()
        .zip(categories)
        .map(|(key, category)| (key, category.id))
        .collect();
    let mut classifications = Vec::with_capacity(answers.len());
    let mut usage = TokenUsage::default();
    let mut failure = None;
    for (index, state, result) in answers {
        history.push(Message::user(state.to_string()));
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                failure.get_or_insert(AiError::from(error));
                continue;
            }
        };
        let reported = response.usage.map(TokenUsage::from).unwrap_or_default();
        usage.input += reported.input;
        usage.output += reported.output;
        history.push(
            Entry::new(Message::assistant(response.answers.get().to_string()))
                .by(Author::agent(response.model, None)),
        );
        match serde_json::from_str::<SystemOneAnswers>(response.answers.get()) {
            Ok(answers) => classifications.extend(classify(index, &answers, &by_key)),
            Err(error) => {
                failure.get_or_insert(AiError::ProviderUnavailable {
                    detail: format!("the classification answer was not valid: {error}"),
                });
            }
        }
    }
    history::save(&*conversation, &mut history).await?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok((classifications, usage))
}

fn classify(
    index: u32,
    answers: &SystemOneAnswers,
    categories: &HashMap<&String, i32>,
) -> Option<GroupClassification> {
    let (kind, _) = KINDS
        .iter()
        .find(|(kind, _)| kind_name(*kind) == answers.kind.choice)?;
    let category_id = categories.get(&answers.category.choice).copied();
    let confidence = answers
        .category
        .probabilities
        .get(&answers.category.choice)
        .copied()
        .unwrap_or(0.0);
    Some(GroupClassification {
        index,
        classification: LineClassification {
            kind: *kind,
            category_id,
            confidence,
            confident: confidence >= CONFIDENT_FROM
                && category_id.is_some_and(|id| id != MISCELLANEOUS_CATEGORY),
        },
    })
}

fn category_keys(categories: &[ClassificationCategory]) -> Vec<String> {
    let mut named: HashMap<&str, usize> = HashMap::new();
    for category in categories {
        *named.entry(category.name.as_str()).or_default() += 1;
    }
    categories
        .iter()
        .map(|category| match named[category.name.as_str()] {
            1 => category.name.clone(),
            _ => format!("{} ({})", category.name, category.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use rig::typesafeai::JevConfig;

    use super::*;
    use crate::models::classification::{ClassifiedKind, MoneyDirection};
    use crate::test_support::{FakeModel, MemoryConversation, Reply};

    fn group() -> ClassificationGroup {
        ClassificationGroup {
            index: 7,
            description: "MAXIMA LT".to_string(),
            direction: MoneyDirection::Out,
            typical_amount: "18.20 EUR".to_string(),
            times_seen: 12,
            account: "LT12 3456".to_string(),
            includes_a_fee: false,
        }
    }

    fn categories() -> Vec<ClassificationCategory> {
        [(25, "Groceries"), (76, "Miscellaneous")]
            .into_iter()
            .map(|(id, name)| ClassificationCategory {
                id,
                type_name: "Everyday".to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    fn answer(category: &str, probability: f64) -> Reply {
        let other = if category == "Groceries" {
            "Miscellaneous"
        } else {
            "Groceries"
        };
        Reply::Json(json!({
            "model": "jev-1.14.0",
            "answers": {
                "kind": {"type": "choice", "choice": "expense", "confidence": 0.9,
                    "probabilities": {"expense": 0.97, "refund": 0.03}},
                "category": {"type": "choice", "choice": category, "confidence": 0.8,
                    "probabilities": {category: probability, other: 1.0 - probability}}
            },
            "usage": {"input_tokens": 2500, "output_tokens": 0}
        }))
    }

    async fn classify(
        reply: Reply,
    ) -> (
        (Vec<GroupClassification>, TokenUsage),
        FakeModel,
        Arc<MemoryConversation>,
    ) {
        classify_after(vec![reply]).await
    }

    async fn classify_after(
        replies: Vec<Reply>,
    ) -> (
        (Vec<GroupClassification>, TokenUsage),
        FakeModel,
        Arc<MemoryConversation>,
    ) {
        let model = FakeModel::start(replies).await;
        let conversation = Arc::new(MemoryConversation::default());
        let run = run(
            JevConfig::new("test-key")
                .with_endpoint(format!("{}/v1/systemone", model.url))
                .client()
                .evaluation(),
            conversation.clone(),
            &[group()],
            &categories(),
        )
        .await
        .unwrap();
        (run, model, conversation)
    }

    #[tokio::test]
    async fn a_likely_category_is_confident_and_every_call_is_kept() {
        let (run, model, conversation) = classify(answer("Groceries", 0.95)).await;

        assert_eq!(
            run.0,
            vec![GroupClassification {
                index: 7,
                classification: LineClassification {
                    kind: ClassifiedKind::Expense,
                    category_id: Some(25),
                    confidence: 0.95,
                    confident: true,
                },
            }]
        );
        assert_eq!(run.1.input, 2500);
        let sent: Value = model.requests()[0].clone();
        assert_eq!(sent["state"]["description"], "MAXIMA LT");
        assert!(sent["state"].get("index").is_none());
        let saved = history::load(&*conversation).await.unwrap();
        assert_eq!(saved.messages().len(), 3);
    }

    #[tokio::test]
    async fn a_failed_call_is_retried() {
        let (run, model, _) = classify_after(vec![
            Reply::Status(503, json!({"error": "unavailable"})),
            answer("Groceries", 0.95),
        ])
        .await;

        assert_eq!(model.requests().len(), 2);
        assert_eq!(run.0[0].classification.category_id, Some(25));
    }

    #[tokio::test]
    async fn miscellaneous_is_never_confident() {
        let (run, _, _) = classify(answer("Miscellaneous", 0.97)).await;

        assert_eq!(run.0[0].classification.category_id, Some(76));
        assert!(!run.0[0].classification.confident);
    }

    #[test]
    fn categories_sharing_a_name_get_distinct_keys() {
        let mut categories = categories();
        categories.push(ClassificationCategory {
            id: 901,
            type_name: "Mine".to_string(),
            name: "Groceries".to_string(),
        });

        assert_eq!(
            category_keys(&categories),
            vec!["Groceries (25)", "Miscellaneous", "Groceries (901)"]
        );
    }
}
