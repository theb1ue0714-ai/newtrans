//! The three-step translation chain.
//!
//! 1. **원문** in language L1 — detected automatically or pinned by the user.
//! 2. **번역** into L2 — Korean by default, pinned by the user.
//! 3. **재번역** back into L1, so the user can check whether the meaning
//!    survived the round trip.
//!
//! When L1 and L2 would be the same (a Korean selection with a Korean
//! target), step 2 uses the fallback language (English by default) instead.

use crate::lang::{SourceChoice, normalize};

/// What to send for step 2 before the provider has detected anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstStep {
    /// `None` asks the provider to detect the source language.
    pub source: Option<String>,
    pub target: String,
}

pub fn plan_first_step(
    choice: &SourceChoice,
    local_guess: Option<&str>,
    target: &str,
    fallback: &str,
) -> FirstStep {
    let target = normalize(target);
    let fallback = normalize(fallback);
    match choice {
        SourceChoice::Fixed(code) => {
            let source = normalize(code);
            let target = if source == target { fallback } else { target };
            FirstStep {
                source: Some(source),
                target,
            }
        }
        SourceChoice::Auto => match local_guess.map(normalize) {
            Some(guess) if guess == target => FirstStep {
                source: Some(guess),
                target: fallback,
            },
            _ => FirstStep {
                source: None,
                target,
            },
        },
    }
}

/// After step 2 returns, decide whether it must be repeated because the
/// provider detected that the text was already in the target language.
/// Returns the corrected target, or `None` when step 2 is fine as is.
pub fn retarget_after_detection(
    detected: &str,
    used_target: &str,
    fallback: &str,
) -> Option<String> {
    let detected = normalize(detected);
    let fallback = normalize(fallback);
    (detected == normalize(used_target) && detected != fallback).then_some(fallback)
}

/// Step 3 translates back into the source language unless that is
/// pointless (unknown source, or identical to the intermediate language).
pub fn back_translation_target(source: &str, intermediate: &str) -> Option<String> {
    let source = normalize(source);
    (!source.is_empty() && source != "auto" && source != normalize(intermediate)).then_some(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(code: &str) -> SourceChoice {
        SourceChoice::Fixed(code.to_owned())
    }

    #[test]
    fn auto_english_goes_to_korean() {
        assert_eq!(
            plan_first_step(&SourceChoice::Auto, None, "ko", "en"),
            FirstStep {
                source: None,
                target: "ko".into()
            }
        );
    }

    #[test]
    fn auto_korean_goes_to_fallback() {
        assert_eq!(
            plan_first_step(&SourceChoice::Auto, Some("ko"), "ko", "en"),
            FirstStep {
                source: Some("ko".into()),
                target: "en".into()
            }
        );
    }

    #[test]
    fn auto_japanese_goes_to_korean() {
        assert_eq!(
            plan_first_step(&SourceChoice::Auto, Some("ja"), "ko", "en"),
            FirstStep {
                source: None,
                target: "ko".into()
            }
        );
    }

    #[test]
    fn pinned_languages_are_respected() {
        assert_eq!(
            plan_first_step(&fixed("de"), None, "ja", "en"),
            FirstStep {
                source: Some("de".into()),
                target: "ja".into()
            }
        );
        assert_eq!(
            plan_first_step(&fixed("ko"), None, "ko", "en"),
            FirstStep {
                source: Some("ko".into()),
                target: "en".into()
            }
        );
    }

    #[test]
    fn retargets_when_provider_detects_target_language() {
        assert_eq!(
            retarget_after_detection("ko", "ko", "en").as_deref(),
            Some("en")
        );
        assert_eq!(retarget_after_detection("en", "ko", "en"), None);
        assert_eq!(retarget_after_detection("en", "en", "en"), None);
    }

    #[test]
    fn back_translation_returns_to_source() {
        assert_eq!(back_translation_target("en", "ko").as_deref(), Some("en"));
        assert_eq!(
            back_translation_target("EN-US", "ko").as_deref(),
            Some("en")
        );
        assert_eq!(back_translation_target("ko", "ko"), None);
        assert_eq!(back_translation_target("auto", "ko"), None);
    }
}
