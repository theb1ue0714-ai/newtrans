//! Persisted user settings. Secrets (API keys) are NOT stored here; see
//! [`Secrets`], which lives in a separate permission-restricted file.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hotkey::{Hotkey, Modifier};
use crate::lang::{DEFAULT_ENABLED, SourceChoice};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Google,
    Papago,
    Deepl,
}

impl ProviderId {
    pub const ALL: [Self; 3] = [Self::Google, Self::Papago, Self::Deepl];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Papago => "Papago",
            Self::Deepl => "DeepL",
        }
    }
}

/// Google can run without any key through the public web endpoint, or
/// through the official Cloud Translation API with the user's own key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoogleMode {
    #[default]
    Free,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    /// Default: Ctrl pressed twice quickly.
    pub primary: Option<Hotkey>,
    /// Default: Ctrl+Q. Always registered with the OS so it still works
    /// where key polling is unavailable (Wayland, permission denied).
    pub secondary: Option<Hotkey>,
    pub double_tap_interval_ms: u64,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            primary: Some(Hotkey::DoubleTap(Modifier::Ctrl)),
            secondary: Some(Hotkey::Combo {
                modifiers: vec![Modifier::Ctrl],
                key: "Q".to_owned(),
            }),
            double_tap_interval_ms: crate::doubletap::DEFAULT_INTERVAL_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageSettings {
    /// Language 1 (원문). Auto until the user pins one in the popup.
    pub source: SourceChoice,
    /// Language 2 (번역). Korean by default.
    pub target: String,
    /// Used for step 2 when the source already is the target language.
    pub fallback: String,
    /// Languages offered in the popup selectors.
    pub enabled: Vec<String>,
    /// Translate step 2 back into language 1 (재번역).
    pub back_translate: bool,
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            source: SourceChoice::Auto,
            target: "ko".to_owned(),
            fallback: "en".to_owned(),
            enabled: DEFAULT_ENABLED.iter().map(|s| (*s).to_owned()).collect(),
            back_translate: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    /// Tab order; providers not listed are hidden.
    pub order: Vec<ProviderId>,
    pub default_tab: ProviderId,
    pub google_mode: GoogleMode,
    pub request_timeout_secs: u64,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            order: ProviderId::ALL.to_vec(),
            default_tab: ProviderId::Google,
            google_mode: GoogleMode::Free,
            request_timeout_secs: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub hotkeys: HotkeySettings,
    pub languages: LanguageSettings,
    pub providers: ProviderSettings,
    /// Start with the operating system (enabled on first run).
    pub autostart: bool,
    /// Closing a window hides it; NewTrans keeps running in the tray.
    pub close_to_tray: bool,
    pub hide_on_blur: bool,
    pub join_wrapped_lines: bool,
    pub always_on_top: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            hotkeys: HotkeySettings::default(),
            languages: LanguageSettings::default(),
            providers: ProviderSettings::default(),
            autostart: true,
            close_to_tray: true,
            hide_on_blur: false,
            join_wrapped_lines: true,
            always_on_top: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("설정 파일을 읽거나 쓸 수 없습니다: {0}")]
    Io(#[from] std::io::Error),
    #[error("설정 형식이 올바르지 않습니다: {0}")]
    Format(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        let languages = &self.languages;
        if languages.target.trim().is_empty() || languages.fallback.trim().is_empty() {
            return Err(SettingsError::Invalid("번역 언어가 비어 있습니다".into()));
        }
        if languages.target == languages.fallback {
            return Err(SettingsError::Invalid(
                "기본 번역 언어와 대체 언어는 달라야 합니다".into(),
            ));
        }
        if self.providers.order.is_empty() {
            return Err(SettingsError::Invalid(
                "번역 서비스를 하나 이상 사용해야 합니다".into(),
            ));
        }
        if self.hotkeys.primary.is_none() && self.hotkeys.secondary.is_none() {
            return Err(SettingsError::Invalid(
                "단축키를 하나 이상 지정해야 합니다".into(),
            ));
        }
        if self.hotkeys.primary.is_some() && self.hotkeys.primary == self.hotkeys.secondary {
            return Err(SettingsError::Invalid(
                "주 단축키와 보조 단축키가 같습니다".into(),
            ));
        }
        if !(150..=1000).contains(&self.hotkeys.double_tap_interval_ms) {
            return Err(SettingsError::Invalid(
                "두 번 누르기 간격은 150~1000ms 사이여야 합니다".into(),
            ));
        }
        if !(3..=60).contains(&self.providers.request_timeout_secs) {
            return Err(SettingsError::Invalid(
                "요청 제한 시간은 3~60초 사이여야 합니다".into(),
            ));
        }
        Ok(())
    }
}

/// Outcome of loading: the settings plus whether this is the first run.
pub struct Loaded<T> {
    pub value: T,
    pub first_run: bool,
    pub recovered_from: Option<PathBuf>,
}

/// Loads JSON, falling back to defaults. A corrupt file is renamed aside
/// (never deleted) so the user can inspect it.
pub fn load_json<T: Default + for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Loaded<T>, SettingsError> {
    match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(value) => Ok(Loaded {
                value,
                first_run: false,
                recovered_from: None,
            }),
            Err(_) => {
                let backup = path.with_extension("corrupt.json");
                fs::rename(path, &backup)?;
                Ok(Loaded {
                    value: T::default(),
                    first_run: false,
                    recovered_from: Some(backup),
                })
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Loaded {
            value: T::default(),
            first_run: true,
            recovered_from: None,
        }),
        Err(error) => Err(error.into()),
    }
}

/// Writes atomically (temp file + rename). With `private`, the file is
/// created readable by the current user only on Unix.
pub fn save_json<T: Serialize>(path: &Path, value: &T, private: bool) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut file = options.open(&tmp)?;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// User-supplied API keys. Stored separately from settings so that
/// settings can be shared or exported without leaking credentials.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Secrets {
    pub google_api_key: Option<String>,
    pub papago_client_id: Option<String>,
    pub papago_client_secret: Option<String>,
    pub deepl_api_key: Option<String>,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key material, even in debug logs.
        f.debug_struct("Secrets")
            .field("google_api_key", &self.google_api_key.is_some())
            .field(
                "papago",
                &(self.papago_client_id.is_some() && self.papago_client_secret.is_some()),
            )
            .field("deepl_api_key", &self.deepl_api_key.is_some())
            .finish()
    }
}

/// Which keys are present, safe to send to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SecretStatus {
    pub google: bool,
    pub papago: bool,
    pub deepl: bool,
}

impl Secrets {
    pub fn status(&self) -> SecretStatus {
        let set = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.trim().is_empty());
        SecretStatus {
            google: set(&self.google_api_key),
            papago: set(&self.papago_client_id) && set(&self.papago_client_secret),
            deepl: set(&self.deepl_api_key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("newtrans-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_match_product_decisions() {
        let settings = Settings::default();
        assert_eq!(
            settings.hotkeys.primary.as_ref().unwrap().to_string(),
            "DoubleCtrl"
        );
        assert_eq!(
            settings.hotkeys.secondary.as_ref().unwrap().to_string(),
            "Ctrl+Q"
        );
        assert_eq!(settings.languages.source, SourceChoice::Auto);
        assert_eq!(settings.languages.target, "ko");
        assert_eq!(
            settings.languages.enabled,
            ["ko", "en", "ja", "de", "fr", "es"]
        );
        assert!(settings.languages.back_translate);
        assert!(settings.autostart && settings.close_to_tray);
        assert_eq!(settings.providers.order, ProviderId::ALL);
        assert_eq!(settings.providers.google_mode, GoogleMode::Free);
        settings.validate().unwrap();
    }

    #[test]
    fn validation_catches_conflicts() {
        let mut settings = Settings::default();
        settings.hotkeys.secondary = settings.hotkeys.primary.clone();
        assert!(settings.validate().is_err());
        let mut settings = Settings::default();
        settings.languages.fallback = "ko".into();
        assert!(settings.validate().is_err());
        let mut settings = Settings::default();
        settings.providers.order.clear();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"autostart": false}"#).unwrap();
        assert!(!settings.autostart);
        assert_eq!(settings.languages.target, "ko");
    }

    #[test]
    fn save_and_load_round_trip_and_first_run() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("settings.json");
        let loaded: Loaded<Settings> = load_json(&path).unwrap();
        assert!(loaded.first_run);
        let mut settings = Settings::default();
        settings.languages.source = SourceChoice::Fixed("en".into());
        save_json(&path, &settings, false).unwrap();
        let loaded: Loaded<Settings> = load_json(&path).unwrap();
        assert!(!loaded.first_run);
        assert_eq!(loaded.value, settings);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let loaded: Loaded<Settings> = load_json(&path).unwrap();
        assert_eq!(loaded.value, Settings::default());
        assert!(loaded.recovered_from.unwrap().exists());
    }

    #[cfg(unix)]
    #[test]
    fn secrets_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("secrets");
        let path = dir.join("secrets.json");
        let secrets = Secrets {
            deepl_api_key: Some("abc:fx".into()),
            ..Secrets::default()
        };
        save_json(&path, &secrets, true).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn secrets_debug_hides_values() {
        let secrets = Secrets {
            deepl_api_key: Some("super-secret".into()),
            ..Secrets::default()
        };
        assert!(!format!("{secrets:?}").contains("super-secret"));
        assert!(secrets.status().deepl);
        assert!(!secrets.status().papago);
    }
}
