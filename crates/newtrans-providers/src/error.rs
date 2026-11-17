use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ProviderError {
    #[error("API 키가 등록되지 않았습니다. 설정에서 키를 입력하세요.")]
    MissingKey,
    #[error("API 키가 올바르지 않거나 권한이 없습니다.")]
    InvalidKey,
    #[error("이번 달 무료 사용량을 모두 사용했습니다.")]
    QuotaExceeded,
    #[error("요청이 너무 많습니다. 잠시 후 다시 시도하세요.")]
    RateLimited,
    #[error("이 서비스는 해당 언어를 지원하지 않습니다: {0}")]
    UnsupportedLanguage(String),
    #[error("응답 시간이 초과되었습니다.")]
    Timeout,
    #[error("네트워크에 연결할 수 없습니다.")]
    Network,
    #[error("서비스 오류가 발생했습니다 (HTTP {0}).")]
    Upstream(u16),
    #[error("서비스 응답 형식을 해석하지 못했습니다.")]
    BadResponse,
}

impl ProviderError {
    pub(crate) fn from_reqwest(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else if let Some(status) = error.status() {
            Self::from_status(status.as_u16(), "")
        } else if error.is_decode() {
            Self::BadResponse
        } else {
            Self::Network
        }
    }

    /// Maps HTTP status codes; the body is inspected only for well-known
    /// markers and never copied into the error (it may echo user text).
    pub(crate) fn from_status(status: u16, body: &str) -> Self {
        let lower = body.to_ascii_lowercase();
        match status {
            401 | 403 => Self::InvalidKey,
            456 => Self::QuotaExceeded,
            429 if lower.contains("quota") => Self::QuotaExceeded,
            429 => Self::RateLimited,
            400 if lower.contains("lang")
                || lower.contains("n2mt05")
                || lower.contains("n2mt06") =>
            {
                Self::UnsupportedLanguage(String::new())
            }
            _ => Self::Upstream(status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_statuses() {
        assert_eq!(
            ProviderError::from_status(403, ""),
            ProviderError::InvalidKey
        );
        assert_eq!(
            ProviderError::from_status(456, ""),
            ProviderError::QuotaExceeded
        );
        assert_eq!(
            ProviderError::from_status(429, "Quota exceeded"),
            ProviderError::QuotaExceeded
        );
        assert_eq!(
            ProviderError::from_status(429, ""),
            ProviderError::RateLimited
        );
        assert_eq!(
            ProviderError::from_status(500, ""),
            ProviderError::Upstream(500)
        );
        assert!(matches!(
            ProviderError::from_status(400, "Value for 'target_lang' not supported."),
            ProviderError::UnsupportedLanguage(_)
        ));
    }

    #[test]
    fn serializes_for_ui() {
        let json = serde_json::to_string(&ProviderError::Upstream(502)).unwrap();
        assert_eq!(json, r#"{"kind":"upstream","detail":502}"#);
    }
}
