use std::path::{Path, PathBuf};
use crate::config::AudioRetention;

/// 一场会议在磁盘上的各文件路径(基于文件夹与基名)。
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingPaths {
    pub dir: PathBuf,
    pub mp3: PathBuf,
    pub mic_wav: PathBuf,
    pub system_wav: PathBuf,
}

impl MeetingPaths {
    /// 由会议根目录与基名推出所有路径(`<dir>/<base>/<base>.*`)。
    pub fn new(root: &Path, base: &str) -> MeetingPaths {
        let dir = root.join(base);
        MeetingPaths {
            mp3: dir.join(format!("{base}.mp3")),
            mic_wav: dir.join(format!("{base}.mic.wav")),
            system_wav: dir.join(format!("{base}.system.wav")),
            dir,
        }
    }
}

/// 按保留档决定:处理后应删除哪些原始 WAV(返回待删路径)。
pub fn plan_retention(paths: &MeetingPaths, retention: AudioRetention) -> Vec<PathBuf> {
    match retention {
        // none/mixed:原始双轨都删(none 连 mp3 也删,但 mp3 删除在调用处单独处理)。
        AudioRetention::None | AudioRetention::Mixed => {
            vec![paths.mic_wav.clone(), paths.system_wav.clone()]
        }
        // tracks:都留。
        AudioRetention::Tracks => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_share_base_name() {
        let p = MeetingPaths::new(Path::new("/m"), "2026-06-11_143005");
        // 用组件分别断言,避免 Windows/Unix 路径分隔符差异
        assert_eq!(p.mp3.file_name().unwrap(), "2026-06-11_143005.mp3");
        assert!(p.mp3.parent().unwrap().ends_with("2026-06-11_143005"));
        assert_eq!(p.mic_wav.file_name().unwrap(), "2026-06-11_143005.mic.wav");
        assert_eq!(p.system_wav.file_name().unwrap(), "2026-06-11_143005.system.wav");
    }

    #[test]
    fn mixed_and_none_delete_raw_tracks() {
        let p = MeetingPaths::new(Path::new("/m"), "b");
        assert_eq!(plan_retention(&p, AudioRetention::Mixed).len(), 2);
        assert_eq!(plan_retention(&p, AudioRetention::None).len(), 2);
    }

    #[test]
    fn tracks_keeps_everything() {
        let p = MeetingPaths::new(Path::new("/m"), "b");
        assert!(plan_retention(&p, AudioRetention::Tracks).is_empty());
    }
}
