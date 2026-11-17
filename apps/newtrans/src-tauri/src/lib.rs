//! NewTrans desktop shell.

mod commands;
mod input;
mod selection;
mod state;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use newtrans_core::settings::{Loaded, Secrets, Settings, load_json};
use newtrans_core::text::join_wrapped_lines;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::state::{AppState, Paths, lock};

pub const POPUP: &str = "popup";
pub const SETTINGS: &str = "settings";

static CAPTURING: AtomicBool = AtomicBool::new(false);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch (e.g. from the start menu) opens the window.
            show_popup(app, false);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && !app.state::<AppState>().paused.load(Ordering::Relaxed)
                    {
                        trigger(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            setup(app.handle())?;
            tray::build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) if window.label() == POPUP => {
                let state = window.state::<AppState>();
                if lock(&state.settings).hide_on_blur && !CAPTURING.load(Ordering::Relaxed) {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::take_pending_text,
            commands::translate,
            commands::pin_languages,
            commands::save_settings,
            commands::save_secrets,
            commands::verify_provider,
            commands::validate_hotkey,
            commands::open_settings,
            commands::hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NewTrans");
}

fn setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_config_dir()?;
    let paths = Paths {
        settings: dir.join("settings.json"),
        secrets: dir.join("secrets.json"),
    };
    let Loaded {
        value: settings,
        first_run,
        recovered_from,
    } = load_json::<Settings>(&paths.settings)?;
    let settings = if settings.validate().is_ok() {
        settings
    } else {
        Settings::default()
    };
    let secrets: Secrets = load_json(&paths.secrets)
        .map(|loaded| loaded.value)
        .unwrap_or_default();
    app.manage(AppState::new(paths, settings.clone(), secrets));
    let state = app.state::<AppState>();

    if let Some(backup) = recovered_from {
        state.note(format!(
            "손상된 설정 파일을 {}로 옮기고 기본값으로 시작했습니다.",
            backup.display()
        ));
    }
    if first_run {
        // Persist defaults so the next start is not treated as first run.
        let _ = state.save_settings(settings.clone());
    }
    reconcile_autostart(app, settings.autostart);

    for failure in input::register_combos(app, &settings) {
        state.note(format!("단축키 등록 실패 — {failure}"));
    }
    if !input::start_double_tap_poller(app) {
        state.note(
            "키 입력 상태를 읽을 수 없어 'Ctrl 두 번'을 쓸 수 없습니다. macOS는 손쉬운 사용 권한을 허용하고, \
             Linux Wayland에서는 보조 단축키를 사용하세요."
                .to_owned(),
        );
    }
    if first_run {
        show_popup(app, false);
    }
    Ok(())
}

pub fn reconcile_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let manager = app.autolaunch();
    let result = match (enabled, manager.is_enabled().unwrap_or(false)) {
        (true, false) => manager.enable(),
        (false, true) => manager.disable(),
        _ => Ok(()),
    };
    if let Err(error) = result {
        app.state::<AppState>()
            .note(format!("자동 실행 설정 실패: {error}"));
    }
}

/// Hotkey entry point: capture the selection, then show the popup with it.
/// Pressing the hotkey while the popup is focused cycles to the next tab.
pub fn trigger<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(POPUP)
        && window.is_visible().unwrap_or(false)
        && window.is_focused().unwrap_or(false)
    {
        let _ = window.emit("cycle-tab", ());
        return;
    }
    if CAPTURING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let state = app.state::<AppState>();
        let join = lock(&state.settings).join_wrapped_lines;
        let text = selection::capture().map(|text| {
            if join {
                join_wrapped_lines(&text)
            } else {
                text.trim().to_owned()
            }
        });
        *lock(&state.pending_text) = text.clone();
        show_popup(&app, true);
        let _ = app.emit_to(POPUP, "new-text", text);
        CAPTURING.store(false, Ordering::Release);
    });
}

pub fn show_popup<R: Runtime>(app: &AppHandle<R>, near_cursor: bool) {
    let Some(window) = app.get_webview_window(POPUP) else {
        return;
    };
    let always_on_top = lock(&app.state::<AppState>().settings).always_on_top;
    let _ = window.set_always_on_top(always_on_top);
    if near_cursor {
        place_near_cursor(app, &window);
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn place_near_cursor<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Ok(cursor) = app.cursor_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(Some(monitor)) = app.monitor_from_point(cursor.x, cursor.y) else {
        return;
    };
    let area = monitor.work_area();
    let (min_x, min_y) = (area.position.x as f64, area.position.y as f64);
    let (max_x, max_y) = (
        min_x + area.size.width as f64,
        min_y + area.size.height as f64,
    );
    let offset = 16.0;
    let mut x = cursor.x + offset;
    let mut y = cursor.y + offset;
    if x + size.width as f64 > max_x {
        x = (cursor.x - size.width as f64 - offset).max(min_x);
    }
    if y + size.height as f64 > max_y {
        y = (cursor.y - size.height as f64 - offset).max(min_y);
    }
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

pub fn show_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
