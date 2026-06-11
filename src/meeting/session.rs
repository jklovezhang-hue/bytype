use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use chrono::{Datelike, Timelike};
use crate::config::{AudioRetention, RecordMode};
use super::capture_mic::MicCapture;
use super::capture_system::SystemCapture;
use super::mix::mix_tracks;
use super::mode::record_behavior;
use super::mp3::encode_mp3_file;
use super::naming::meeting_base_name;

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

/// 进行中的一场会议。
pub struct MeetingSession {
    paths: MeetingPaths,
    mic: Option<MicCapture>,
    system: Option<SystemCapture>,
}

impl MeetingSession {
    /// 取当前本地时间生成基名(放调用边界,纯逻辑不碰时钟)。
    fn now_base() -> String {
        let now = chrono::Local::now();
        meeting_base_name(
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
        )
    }

    /// 开始一场会议:建文件夹、按模式起采集。
    pub fn start(mode: RecordMode, output_root: &Path) -> Result<MeetingSession> {
        let base = Self::now_base();
        let paths = MeetingPaths::new(output_root, &base);
        std::fs::create_dir_all(&paths.dir)
            .with_context(|| format!("建会议文件夹失败: {}", paths.dir.display()))?;
        let b = record_behavior(mode);
        let mic = if b.capture_mic {
            Some(MicCapture::start(paths.mic_wav.clone())?)
        } else {
            None
        };
        let system = if b.capture_system {
            Some(SystemCapture::start(paths.system_wav.clone())?)
        } else {
            None
        };
        Ok(MeetingSession { paths, mic, system })
    }

    /// 结束:停采集 → 混音 MP3 → 后台转写(写 md/json)→ 按保留档删原始轨。
    /// 立即返回 mp3 路径;转写在后台线程完成并打印日志。
    pub fn stop(
        self,
        retention: AudioRetention,
        bitrate: u32,
        asr_model_dir: String,
        language: String,
        vad_model: String,
        llm: crate::config::LlmConfig,
        minutes_prompt: String,
        clean: bool,
    ) -> Result<PathBuf> {
        if let Some(m) = self.mic {
            m.stop()?;
        }
        if let Some(s) = self.system {
            s.stop()?;
        }

        let mut tracks: Vec<Vec<i16>> = Vec::new();
        for wav in [&self.paths.mic_wav, &self.paths.system_wav] {
            if wav.exists() {
                let mut r = hound::WavReader::open(wav)
                    .with_context(|| format!("读 WAV 失败: {}", wav.display()))?;
                let samples: Vec<i16> = r.samples::<i16>().filter_map(|s| s.ok()).collect();
                tracks.push(samples);
            }
        }
        let mixed = mix_tracks(&tracks);
        encode_mp3_file(&self.paths.mp3, &mixed, bitrate)?;

        let paths = self.paths.clone();
        let base = paths
            .dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        std::thread::spawn(move || {
            eprintln!("会议转写中…(可能数分钟)");
            match super::pipeline::transcribe_meeting(
                &base,
                &paths.mic_wav,
                &paths.system_wav,
                &asr_model_dir,
                &language,
                &vad_model,
            ) {
                Ok(mut t) => {
                    if clean && llm.enabled && !llm.base_url.trim().is_empty() {
                        if let Ok(c) = crate::corrector::Corrector::new(llm.clone()) {
                            super::pipeline::clean_transcript(&mut t, &c);
                        }
                    }
                    let json = paths.dir.join(format!("{base}.json"));
                    let _ = std::fs::write(&json, t.to_json());
                    let minutes = if llm.enabled && !llm.base_url.trim().is_empty() {
                        let input = super::minutes::transcript_to_input(&t);
                        match crate::corrector::generate_minutes(&llm, &minutes_prompt, &input) {
                            Ok(m) => Some(m),
                            Err(e) => {
                                eprintln!("会议纪要生成失败(转写已保存):{e}");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let md = paths.dir.join(format!("{base}.md"));
                    let _ = std::fs::write(
                        &md,
                        super::minutes::assemble_md(&base, minutes.as_deref(), &t),
                    );
                    eprintln!(
                        "会议成稿:{}({} 行转写{})",
                        md.display(),
                        t.lines.len(),
                        if minutes.is_some() { " + 纪要" } else { "" }
                    );
                }
                Err(e) => eprintln!("会议转写失败(录音与 MP3 已保留):{e}"),
            }
            for p in plan_retention(&paths, retention) {
                let _ = std::fs::remove_file(p);
            }
            if retention == AudioRetention::None {
                let _ = std::fs::remove_file(&paths.mp3);
            }
        });

        Ok(self.paths.mp3.clone())
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
