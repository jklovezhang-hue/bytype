/// 进入状态机的归一化事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 主热键(左 Win)按下
    HotkeyDown,
    /// 主热键(左 Win)释放,附带本次按住时长(毫秒)
    HotkeyUp { held_ms: u64 },
    /// 修饰键(左 Alt)按下
    AltDown,
    /// 修饰键(左 Alt)释放
    AltUp,
    /// 其他任意键按下
    OtherDown,
}

/// 状态机给出的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    CancelRecording,
    /// 普通模式:识别 + LLM 整理后输出。
    StopAndTranscribe,
    /// 翻译模式(Win+Alt):识别 + 去语气词 + 翻译成英文后输出。
    StopAndTranslate,
    DiscardRecording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    /// 是否在钩子层吞掉该事件(return 1)
    pub suppress: bool,
}

/// 低于该时长的"单独按住"视为误触。
pub const MIN_HOLD_MS: u64 = 300;

#[derive(Debug, Default)]
pub struct HotkeyState {
    /// 左 Win 是否按住。
    pressed: bool,
    /// 本次按住期间出现过非 Alt 的其他键 → 视为组合键,取消录音。
    combo: bool,
    /// 本次按住期间(左 Alt 已先按住或按住期间按下)→ 翻译模式。
    alt_seen: bool,
    /// 左 Alt 当前物理是否按住(跨 Win 边界跟踪)。
    alt_down: bool,
    /// 我们吞掉了一次 Alt 按下,需把其对应的 Alt 弹起也吞掉,
    /// 保证系统不会看到"半截 Alt"(否则可能激活菜单)。
    alt_swallowing: bool,
}

impl HotkeyState {
    pub fn handle(&mut self, event: Event) -> Decision {
        match event {
            Event::HotkeyDown => {
                if self.pressed {
                    Decision { action: Action::None, suppress: false }
                } else {
                    self.pressed = true;
                    self.combo = false;
                    // 若 Alt 已先按住,本次即翻译模式。
                    self.alt_seen = self.alt_down;
                    Decision { action: Action::StartRecording, suppress: false }
                }
            }
            Event::AltDown => {
                self.alt_down = true;
                if self.pressed && !self.combo {
                    // Win 按住期间按下 Alt = 翻译模式;吞掉它,避免激活菜单/卡键。
                    self.alt_seen = true;
                    self.alt_swallowing = true;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::AltUp => {
                self.alt_down = false;
                if self.alt_swallowing {
                    self.alt_swallowing = false;
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
            Event::HotkeyUp { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                let was_translate = self.alt_seen;
                self.pressed = false;
                self.combo = false;
                self.alt_seen = false;
                if was_combo {
                    Decision { action: Action::None, suppress: false }
                } else if held_ms >= MIN_HOLD_MS {
                    let action = if was_translate {
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
    fn lone_press_then_release_transcribes_and_suppresses_up() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::HotkeyDown),
            Decision { action: Action::StartRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranscribe, suppress: true }
        );
    }

    #[test]
    fn too_short_lone_press_discards_but_still_suppresses() {
        let mut s = HotkeyState::default();
        s.handle(Event::HotkeyDown);
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 100 }),
            Decision { action: Action::DiscardRecording, suppress: true }
        );
    }

    #[test]
    fn other_key_makes_it_a_combo_and_cancels() {
        let mut s = HotkeyState::default();
        s.handle(Event::HotkeyDown);
        assert_eq!(
            s.handle(Event::OtherDown),
            Decision { action: Action::CancelRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 1000 }),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn repeated_down_while_held_is_noop() {
        let mut s = HotkeyState::default();
        s.handle(Event::HotkeyDown);
        assert_eq!(
            s.handle(Event::HotkeyDown),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn other_key_when_not_pressed_is_ignored() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::OtherDown),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn win_then_alt_translates_and_swallows_alt_down_and_up() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::HotkeyDown),
            Decision { action: Action::StartRecording, suppress: false }
        );
        // Win 按住期间按 Alt:吞掉、进翻译模式
        assert_eq!(
            s.handle(Event::AltDown),
            Decision { action: Action::None, suppress: true }
        );
        // Alt 先于 Win 松开:对应弹起也要吞掉
        assert_eq!(
            s.handle(Event::AltUp),
            Decision { action: Action::None, suppress: true }
        );
        // Win 松开:翻译 + 吞掉物理弹起
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranslate, suppress: true }
        );
    }

    #[test]
    fn alt_held_before_win_also_translates() {
        let mut s = HotkeyState::default();
        // Alt 先按(Win 未按住)→ 不吞、放行
        assert_eq!(
            s.handle(Event::AltDown),
            Decision { action: Action::None, suppress: false }
        );
        // 再按 Win → 因 Alt 已按住,进翻译模式
        assert_eq!(
            s.handle(Event::HotkeyDown),
            Decision { action: Action::StartRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranslate, suppress: true }
        );
    }

    #[test]
    fn alt_outside_hold_is_not_suppressed() {
        let mut s = HotkeyState::default();
        // 纯粹按 Alt(没按 Win):放行,不吞,不影响正常 Alt 用法
        assert_eq!(
            s.handle(Event::AltDown),
            Decision { action: Action::None, suppress: false }
        );
        assert_eq!(
            s.handle(Event::AltUp),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn alt_during_combo_is_not_translate() {
        // 已经因其他键变成 combo 后再按 Alt,不应吞、也不进翻译
        let mut s = HotkeyState::default();
        s.handle(Event::HotkeyDown);
        s.handle(Event::OtherDown); // combo
        assert_eq!(
            s.handle(Event::AltDown),
            Decision { action: Action::None, suppress: false }
        );
        assert_eq!(
            s.handle(Event::HotkeyUp { held_ms: 1000 }),
            Decision { action: Action::None, suppress: false }
        );
    }
}
