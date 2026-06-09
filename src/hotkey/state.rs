/// 进入状态机的归一化事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 主键按下
    PrimaryDown,
    /// 主键释放,附本次按住时长(毫秒)
    PrimaryUp { held_ms: u64 },
    /// 修饰键 A(翻译)按下/释放
    ModADown,
    ModAUp,
    /// 修饰键 B(命令)按下/释放
    ModBDown,
    ModBUp,
    /// 其他任意键按下
    OtherDown,
}

/// 状态机给出的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    StopAndTranslate,
    StopAndCommand,
    DiscardRecording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    pub suppress: bool,
}

/// 低于该时长的"单独按住"视为误触。
pub const MIN_HOLD_MS: u64 = 300;

#[derive(Debug, Default)]
pub struct HotkeyState {
    pressed: bool,
    combo: bool,
    mod_a_seen: bool,
    mod_b_seen: bool,
    mod_a_down: bool,
    mod_b_down: bool,
    mod_a_swallow: bool,
    mod_b_swallow: bool,
}

impl HotkeyState {
    pub fn handle(&mut self, event: Event) -> Decision {
        match event {
            Event::PrimaryDown => {
                if self.pressed {
                    Decision { action: Action::None, suppress: false }
                } else {
                    self.pressed = true;
                    self.combo = false;
                    self.mod_a_seen = self.mod_a_down;
                    self.mod_b_seen = self.mod_b_down;
                    Decision { action: Action::StartRecording, suppress: false }
                }
            }
            Event::ModADown => {
                self.mod_a_down = true;
                if self.pressed && !self.combo {
                    self.mod_a_seen = true;
                    self.mod_a_swallow = true;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModAUp => {
                self.mod_a_down = false;
                if self.mod_a_swallow {
                    self.mod_a_swallow = false;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModBDown => {
                self.mod_b_down = true;
                if self.pressed && !self.combo {
                    self.mod_b_seen = true;
                    self.mod_b_swallow = true;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModBUp => {
                self.mod_b_down = false;
                if self.mod_b_swallow {
                    self.mod_b_swallow = false;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::OtherDown => {
                if self.pressed && !self.combo {
                    self.combo = true;
                    Decision { action: Action::CancelRecording, suppress: false }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::PrimaryUp { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                let a = self.mod_a_seen;
                let b = self.mod_b_seen;
                self.pressed = false;
                self.combo = false;
                self.mod_a_seen = false;
                self.mod_b_seen = false;
                if was_combo {
                    Decision { action: Action::None, suppress: false }
                } else if held_ms >= MIN_HOLD_MS {
                    let action = if b {
                        Action::StopAndCommand
                    } else if a {
                        Action::StopAndTranslate
                    } else {
                        Action::StopAndTranscribe
                    };
                    Decision { action, suppress: true }
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
    fn primary_alone_transcribes() {
        let mut s = HotkeyState::default();
        assert_eq!(s.handle(Event::PrimaryDown), Decision { action: Action::StartRecording, suppress: false });
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::StopAndTranscribe, suppress: true });
    }

    #[test]
    fn too_short_discards() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 100 }), Decision { action: Action::DiscardRecording, suppress: true });
    }

    #[test]
    fn primary_plus_mod_a_translates_and_swallows() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::ModADown), Decision { action: Action::None, suppress: true });
        assert_eq!(s.handle(Event::ModAUp), Decision { action: Action::None, suppress: true });
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::StopAndTranslate, suppress: true });
    }

    #[test]
    fn primary_plus_mod_b_commands_and_swallows() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::ModBDown), Decision { action: Action::None, suppress: true });
        assert_eq!(s.handle(Event::ModBUp), Decision { action: Action::None, suppress: true });
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::StopAndCommand, suppress: true });
    }

    #[test]
    fn command_beats_translate_when_both_held() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        s.handle(Event::ModADown);
        s.handle(Event::ModBDown);
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::StopAndCommand, suppress: true });
    }

    #[test]
    fn mod_a_held_before_primary_translates() {
        let mut s = HotkeyState::default();
        assert_eq!(s.handle(Event::ModADown), Decision { action: Action::None, suppress: false });
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::StopAndTranslate, suppress: true });
    }

    #[test]
    fn other_key_cancels_as_combo() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::OtherDown), Decision { action: Action::CancelRecording, suppress: false });
        assert_eq!(s.handle(Event::PrimaryUp { held_ms: 1000 }), Decision { action: Action::None, suppress: false });
    }

    #[test]
    fn mods_outside_hold_pass_through() {
        let mut s = HotkeyState::default();
        assert_eq!(s.handle(Event::ModADown), Decision { action: Action::None, suppress: false });
        assert_eq!(s.handle(Event::ModBDown), Decision { action: Action::None, suppress: false });
        assert_eq!(s.handle(Event::ModAUp), Decision { action: Action::None, suppress: false });
        assert_eq!(s.handle(Event::ModBUp), Decision { action: Action::None, suppress: false });
    }

    #[test]
    fn repeated_primary_down_is_noop() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(s.handle(Event::PrimaryDown), Decision { action: Action::None, suppress: false });
    }
}
