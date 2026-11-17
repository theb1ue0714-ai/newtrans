//! Reads the text the user selected in another application.
//!
//! * Linux: the PRIMARY selection (filled by dragging), no key simulation.
//! * Windows/macOS (and Linux fallback): simulate the copy shortcut, read
//!   the clipboard, then restore the previous clipboard contents.
//!
//! Selected text is never logged.

use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};

const RELEASE_WAIT: Duration = Duration::from_millis(400);
const COPY_WAIT: Duration = Duration::from_millis(600);

pub fn capture() -> Option<String> {
    #[cfg(target_os = "linux")]
    if let Some(text) = primary_selection() {
        return Some(text);
    }
    copy_via_shortcut()
}

#[cfg(target_os = "linux")]
fn primary_selection() -> Option<String> {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    let mut clipboard = Clipboard::new().ok()?;
    clipboard
        .get()
        .clipboard(LinuxClipboardKind::Primary)
        .text()
        .ok()
        .filter(|text| !text.trim().is_empty())
}

enum Backup {
    Text(String),
    Image(arboard::ImageData<'static>),
    Empty,
}

fn copy_via_shortcut() -> Option<String> {
    let mut clipboard = Clipboard::new().ok()?;
    let backup = if let Ok(text) = clipboard.get_text() {
        Backup::Text(text)
    } else if let Ok(image) = clipboard.get_image() {
        Backup::Image(image.to_owned_img())
    } else {
        Backup::Empty
    };
    // A sentinel distinguishes "nothing selected" from "selection equals
    // the old clipboard text".
    let _ = clipboard.clear();

    let deadline = Instant::now() + RELEASE_WAIT;
    while crate::input::any_key_down() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    send_copy_shortcut().ok()?;

    let deadline = Instant::now() + COPY_WAIT;
    let mut captured = None;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
        if let Ok(text) = clipboard.get_text()
            && !text.trim().is_empty()
        {
            captured = Some(text);
            break;
        }
    }

    match backup {
        Backup::Text(text) => {
            let _ = clipboard.set_text(text);
        }
        Backup::Image(image) => {
            let _ = clipboard.set_image(image);
        }
        Backup::Empty => {
            let _ = clipboard.clear();
        }
    }
    captured
}

fn send_copy_shortcut() -> enigo::InputResult<()> {
    let mut enigo =
        Enigo::new(&EnigoSettings::default()).map_err(|_| enigo::InputError::Simulate("init"))?;
    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };
    enigo.key(modifier, Direction::Press)?;
    let result = enigo.key(Key::Unicode('c'), Direction::Click);
    enigo.key(modifier, Direction::Release)?;
    result
}
