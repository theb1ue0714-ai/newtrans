//! Shared application state.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use newtrans_core::settings::{
    GoogleMode, ProviderId, Secrets, Settings, SettingsError, save_json,
};
use newtrans_providers::Client;
use newtrans_providers::{Provider, ProviderError, http_client};

pub struct Paths {
    pub settings: PathBuf,
    pub secrets: PathBuf,
}

pub struct AppState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub secrets: Mutex<Secrets>,
    pub client: Mutex<Client>,
    /// Hotkeys are ignored while paused from the tray menu.
    pub paused: Arc<AtomicBool>,
    /// Shared with the double-tap poller thread.
    pub double_tap: Arc<Mutex<crate::input::DoubleTapConfig>>,
    pub diagnostics: Mutex<Vec<String>>,
    /// Text most recently captured by a hotkey, served to a popup that
    /// was not yet loaded when the event fired.
    pub pending_text: Mutex<Option<String>>,
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AppState {
    pub fn new(paths: Paths, settings: Settings, secrets: Secrets) -> Self {
        let client = build_client(&settings);
        let double_tap = crate::input::DoubleTapConfig::from_settings(&settings);
        Self {
            paths,
            settings: Mutex::new(settings),
            secrets: Mutex::new(secrets),
            client: Mutex::new(client),
            paused: Arc::new(AtomicBool::new(false)),
            double_tap: Arc::new(Mutex::new(double_tap)),
            diagnostics: Mutex::new(Vec::new()),
            pending_text: Mutex::new(None),
        }
    }

    pub fn note(&self, message: impl Into<String>) {
        let mut log = lock(&self.diagnostics);
        log.push(message.into());
        let overflow = log.len().saturating_sub(50);
        log.drain(..overflow);
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn save_settings(&self, settings: Settings) -> Result<(), SettingsError> {
        settings.validate()?;
        save_json(&self.paths.settings, &settings, false)?;
        *lock(&self.client) = build_client(&settings);
        *lock(&self.double_tap) = crate::input::DoubleTapConfig::from_settings(&settings);
        *lock(&self.settings) = settings;
        Ok(())
    }

    pub fn save_secrets(&self, secrets: Secrets) -> Result<(), SettingsError> {
        save_json(&self.paths.secrets, &secrets, true)?;
        *lock(&self.secrets) = secrets;
        Ok(())
    }

    pub fn provider(&self, id: ProviderId) -> Result<Provider, ProviderError> {
        let secrets = lock(&self.secrets);
        let key = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .ok_or(ProviderError::MissingKey)
        };
        Ok(match id {
            ProviderId::Google => match lock(&self.settings).providers.google_mode {
                GoogleMode::Free => Provider::GoogleFree,
                GoogleMode::Cloud => Provider::GoogleCloud {
                    api_key: key(&secrets.google_api_key)?,
                },
            },
            ProviderId::Papago => Provider::Papago {
                client_id: key(&secrets.papago_client_id)?,
                client_secret: key(&secrets.papago_client_secret)?,
            },
            ProviderId::Deepl => Provider::Deepl {
                api_key: key(&secrets.deepl_api_key)?,
            },
        })
    }

    pub fn client(&self) -> Client {
        lock(&self.client).clone()
    }
}

fn build_client(settings: &Settings) -> Client {
    let timeout = Duration::from_secs(settings.providers.request_timeout_secs.clamp(3, 60));
    http_client(timeout).unwrap_or_else(|_| Client::new())
}
