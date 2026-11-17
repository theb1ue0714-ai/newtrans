//! Language catalog. Codes are lowercase BCP 47 tags as used by Google
//! Translate; providers map them to their own codes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Language {
    pub code: &'static str,
    /// Korean display name used by the UI.
    pub name: &'static str,
    pub native: &'static str,
}

/// Languages enabled on a fresh install.
pub const DEFAULT_ENABLED: [&str; 6] = ["ko", "en", "ja", "de", "fr", "es"];

/// Every language the user can enable in settings.
pub const CATALOG: &[Language] = &[
    Language {
        code: "ko",
        name: "한국어",
        native: "한국어",
    },
    Language {
        code: "en",
        name: "영어",
        native: "English",
    },
    Language {
        code: "ja",
        name: "일본어",
        native: "日本語",
    },
    Language {
        code: "de",
        name: "독일어",
        native: "Deutsch",
    },
    Language {
        code: "fr",
        name: "프랑스어",
        native: "Français",
    },
    Language {
        code: "es",
        name: "스페인어",
        native: "Español",
    },
    Language {
        code: "zh-CN",
        name: "중국어(간체)",
        native: "简体中文",
    },
    Language {
        code: "zh-TW",
        name: "중국어(번체)",
        native: "繁體中文",
    },
    Language {
        code: "it",
        name: "이탈리아어",
        native: "Italiano",
    },
    Language {
        code: "pt",
        name: "포르투갈어",
        native: "Português",
    },
    Language {
        code: "ru",
        name: "러시아어",
        native: "Русский",
    },
    Language {
        code: "vi",
        name: "베트남어",
        native: "Tiếng Việt",
    },
    Language {
        code: "th",
        name: "태국어",
        native: "ไทย",
    },
    Language {
        code: "id",
        name: "인도네시아어",
        native: "Bahasa Indonesia",
    },
    Language {
        code: "nl",
        name: "네덜란드어",
        native: "Nederlands",
    },
    Language {
        code: "pl",
        name: "폴란드어",
        native: "Polski",
    },
    Language {
        code: "tr",
        name: "터키어",
        native: "Türkçe",
    },
    Language {
        code: "uk",
        name: "우크라이나어",
        native: "Українська",
    },
    Language {
        code: "ar",
        name: "아랍어",
        native: "العربية",
    },
    Language {
        code: "hi",
        name: "힌디어",
        native: "हिन्दी",
    },
];

pub fn find(code: &str) -> Option<&'static Language> {
    let code = normalize(code);
    CATALOG
        .iter()
        .find(|lang| lang.code.eq_ignore_ascii_case(&code))
}

/// Normalizes provider-reported codes ("EN-US", "zh-Hans", "ko_KR") to the
/// catalog form so detection results compare equal to user choices.
pub fn normalize(code: &str) -> String {
    let code = code.trim().replace('_', "-");
    let lower = code.to_ascii_lowercase();
    match lower.as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh-CN".to_owned(),
        "zh-tw" | "zh-hant" | "zh-hk" => "zh-TW".to_owned(),
        "iw" => "he".to_owned(),
        _ => lower.split('-').next().unwrap_or_default().to_owned(),
    }
}

/// The first language of the chain: detected automatically or pinned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", content = "code", rename_all = "snake_case")]
pub enum SourceChoice {
    #[default]
    Auto,
    Fixed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_in_catalog() {
        for code in DEFAULT_ENABLED {
            assert!(find(code).is_some(), "{code}");
        }
    }

    #[test]
    fn normalize_maps_provider_variants() {
        assert_eq!(normalize("EN-US"), "en");
        assert_eq!(normalize("ko_KR"), "ko");
        assert_eq!(normalize("zh-Hans"), "zh-CN");
        assert_eq!(normalize("ZH"), "zh-CN");
        assert_eq!(normalize("zh-TW"), "zh-TW");
        assert_eq!(normalize(" ja "), "ja");
    }

    #[test]
    fn source_choice_serializes_compactly() {
        let json = serde_json::to_string(&SourceChoice::Fixed("en".into())).unwrap();
        assert_eq!(json, r#"{"mode":"fixed","code":"en"}"#);
        let auto: SourceChoice = serde_json::from_str(r#"{"mode":"auto"}"#).unwrap();
        assert_eq!(auto, SourceChoice::Auto);
    }
}
