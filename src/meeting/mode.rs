use crate::config::RecordMode;

/// 某录音模式的行为:录哪些轨、是否对系统轨分人、是否挂起听写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeBehavior {
    pub capture_mic: bool,
    pub capture_system: bool,
    /// 是否对系统轨做说话人分离(M1 仅记录意图,处理在 M3)。
    pub diarize_system: bool,
    /// 录制期间是否挂起听写(用到麦克风就挂起,避免抢麦)。
    pub suspend_dictation: bool,
}

/// 录音模式 → 行为。规则:有系统声音就分人;用到麦克风就挂起听写。
pub fn record_behavior(mode: RecordMode) -> ModeBehavior {
    match mode {
        RecordMode::MicSystem => ModeBehavior {
            capture_mic: true,
            capture_system: true,
            diarize_system: true,
            suspend_dictation: true,
        },
        RecordMode::System => ModeBehavior {
            capture_mic: false,
            capture_system: true,
            diarize_system: true,
            suspend_dictation: false,
        },
        RecordMode::Mic => ModeBehavior {
            capture_mic: true,
            capture_system: false,
            diarize_system: false,
            suspend_dictation: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::record_behavior;
    use crate::config::RecordMode;

    #[test]
    fn mic_system_records_both_diarizes_and_suspends() {
        let b = record_behavior(RecordMode::MicSystem);
        assert!(b.capture_mic);
        assert!(b.capture_system);
        assert!(b.diarize_system);
        assert!(b.suspend_dictation);
    }

    #[test]
    fn system_only_records_system_diarizes_keeps_dictation() {
        let b = record_behavior(RecordMode::System);
        assert!(!b.capture_mic);
        assert!(b.capture_system);
        assert!(b.diarize_system);
        assert!(!b.suspend_dictation);
    }

    #[test]
    fn mic_only_records_mic_no_diarize_suspends() {
        let b = record_behavior(RecordMode::Mic);
        assert!(b.capture_mic);
        assert!(!b.capture_system);
        assert!(!b.diarize_system);
        assert!(b.suspend_dictation);
    }
}
