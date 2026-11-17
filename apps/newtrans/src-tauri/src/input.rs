//! Global hotkeys: OS-registered combinations plus a polled modifier
//! double tap.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use device_query::{DeviceQuery, DeviceState, Keycode};
use newtrans_core::doubletap::{DoubleTapDetector, Key, SnapshotTracker};
use newtrans_core::hotkey::{Hotkey, Modifier};
use newtrans_core::settings::Settings;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::state::{AppState, lock};

const POLL_INTERVAL: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleTapConfig {
    pub modifier: Option<Modifier>,
    pub interval_ms: u64,
}

impl DoubleTapConfig {
    pub fn from_settings(settings: &Settings) -> Self {
        let modifier = [&settings.hotkeys.primary, &settings.hotkeys.secondary]
            .into_iter()
            .flatten()
            .find_map(|hotkey| match hotkey {
                Hotkey::DoubleTap(modifier) => Some(*modifier),
                Hotkey::Combo { .. } => None,
            });
        Self {
            modifier,
            interval_ms: settings.hotkeys.double_tap_interval_ms,
        }
    }
}

pub fn classify(key: Keycode) -> Key {
    match key {
        Keycode::LControl | Keycode::RControl => Key::Modifier(Modifier::Ctrl),
        Keycode::LShift | Keycode::RShift => Key::Modifier(Modifier::Shift),
        Keycode::LAlt | Keycode::RAlt | Keycode::LOption | Keycode::ROption => {
            Key::Modifier(Modifier::Alt)
        }
        Keycode::LMeta | Keycode::RMeta | Keycode::Command | Keycode::RCommand => {
            Key::Modifier(Modifier::Super)
        }
        _ => Key::Other,
    }
}

/// Starts the poller. Returns `false` when key state cannot be read
/// (macOS without Input Monitoring/Accessibility permission, Wayland
/// without XWayland); the OS-registered combination still works then.
pub fn start_double_tap_poller<R: Runtime>(app: &AppHandle<R>) -> bool {
    let state = app.state::<AppState>();
    let config = Arc::clone(&state.double_tap);
    let paused = Arc::clone(&state.paused);
    let handle = app.clone();
    // The key-state reader is not Send on every platform, so it is created
    // on the poller thread, which reports back whether it is available.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let spawned = thread::Builder::new()
        .name("newtrans-doubletap".into())
        .spawn(move || {
            let Some(device) = DeviceState::checked_new() else {
                let _ = ready_tx.send(false);
                return;
            };
            let _ = ready_tx.send(true);
            poll_loop(device, config, paused, handle);
        })
        .is_ok();
    spawned
        && ready_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or(false)
}

fn poll_loop<R: Runtime>(
    device: DeviceState,
    config: Arc<Mutex<DoubleTapConfig>>,
    paused: Arc<std::sync::atomic::AtomicBool>,
    app: AppHandle<R>,
) {
    let started = Instant::now();
    let mut tracker = SnapshotTracker::default();
    let mut active = lock(&config).clone();
    let mut detector = active
        .modifier
        .map(|m| DoubleTapDetector::new(m, active.interval_ms));
    loop {
        thread::sleep(POLL_INTERVAL);
        let current = lock(&config).clone();
        if current != active {
            detector = current
                .modifier
                .map(|m| DoubleTapDetector::new(m, current.interval_ms));
            active = current;
        }
        let Some(detector) = detector.as_mut() else {
            continue;
        };
        let pressed = device
            .get_keys()
            .into_iter()
            .map(|key| (classify(key), key as u32))
            .collect();
        let now = started.elapsed().as_millis() as u64;
        for edge in tracker.edges(pressed) {
            if detector.feed(edge, now) && !paused.load(Ordering::Relaxed) {
                crate::trigger(&app);
            }
        }
    }
}

/// True while any key is physically down; used to wait until the user
/// releases the hotkey before simulating Ctrl+C.
pub fn any_key_down() -> bool {
    DeviceState::checked_new().is_some_and(|device| !device.get_keys().is_empty())
}

/// Re-registers the OS combinations from settings. Returns one message per
/// combination that could not be registered (taken by another program).
pub fn register_combos<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> Vec<String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let mut failures = Vec::new();
    for hotkey in [&settings.hotkeys.primary, &settings.hotkeys.secondary]
        .into_iter()
        .flatten()
    {
        let Some(accelerator) = hotkey.accelerator() else {
            continue;
        };
        let result = accelerator
            .parse::<Shortcut>()
            .map_err(|error| error.to_string())
            .and_then(|shortcut| {
                manager
                    .register(shortcut)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            failures.push(format!("{hotkey}: {error}"));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_both_sides_of_modifiers() {
        assert_eq!(classify(Keycode::LControl), Key::Modifier(Modifier::Ctrl));
        assert_eq!(classify(Keycode::RControl), Key::Modifier(Modifier::Ctrl));
        assert_eq!(classify(Keycode::Command), Key::Modifier(Modifier::Super));
        assert_eq!(classify(Keycode::C), Key::Other);
    }

    #[test]
    fn double_tap_config_follows_settings() {
        let settings = Settings::default();
        assert_eq!(
            DoubleTapConfig::from_settings(&settings).modifier,
            Some(Modifier::Ctrl)
        );
        let mut settings = Settings::default();
        settings.hotkeys.primary = Some("Ctrl+Shift+Z".parse().unwrap());
        assert_eq!(DoubleTapConfig::from_settings(&settings).modifier, None);
    }
}
