use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitScope {
    User,
    Global,
}

/// Total classification of every AI failure. Constructed only via the
/// `From` impls below and `AiError::unknown` — classification lives in
/// this file and nowhere else.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AiError {
    #[error("AI usage limit reached")]
    RateLimited {
        scope: LimitScope,
        reset_at: Option<OffsetDateTime>,
    },
    #[error("The AI provider is rate limiting requests")]
    ProviderRateLimited,
    #[error("The AI provider is unavailable: {detail}")]
    ProviderUnavailable { detail: String },
    #[error("Another AI request is already running")]
    ConcurrencyLimited,
    #[error("The assistant ran out of turns (limit {max_turns})")]
    TurnLimit { max_turns: u32 },
    #[error("Message is too large for a single request")]
    InputTooLarge,
    #[error("Attachment could not be loaded: {detail}")]
    InvalidAttachment { detail: String },
    #[error("Request cannot succeed: {detail}")]
    Fatal { detail: String },
    #[error("The model's answer did not match the requested structure: {detail}")]
    InvalidAnswer { detail: String },
    #[error("Unexpected AI error: {detail}")]
    Unknown { detail: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetryClass {
    Transient {
        min_delay_secs: Option<u64>,
    },
    RateLimited {
        reset_at: Option<OffsetDateTime>,
        scope: LimitScope,
    },
    Terminal,
}

impl AiError {
    /// Every `Unknown` is loud: it is the signal to add a classifier arm.
    pub fn unknown(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        tracing::error!(detail = %detail, error.type = "AiError::Unknown", "Unclassified AI error");
        Self::Unknown { detail }
    }

    pub fn retry_class(&self) -> RetryClass {
        match self {
            Self::RateLimited { scope, reset_at } => RetryClass::RateLimited {
                reset_at: *reset_at,
                scope: *scope,
            },
            Self::ProviderRateLimited => RetryClass::Transient {
                min_delay_secs: None,
            },
            Self::ConcurrencyLimited => RetryClass::Transient {
                min_delay_secs: Some(5),
            },
            Self::ProviderUnavailable { .. }
            | Self::InvalidAnswer { .. }
            | Self::Unknown { .. }
            | Self::InvalidAttachment { .. } => RetryClass::Transient {
                min_delay_secs: None,
            },
            Self::TurnLimit { .. } | Self::InputTooLarge | Self::Fatal { .. } => {
                RetryClass::Terminal
            }
        }
    }
}

fn classify_status(status: u16, detail: &str) -> Option<AiError> {
    match status {
        429 => Some(AiError::ProviderRateLimited),
        500..=599 => Some(AiError::ProviderUnavailable {
            detail: detail.to_string(),
        }),
        400 | 401 | 403 | 404 => Some(AiError::Fatal {
            detail: detail.to_string(),
        }),
        _ => None,
    }
}

fn classify_code(code: &str, detail: &str) -> Option<AiError> {
    match code {
        "RESOURCE_EXHAUSTED" => Some(AiError::ProviderRateLimited),
        "UNAVAILABLE" | "DEADLINE_EXCEEDED" | "INTERNAL" => Some(AiError::ProviderUnavailable {
            detail: detail.to_string(),
        }),
        "INVALID_ARGUMENT"
        | "FAILED_PRECONDITION"
        | "PERMISSION_DENIED"
        | "UNAUTHENTICATED"
        | "NOT_FOUND" => Some(AiError::Fatal {
            detail: detail.to_string(),
        }),
        _ => None,
    }
}

fn classify_report(report: &rig::error::ErrorReport) -> AiError {
    let detail = rig::redact::text(&report.message).into_owned();
    if report.refusal {
        return AiError::Fatal { detail };
    }
    report
        .http_status
        .and_then(|status| classify_status(status, &detail))
        .or_else(|| {
            report
                .code
                .as_deref()
                .and_then(|code| classify_code(code, &detail))
        })
        .unwrap_or_else(|| {
            if report.retryable {
                AiError::ProviderUnavailable { detail }
            } else if report.kind == rig::error::ErrorKind::Response {
                AiError::Fatal { detail }
            } else {
                AiError::unknown(detail)
            }
        })
}

impl From<rig::error::ErrorReport> for AiError {
    fn from(report: rig::error::ErrorReport) -> Self {
        classify_report(&report)
    }
}

impl From<rig::error::ProviderError> for AiError {
    fn from(e: rig::error::ProviderError) -> Self {
        classify_report(&e.report())
    }
}

impl From<rig::completion::PromptError> for AiError {
    fn from(e: rig::completion::PromptError) -> Self {
        use rig::completion::PromptError as PE;
        match e {
            PE::Report(report) => report.into(),
            PE::Provider(error) => error.into(),
            PE::MaxTurns { max_turns, .. } => AiError::TurnLimit {
                max_turns: u32::try_from(max_turns).unwrap_or(u32::MAX),
            },
            other => AiError::unknown(other.to_string()),
        }
    }
}

impl From<rig::memory::SessionError> for AiError {
    fn from(e: rig::memory::SessionError) -> Self {
        use rig::memory::SessionError as SE;
        match e {
            SE::OldLayout | SE::Format { .. } | SE::Record { .. } => AiError::Fatal {
                detail: e.to_string(),
            },
            SE::Changed { .. } => AiError::ConcurrencyLimited,
            SE::Io(_) => AiError::unknown(e.to_string()),
        }
    }
}

impl From<crate::rate_limit_provider::RateLimitError> for AiError {
    fn from(e: crate::rate_limit_provider::RateLimitError) -> Self {
        use crate::rate_limit_provider::RateLimitError as RL;
        match e {
            RL::QuotaExceeded { scope, reset_at } => AiError::RateLimited { scope, reset_at },
            RL::ConcurrencyLimit => AiError::ConcurrencyLimited,
            RL::PerRequestCap => AiError::InputTooLarge,
            RL::Other(e) => AiError::unknown(format!("{e:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::completion::PromptError;
    use rig::error::ProviderError;
    use rig::http_client::StatusCode;

    #[test]
    fn max_turns_maps_to_turn_limit() {
        let e = PromptError::MaxTurns {
            max_turns: 5,
            chat_history: vec![],
            prompt: rig::message::Message::user("x"),
        };
        assert_eq!(AiError::from(e), AiError::TurnLimit { max_turns: 5 });
    }

    #[test]
    fn provider_resource_exhausted_maps_to_provider_rate_limited() {
        let e = ProviderError::from_provider_body(
            r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"quota"}}"#,
        );
        assert_eq!(AiError::from(e), AiError::ProviderRateLimited);
    }

    #[test]
    fn provider_unavailable_maps_to_provider_unavailable() {
        let e = ProviderError::from_provider_body(
            r#"{"error":{"code":503,"status":"UNAVAILABLE","message":"The model is overloaded"}}"#,
        );
        assert!(matches!(
            AiError::from(e),
            AiError::ProviderUnavailable { .. }
        ));
    }

    #[test]
    fn http_status_503_maps_to_provider_unavailable() {
        let e = ProviderError::from_http_response(StatusCode::SERVICE_UNAVAILABLE, "");
        assert!(matches!(
            AiError::from(e),
            AiError::ProviderUnavailable { .. }
        ));
    }

    #[test]
    fn http_status_429_maps_to_provider_rate_limited() {
        let e = ProviderError::from_http_response(StatusCode::TOO_MANY_REQUESTS, "");
        assert!(matches!(AiError::from(e), AiError::ProviderRateLimited));
    }

    #[test]
    fn unrecognized_provider_error_maps_to_unknown() {
        let e = ProviderError::Provider("some brand new failure mode".into());
        assert!(matches!(AiError::from(e), AiError::Unknown { .. }));
    }

    #[test]
    fn a_refused_prompt_is_fatal() {
        let mut report = ProviderError::Provider("blocked".into()).report();
        report.refusal = true;
        assert!(matches!(AiError::from(report), AiError::Fatal { .. }));
    }

    #[test]
    fn transport_errors_do_not_carry_the_api_key() {
        let mut report = ProviderError::Truncated.report();
        report.message = "error sending request for url (https://x/v1:generate?alt=sse&key=AIzaSyA1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q)".into();

        let AiError::ProviderUnavailable { detail } = AiError::from(report) else {
            panic!("expected a transient provider error");
        };
        assert!(!detail.contains("AIzaSy"), "{detail}");
    }

    #[test]
    fn a_turn_the_model_ended_without_an_answer_is_fatal() {
        let e = ProviderError::Response("the model stopped: SAFETY".into());
        assert!(matches!(AiError::from(e), AiError::Fatal { .. }));
    }

    #[test]
    fn quota_exceeded_maps_to_rate_limited_preserving_scope() {
        let e = crate::rate_limit_provider::RateLimitError::QuotaExceeded {
            scope: LimitScope::Global,
            reset_at: None,
        };
        assert_eq!(
            AiError::from(e),
            AiError::RateLimited {
                scope: LimitScope::Global,
                reset_at: None
            }
        );
    }

    #[test]
    fn per_request_cap_is_terminal() {
        let e = crate::rate_limit_provider::RateLimitError::PerRequestCap;
        let err = AiError::from(e);
        assert_eq!(err, AiError::InputTooLarge);
        assert!(matches!(err.retry_class(), RetryClass::Terminal));
    }

    #[test]
    fn retry_classes() {
        assert!(matches!(
            AiError::TurnLimit { max_turns: 5 }.retry_class(),
            RetryClass::Terminal
        ));
        assert!(matches!(
            AiError::unknown("x").retry_class(),
            RetryClass::Transient { .. }
        ));
        assert!(matches!(
            AiError::ProviderRateLimited.retry_class(),
            RetryClass::Transient {
                min_delay_secs: None
            }
        ));
    }

    #[test]
    fn invalid_argument_with_numbers_is_fatal_not_transient() {
        let e = ProviderError::from_provider_body(
            r#"{"error":{"code":400,"status":"INVALID_ARGUMENT","message":"input exceeds the maximum number of tokens allowed (500000)"}}"#,
        );
        assert!(matches!(AiError::from(e), AiError::Fatal { .. }));
    }

    #[test]
    fn permission_denied_is_fatal() {
        let e = ProviderError::from_provider_body(
            r#"{"error":{"code":403,"status":"PERMISSION_DENIED","message":"API key not valid"}}"#,
        );
        assert!(matches!(AiError::from(e), AiError::Fatal { .. }));
    }

    #[test]
    fn http_401_is_fatal_terminal() {
        let e = ProviderError::from_http_response(StatusCode::UNAUTHORIZED, "");
        let err = AiError::from(e);
        assert!(matches!(err, AiError::Fatal { .. }));
        assert!(matches!(err.retry_class(), RetryClass::Terminal));
    }

    #[test]
    fn concurrency_limit_maps_to_dedicated_variant() {
        let e = crate::rate_limit_provider::RateLimitError::ConcurrencyLimit;
        let err = AiError::from(e);
        assert_eq!(err, AiError::ConcurrencyLimited);
        assert!(matches!(
            err.retry_class(),
            RetryClass::Transient {
                min_delay_secs: Some(5)
            }
        ));
    }
}
