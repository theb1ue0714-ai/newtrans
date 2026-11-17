//! User-configurable hotkeys.
//!
//! Two kinds exist:
//! * a modifier double tap ("Ctrl 두 번") detected by polling key state, and
//! * an ordinary combination of at most three keys (modifiers plus exactly
//!   one non-modifier key) registered with the OS, which also stops the
//!   keystroke from reaching the focused application.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    /// Windows key / ⌘ Command.
    Super,
}

impl Modifier {
    const fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Shift => "Shift",
            Self::Super => "Super",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "alt" | "option" | "opt" => Some(Self::Alt),
            "shift" => Some(Self::Shift),
            "super" | "win" | "cmd" | "command" | "meta" => Some(Self::Super),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Hotkey {
    DoubleTap(Modifier),
    Combo {
        modifiers: Vec<Modifier>,
        key: String,
    },
}

pub const MAX_KEYS: usize = 3;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HotkeyError {
    #[error("단축키가 비어 있습니다")]
    Empty,
    #[error("단축키는 최대 {MAX_KEYS}개 키까지 조합할 수 있습니다")]
    TooManyKeys,
    #[error("수식키(Ctrl·Alt·Shift·Win) 외의 일반 키가 정확히 하나 있어야 합니다")]
    NeedsOneKey,
    #[error("알 수 없는 키입니다: {0}")]
    UnknownKey(String),
    #[error("같은 키가 두 번 들어 있습니다")]
    Duplicate,
    #[error("운영체제가 예약한 조합이라 사용할 수 없습니다: {0}")]
    Reserved(String),
    #[error("일반 키 단독 단축키는 입력을 방해하므로 수식키를 하나 이상 포함해야 합니다")]
    NeedsModifier,
}

impl FromStr for Hotkey {
    type Err = HotkeyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.is_empty() {
            return Err(HotkeyError::Empty);
        }
        let lower = compact.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("double") {
            let rest = rest.trim_start_matches(['-', '+', '_']);
            return Modifier::parse(rest)
                .map(Hotkey::DoubleTap)
                .ok_or_else(|| HotkeyError::UnknownKey(rest.to_owned()));
        }
        let tokens: Vec<&str> = compact.split('+').filter(|t| !t.is_empty()).collect();
        if tokens.is_empty() {
            return Err(HotkeyError::Empty);
        }
        if tokens.len() > MAX_KEYS {
            return Err(HotkeyError::TooManyKeys);
        }
        let mut modifiers = Vec::new();
        let mut key = None;
        for token in tokens {
            if let Some(modifier) = Modifier::parse(token) {
                if modifiers.contains(&modifier) {
                    return Err(HotkeyError::Duplicate);
                }
                modifiers.push(modifier);
            } else {
                let normalized = normalize_key(token)
                    .ok_or_else(|| HotkeyError::UnknownKey(token.to_owned()))?;
                if key.replace(normalized).is_some() {
                    return Err(HotkeyError::NeedsOneKey);
                }
            }
        }
        let key = key.ok_or(HotkeyError::NeedsOneKey)?;
        modifiers.sort();
        if modifiers.is_empty() && !key.starts_with('F') {
            return Err(HotkeyError::NeedsModifier);
        }
        let hotkey = Hotkey::Combo { modifiers, key };
        if let Some(reason) = reserved(&hotkey) {
            return Err(HotkeyError::Reserved(reason.to_owned()));
        }
        Ok(hotkey)
    }
}

/// Maps a key token to the canonical name used in accelerators:
/// `A`–`Z`, `0`–`9`, `F1`–`F24`, and a few named keys.
fn normalize_key(token: &str) -> Option<String> {
    let upper = token.to_ascii_uppercase();
    let upper = upper
        .strip_prefix("KEY")
        .filter(|s| s.len() == 1)
        .map(str::to_owned)
        .unwrap_or(upper);
    let upper = upper
        .strip_prefix("DIGIT")
        .filter(|s| s.len() == 1)
        .map(str::to_owned)
        .unwrap_or(upper);
    if upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(upper);
    }
    if let Some(number) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return (1..=24).contains(&number).then(|| format!("F{number}"));
    }
    let named = match upper.as_str() {
        "SPACE" => "Space",
        "ENTER" | "RETURN" => "Enter",
        "TAB" => "Tab",
        "INSERT" | "INS" => "Insert",
        "HOME" => "Home",
        "END" => "End",
        "PAGEUP" | "PGUP" => "PageUp",
        "PAGEDOWN" | "PGDN" => "PageDown",
        "BACKQUOTE" | "`" => "Backquote",
        "PAUSE" => "Pause",
        _ => return None,
    };
    Some(named.to_owned())
}

fn reserved(hotkey: &Hotkey) -> Option<&'static str> {
    let Hotkey::Combo { modifiers, key } = hotkey else {
        return None;
    };
    let has = |m| modifiers.contains(&m);
    let only =
        |ms: &[Modifier]| modifiers.len() == ms.len() && ms.iter().all(|m| modifiers.contains(m));
    match key.as_str() {
        "Tab" if has(Modifier::Alt) || has(Modifier::Super) => Some("창 전환(Alt+Tab)"),
        "F4" if has(Modifier::Alt) => Some("창 닫기(Alt+F4)"),
        "C" | "V" | "X" | "A" | "Z" if only(&[Modifier::Ctrl]) || only(&[Modifier::Super]) => {
            Some("복사·붙여넣기·실행취소 등 기본 편집 키")
        }
        "L" if only(&[Modifier::Super]) => Some("화면 잠금(Win+L)"),
        _ => None,
    }
}

impl Hotkey {
    /// Accelerator string understood by the OS shortcut registrar, or
    /// `None` for double taps, which are detected by polling instead.
    pub fn accelerator(&self) -> Option<String> {
        match self {
            Hotkey::DoubleTap(_) => None,
            Hotkey::Combo { modifiers, key } => {
                let mut parts: Vec<&str> = modifiers
                    .iter()
                    .map(|m| match m {
                        Modifier::Ctrl => "Control",
                        Modifier::Alt => "Alt",
                        Modifier::Shift => "Shift",
                        Modifier::Super => "Super",
                    })
                    .collect();
                parts.push(key);
                Some(parts.join("+"))
            }
        }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hotkey::DoubleTap(modifier) => write!(f, "Double{}", modifier.label()),
            Hotkey::Combo { modifiers, key } => {
                for modifier in modifiers {
                    write!(f, "{}+", modifier.label())?;
                }
                f.write_str(key)
            }
        }
    }
}

impl Serialize for Hotkey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Hotkey, HotkeyError> {
        s.parse()
    }

    #[test]
    fn parses_double_tap() {
        assert_eq!(
            parse("DoubleCtrl").unwrap(),
            Hotkey::DoubleTap(Modifier::Ctrl)
        );
        assert_eq!(
            parse("double-alt").unwrap(),
            Hotkey::DoubleTap(Modifier::Alt)
        );
        assert_eq!(parse("DoubleCtrl").unwrap().accelerator(), None);
    }

    #[test]
    fn parses_and_normalizes_combos() {
        let hotkey = parse("shift + ctrl + z").unwrap();
        assert_eq!(hotkey.to_string(), "Ctrl+Shift+Z");
        assert_eq!(hotkey.accelerator().as_deref(), Some("Control+Shift+Z"));
        assert_eq!(parse("Ctrl+Q").unwrap().to_string(), "Ctrl+Q");
        assert_eq!(parse("Alt+KeyT").unwrap().to_string(), "Alt+T");
        assert_eq!(parse("F9").unwrap().to_string(), "F9");
        assert_eq!(parse("Ctrl+Alt+Digit1").unwrap().to_string(), "Ctrl+Alt+1");
    }

    #[test]
    fn enforces_three_key_limit() {
        assert_eq!(parse("Ctrl+Alt+Shift+Q"), Err(HotkeyError::TooManyKeys));
        assert!(parse("Ctrl+Alt+Q").is_ok());
    }

    #[test]
    fn rejects_invalid_combos() {
        assert_eq!(parse(""), Err(HotkeyError::Empty));
        assert_eq!(parse("Ctrl+Shift"), Err(HotkeyError::NeedsOneKey));
        assert_eq!(parse("Ctrl+Q+W"), Err(HotkeyError::NeedsOneKey));
        assert_eq!(parse("Ctrl+Ctrl+Q"), Err(HotkeyError::Duplicate));
        assert_eq!(parse("Q"), Err(HotkeyError::NeedsModifier));
        assert!(matches!(
            parse("Ctrl+Hyper"),
            Err(HotkeyError::UnknownKey(_))
        ));
        assert!(matches!(parse("Alt+Tab"), Err(HotkeyError::Reserved(_))));
        assert!(matches!(parse("Ctrl+C"), Err(HotkeyError::Reserved(_))));
        assert!(parse("Ctrl+Shift+C").is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let json = serde_json::to_string(&parse("Ctrl+Shift+Z").unwrap()).unwrap();
        assert_eq!(json, "\"Ctrl+Shift+Z\"");
        let back: Hotkey = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_string(), "Ctrl+Shift+Z");
        assert!(serde_json::from_str::<Hotkey>("\"Alt+Tab\"").is_err());
    }
}
