use std::sync::atomic::Ordering;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager, Runtime};

use crate::state::AppState;

pub fn build<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "번역 창 열기", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "설정", true, None::<&str>)?;
    let pause =
        CheckMenuItem::with_id(app, "pause", "단축키 일시 중지", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "NewTrans 종료", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &settings, &pause, &separator, &quit])?;

    let pause_item = pause.clone();
    let mut tray = TrayIconBuilder::with_id("newtrans")
        .tooltip("NewTrans — 선택 후 Ctrl 두 번")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => crate::show_popup(app, false),
            "settings" => crate::show_settings(app),
            "pause" => {
                let paused = pause_item.is_checked().unwrap_or(false);
                app.state::<AppState>()
                    .paused
                    .store(paused, Ordering::Relaxed);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_popup(tray.app_handle(), false);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
