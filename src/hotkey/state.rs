/// 进入状态机的归一化事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 配置的热键(Win)按下
    Down,
    /// 按住热键期间,任意其他键按下
    Other,
    /// 配置的热键释放,附带本次按住时长(毫秒)
    Up { held_ms: u64 },
}

/// 状态机给出的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    DiscardRecording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    /// 是否在钩子层吞掉该事件(return 1)
    pub suppress: bool,
}

/// 低于该时长的"单独 Win"按住视为误触。
pub const MIN_HOLD_MS: u64 = 300;

#[derive(Debug, Default)]
pub struct HotkeyState {
    pressed: bool,
    combo: bool,
}

impl HotkeyState {
    pub fn handle(&mut self, event: Event) -> Decision {
        match event {
            Event::Down => {
                if self.pressed {
                    Decision { action: Action::None, suppress: false }
                } else {
                    self.pressed = true;
                    self.combo = false;
                    Decision { action: Action::StartRecording, suppress: false }
                }
            }
            Event::Other => {
                if self.pressed && !self.combo {
                    self.combo = true;
                    Decision { action: Action::CancelRecording, suppress: false }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::Up { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                self.pressed = false;
                self.combo = false;
                if was_combo {
                    Decision { action: Action::None, suppress: false }
                } else if held_ms >= MIN_HOLD_MS {
                    Decision { action: Action::StopAndTranscribe, suppress: true }
                } else {
                    Decision { action: Action::DiscardRecording, suppress: true }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_press_then_release_transcribes_and_suppresses_up() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::Down),
            Decision { action: Action::StartRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::Up { held_ms: 1000 }),
            Decision { action: Action::StopAndTranscribe, suppress: true }
        );
    }

    #[test]
    fn too_short_lone_press_discards_but_still_suppresses() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Up { held_ms: 100 }),
            Decision { action: Action::DiscardRecording, suppress: true }
        );
    }

    #[test]
    fn other_key_makes_it_a_combo_and_cancels() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Other),
            Decision { action: Action::CancelRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::Up { held_ms: 1000 }),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn repeated_down_while_held_is_noop() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Down),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn other_key_when_not_pressed_is_ignored() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::Other),
            Decision { action: Action::None, suppress: false }
        );
    }
}
