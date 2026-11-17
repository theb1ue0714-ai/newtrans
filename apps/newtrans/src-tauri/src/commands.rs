//! IPC commands for the popup and settings webviews.

use newtrans_core::hotkey::Hotkey;
use newtrans_core::lang::{CATALOG, Language, SourceChoice};
use newtrans_core::settings::{ProviderId, SecretStatus, Settings};
use newtrans_providers::{ChainRequest, ChainResult, ProviderError, Request, run_chain};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime, State, WebviewWindow};

use crate::state::{AppState, lock};

#[derive(Serialize)]
pub struct Bootstrap {
    version: &'static str,
    settings: Settings,
    secrets: SecretStatus,
    languages: &'static [Language],
    diagnostics: Vec<String>,
    platform: &'static str,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    Bootstrap {
        version: env!("CARGO_PKG_VERSION"),
        settings: state.settings(),
        secrets: lock(&state.secrets).status(),
        languages: CATALOG,
        diagnostics: lock(&state.diagnostics).clone(),
        platform: std::env::consts::OS,
    }
}

#[tauri::command]
pub fn take_pending_text(state: State<'_, AppState>) -> Option<String> {
    lock(&state.pending_text).take()
}

#[tauri::command]
pub async fn translate(
    state: State<'_, AppState>,
    provider: ProviderId,
    text: String,
    source: SourceChoice,
    target: String,
) -> Result<ChainResult, ProviderError> {
    let translator = state.provider(provider)?;
    let client = state.client();
    let languages = lock(&state.settings).languages.clone();
    let fallback = if languages.fallback == target {
        if target == "en" {
            "ko".to_owned()
        } else {
            "en".to_owned()
        }
    } else {
        languages.fallback.clone()
    };
    let request = ChainRequest {
        text,
        source,
        target,
        fallback,
        back_translate: languages.back_translate,
    };
    run_chain(&request, |text, source, target| {
        let translator = translator.clone();
        let client = client.clone();
        async move {
            if !translator.supports(&target) {
                return Err(ProviderError::UnsupportedLanguage(target));
            }
            translator
                .translate(
                    &client,
                    &Request {
                        text: &text,
                        source: source.as_deref(),
                        target: &target,
                    },
                )
                .await
        }
    })
    .await
}

/// Called when the user changes language 1 or 2 in the popup: the choice
/// becomes the new fixed default (고정).
#[tauri::command]
pub fn pin_languages(
    state: State<'_, AppState>,
    source: SourceChoice,
    target: String,
) -> Result<(), String> {
    let mut settings = state.settings();
    settings.languages.source = source;
    if settings.languages.fallback == target {
        settings.languages.fallback = if target == "en" {
            "ko".into()
        } else {
            "en".into()
        };
    }
    settings.languages.target = target;
    state.save_settings(settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Vec<String>, String> {
    let autostart = settings.autostart;
    state
        .save_settings(settings.clone())
        .map_err(|e| e.to_string())?;
    crate::reconcile_autostart(&app, autostart);
    let failures = crate::input::register_combos(&app, &settings);
    let _ = app.emit("settings-changed", ());
    Ok(failures)
}

/// A field is left unchanged when `None`, cleared when `Some("")`.
#[derive(Deserialize)]
pub struct SecretsPatch {
    google_api_key: Option<String>,
    papago_client_id: Option<String>,
    papago_client_secret: Option<String>,
    deepl_api_key: Option<String>,
}

#[tauri::command]
pub fn save_secrets(
    state: State<'_, AppState>,
    patch: SecretsPatch,
) -> Result<SecretStatus, String> {
    let mut secrets = lock(&state.secrets).clone();
    let apply = |slot: &mut Option<String>, value: Option<String>| {
        if let Some(value) = value {
            let value = value.trim().to_owned();
            *slot = (!value.is_empty()).then_some(value);
        }
    };
    apply(&mut secrets.google_api_key, patch.google_api_key);
    apply(&mut secrets.papago_client_id, patch.papago_client_id);
    apply(
        &mut secrets.papago_client_secret,
        patch.papago_client_secret,
    );
    apply(&mut secrets.deepl_api_key, patch.deepl_api_key);
    let status = secrets.status();
    state.save_secrets(secrets).map_err(|e| e.to_string())?;
    Ok(status)
}

#[tauri::command]
pub async fn verify_provider(
    state: State<'_, AppState>,
    provider: ProviderId,
) -> Result<String, ProviderError> {
    let translator = state.provider(provider)?;
    let result = translator
        .translate(
            &state.client(),
            &Request {
                text: "Good morning",
                source: Some("en"),
                target: "ko",
            },
        )
        .await?;
    Ok(result.text)
}

#[tauri::command]
pub fn validate_hotkey(value: String) -> Result<String, String> {
    value
        .parse::<Hotkey>()
        .map(|hotkey| hotkey.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_settings<R: Runtime>(app: AppHandle<R>) {
    crate::show_settings(&app);
}

#[tauri::command]
pub fn hide_window<R: Runtime>(window: WebviewWindow<R>) {
    let _ = window.hide();
}
