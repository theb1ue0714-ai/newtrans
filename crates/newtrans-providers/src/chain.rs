//! Runs 원문 → 번역 → 재번역 against one provider.

use std::future::Future;

use newtrans_core::chain::{back_translation_target, plan_first_step, retarget_after_detection};
use newtrans_core::lang::{SourceChoice, normalize};
use newtrans_core::text::guess_language;
use serde::Serialize;

use crate::{ProviderError, Translation};

#[derive(Debug, Clone)]
pub struct ChainRequest {
    pub text: String,
    pub source: SourceChoice,
    pub target: String,
    pub fallback: String,
    pub back_translate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainResult {
    /// Language 1, as pinned or detected. `None` if nobody could tell.
    pub source_lang: Option<String>,
    /// Language 2 actually used (may be the fallback).
    pub target_lang: String,
    pub translation: String,
    /// Language 3 (= language 1) when the back translation ran.
    pub back_lang: Option<String>,
    pub back_translation: Option<String>,
    /// A failed back translation keeps the main result visible.
    pub back_error: Option<ProviderError>,
}

/// `translate(text, source, target)` performs one provider call.
pub async fn run_chain<F, Fut>(
    request: &ChainRequest,
    translate: F,
) -> Result<ChainResult, ProviderError>
where
    F: Fn(String, Option<String>, String) -> Fut,
    Fut: Future<Output = Result<Translation, ProviderError>>,
{
    let guess = guess_language(&request.text);
    let first = plan_first_step(&request.source, guess, &request.target, &request.fallback);
    let mut target = first.target.clone();
    let mut result = translate(request.text.clone(), first.source.clone(), target.clone()).await?;
    let mut source_lang = first
        .source
        .clone()
        .or_else(|| result.detected.clone())
        .map(|c| normalize(&c));

    if let Some(detected) = source_lang.clone()
        && let Some(new_target) = retarget_after_detection(&detected, &target, &request.fallback)
    {
        result = translate(request.text.clone(), Some(detected), new_target.clone()).await?;
        target = new_target;
    }
    if source_lang.is_none() {
        source_lang = result.detected.clone();
    }

    let mut chain = ChainResult {
        source_lang: source_lang.clone(),
        target_lang: target.clone(),
        translation: result.text.clone(),
        back_lang: None,
        back_translation: None,
        back_error: None,
    };
    if request.back_translate
        && !result.text.trim().is_empty()
        && let Some(back) = source_lang
            .as_deref()
            .and_then(|s| back_translation_target(s, &target))
    {
        chain.back_lang = Some(back.clone());
        match translate(result.text, Some(target), back).await {
            Ok(back_result) => chain.back_translation = Some(back_result.text),
            Err(error) => chain.back_error = Some(error),
        }
    }
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    type Calls = Arc<Mutex<Vec<(Option<String>, String)>>>;

    /// Fake provider: "detects" English unless told otherwise and tags
    /// output with the target language.
    fn fake(
        calls: Calls,
        detected: &'static str,
    ) -> impl Fn(String, Option<String>, String) -> std::future::Ready<Result<Translation, ProviderError>>
    {
        move |text, source, target| {
            calls.lock().unwrap().push((source.clone(), target.clone()));
            std::future::ready(Ok(Translation {
                text: format!("[{target}]{text}"),
                detected: Some(source.unwrap_or_else(|| detected.to_owned())),
            }))
        }
    }

    fn request(text: &str, source: SourceChoice, target: &str) -> ChainRequest {
        ChainRequest {
            text: text.into(),
            source,
            target: target.into(),
            fallback: "en".into(),
            back_translate: true,
        }
    }

    #[tokio::test]
    async fn english_goes_to_korean_and_back() {
        let calls = Calls::default();
        let result = run_chain(
            &request("Hello", SourceChoice::Auto, "ko"),
            fake(calls.clone(), "en"),
        )
        .await
        .unwrap();
        assert_eq!(result.source_lang.as_deref(), Some("en"));
        assert_eq!(result.target_lang, "ko");
        assert_eq!(result.translation, "[ko]Hello");
        assert_eq!(result.back_lang.as_deref(), Some("en"));
        assert_eq!(result.back_translation.as_deref(), Some("[en][ko]Hello"));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![(None, "ko".into()), (Some("ko".into()), "en".into())]
        );
    }

    #[tokio::test]
    async fn korean_selection_uses_fallback_without_wasted_call() {
        let calls = Calls::default();
        let result = run_chain(
            &request("안녕하세요", SourceChoice::Auto, "ko"),
            fake(calls.clone(), "ko"),
        )
        .await
        .unwrap();
        assert_eq!(result.target_lang, "en");
        assert_eq!(result.back_lang.as_deref(), Some("ko"));
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn retries_when_provider_detects_target_language() {
        // Latin-script Korean romanization guessed as unknown locally but
        // detected as Korean by the provider.
        let calls = Calls::default();
        let result = run_chain(
            &request("annyeong", SourceChoice::Auto, "ko"),
            fake(calls.clone(), "ko"),
        )
        .await
        .unwrap();
        assert_eq!(result.target_lang, "en");
        assert_eq!(calls.lock().unwrap()[1], (Some("ko".into()), "en".into()));
        assert_eq!(calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn pinned_pair_is_used_verbatim() {
        let calls = Calls::default();
        let result = run_chain(
            &request("Guten Tag", SourceChoice::Fixed("de".into()), "ja"),
            fake(calls.clone(), "en"),
        )
        .await
        .unwrap();
        assert_eq!(result.source_lang.as_deref(), Some("de"));
        assert_eq!(result.target_lang, "ja");
        assert_eq!(result.back_lang.as_deref(), Some("de"));
    }

    #[tokio::test]
    async fn back_translation_can_be_disabled_or_fail_softly() {
        let calls = Calls::default();
        let mut req = request("Hello", SourceChoice::Auto, "ko");
        req.back_translate = false;
        let result = run_chain(&req, fake(calls.clone(), "en")).await.unwrap();
        assert!(result.back_translation.is_none());
        assert_eq!(calls.lock().unwrap().len(), 1);

        let failing_back = |text: String, source: Option<String>, target: String| {
            std::future::ready(if source.is_some() {
                Err(ProviderError::RateLimited)
            } else {
                Ok(Translation {
                    text: format!("[{target}]{text}"),
                    detected: Some("en".into()),
                })
            })
        };
        let result = run_chain(&request("Hello", SourceChoice::Auto, "ko"), failing_back)
            .await
            .unwrap();
        assert_eq!(result.translation, "[ko]Hello");
        assert_eq!(result.back_error, Some(ProviderError::RateLimited));
    }
}
