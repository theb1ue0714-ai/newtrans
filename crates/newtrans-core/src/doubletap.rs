//! Modifier double-tap detection ("Ctrl 두 번 빠르게").
//!
//! The detector consumes key-down/key-up edges with timestamps and is fed
//! by a key-state poller rather than a low-level keyboard hook. Polling
//! cannot be silently unhooked by Windows when a callback is slow, which is
//! the failure that broke double-Ctrl in older translators on Windows 11.

use crate::hotkey::Modifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Modifier(Modifier),
    /// Any non-modifier key. Pressing one cancels a tap sequence so that
    /// shortcuts like Ctrl+C typed twice never trigger translation.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Down(Key),
    Up(Key),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    FirstDown { at: u64 },
    FirstUp { at: u64 },
    SecondDown { at: u64 },
}

#[derive(Debug, Clone)]
pub struct DoubleTapDetector {
    target: Modifier,
    /// Maximum gap between the first release and the second press.
    interval_ms: u64,
    /// A press held longer than this is a hold, not a tap.
    max_hold_ms: u64,
    state: State,
}

pub const DEFAULT_INTERVAL_MS: u64 = 350;
pub const DEFAULT_MAX_HOLD_MS: u64 = 300;

impl DoubleTapDetector {
    pub fn new(target: Modifier, interval_ms: u64) -> Self {
        Self {
            target,
            interval_ms,
            max_hold_ms: DEFAULT_MAX_HOLD_MS,
            state: State::Idle,
        }
    }

    pub fn target(&self) -> Modifier {
        self.target
    }

    /// Feeds one edge; returns `true` exactly when a double tap completes.
    pub fn feed(&mut self, edge: Edge, now_ms: u64) -> bool {
        let is_target = |key: Key| key == Key::Modifier(self.target);
        match edge {
            Edge::Down(key) if is_target(key) => {
                self.state = match self.state {
                    State::Idle => State::FirstDown { at: now_ms },
                    State::FirstUp { at } if now_ms.saturating_sub(at) <= self.interval_ms => {
                        State::SecondDown { at: now_ms }
                    }
                    State::FirstUp { .. } => State::FirstDown { at: now_ms },
                    // Auto-repeat while held: keep the original press time.
                    held => held,
                };
                false
            }
            Edge::Up(key) if is_target(key) => {
                let (next, fired) = match self.state {
                    State::FirstDown { at } if now_ms.saturating_sub(at) <= self.max_hold_ms => {
                        (State::FirstUp { at: now_ms }, false)
                    }
                    State::SecondDown { at } => {
                        (State::Idle, now_ms.saturating_sub(at) <= self.max_hold_ms)
                    }
                    _ => (State::Idle, false),
                };
                self.state = next;
                fired
            }
            Edge::Down(_) => {
                self.state = State::Idle;
                false
            }
            Edge::Up(_) => false,
        }
    }
}

/// Converts successive "currently pressed" snapshots into edges.
#[derive(Debug, Default)]
pub struct SnapshotTracker {
    previous: Vec<(Key, u32)>,
}

impl SnapshotTracker {
    /// `pressed` lists every key currently down as (classification, raw id).
    pub fn edges(&mut self, pressed: Vec<(Key, u32)>) -> Vec<Edge> {
        let mut edges = Vec::new();
        for item in &self.previous {
            if !pressed.iter().any(|p| p.1 == item.1) {
                edges.push(Edge::Up(item.0));
            }
        }
        for item in &pressed {
            if !self.previous.iter().any(|p| p.1 == item.1) {
                edges.push(Edge::Down(item.0));
            }
        }
        self.previous = pressed;
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Key = Key::Modifier(Modifier::Ctrl);
    const SHIFT: Key = Key::Modifier(Modifier::Shift);

    fn run(events: &[(Edge, u64)]) -> Vec<bool> {
        let mut detector = DoubleTapDetector::new(Modifier::Ctrl, DEFAULT_INTERVAL_MS);
        events
            .iter()
            .map(|(edge, at)| detector.feed(*edge, *at))
            .collect()
    }

    #[test]
    fn quick_double_tap_fires_on_second_release() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Up(CTRL), 80),
            (Edge::Down(CTRL), 200),
            (Edge::Up(CTRL), 260),
        ]);
        assert_eq!(fired, vec![false, false, false, true]);
    }

    #[test]
    fn slow_second_tap_does_not_fire_but_starts_new_sequence() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Up(CTRL), 80),
            (Edge::Down(CTRL), 800),
            (Edge::Up(CTRL), 850),
            (Edge::Down(CTRL), 1000),
            (Edge::Up(CTRL), 1050),
        ]);
        assert_eq!(fired, vec![false, false, false, false, false, true]);
    }

    #[test]
    fn ctrl_c_twice_never_fires() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Down(Key::Other), 30),
            (Edge::Up(Key::Other), 60),
            (Edge::Up(CTRL), 90),
            (Edge::Down(CTRL), 150),
            (Edge::Down(Key::Other), 170),
            (Edge::Up(Key::Other), 190),
            (Edge::Up(CTRL), 220),
        ]);
        assert!(fired.iter().all(|f| !f));
    }

    #[test]
    fn long_hold_is_not_a_tap() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Up(CTRL), 900),
            (Edge::Down(CTRL), 1000),
            (Edge::Up(CTRL), 1100),
        ]);
        assert!(fired.iter().all(|f| !f));
    }

    #[test]
    fn other_modifier_between_taps_cancels() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Up(CTRL), 50),
            (Edge::Down(SHIFT), 100),
            (Edge::Up(SHIFT), 120),
            (Edge::Down(CTRL), 150),
            (Edge::Up(CTRL), 200),
        ]);
        assert!(fired.iter().all(|f| !f));
    }

    #[test]
    fn auto_repeat_down_events_are_ignored() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Down(CTRL), 40),
            (Edge::Up(CTRL), 80),
            (Edge::Down(CTRL), 150),
            (Edge::Up(CTRL), 200),
        ]);
        assert_eq!(fired.last(), Some(&true));
    }

    #[test]
    fn triple_tap_fires_once() {
        let fired = run(&[
            (Edge::Down(CTRL), 0),
            (Edge::Up(CTRL), 50),
            (Edge::Down(CTRL), 100),
            (Edge::Up(CTRL), 150),
            (Edge::Down(CTRL), 200),
            (Edge::Up(CTRL), 250),
        ]);
        assert_eq!(fired.iter().filter(|f| **f).count(), 1);
    }

    #[test]
    fn snapshot_tracker_reports_edges() {
        let mut tracker = SnapshotTracker::default();
        assert_eq!(tracker.edges(vec![(CTRL, 17)]), vec![Edge::Down(CTRL)]);
        assert_eq!(tracker.edges(vec![(CTRL, 17)]), vec![]);
        assert_eq!(
            tracker.edges(vec![(CTRL, 17), (Key::Other, 67)]),
            vec![Edge::Down(Key::Other)]
        );
        assert_eq!(
            tracker.edges(vec![]),
            vec![Edge::Up(CTRL), Edge::Up(Key::Other)]
        );
    }
}
