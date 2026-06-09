//! 录音提示音:开始/结束各播一个 wav。Windows PlaySoundW 异步播放,best-effort(失败不影响听写)。

use std::path::PathBuf;

use crate::config::SoundConfig;

/// 内置默认提示音(合成,见 examples/gen_sounds.rs)。
static START_WAV: &[u8] = include_bytes!("../assets/sounds/start.wav");
static END_WAV: &[u8] = include_bytes!("../assets/sounds/end.wav");

#[derive(Debug)]
enum SoundSource {
    Embedded(&'static [u8]),
    File(PathBuf),
}

#[derive(Debug)]
pub struct SoundPlayer {
    start: SoundSource,
    end: SoundSource,
}

impl SoundPlayer {
    /// 由配置构建。路径已由 Config::load_resolved 解析为绝对(空 = 用内置默认)。
    pub fn from_config(cfg: &SoundConfig) -> SoundPlayer {
        SoundPlayer {
            start: pick(&cfg.start_sound, START_WAV),
            end: pick(&cfg.end_sound, END_WAV),
        }
    }

    pub fn play_start(&self) {
        play(&self.start);
    }

    pub fn play_end(&self) {
        play(&self.end);
    }
}

/// 路径空 → 内置默认;非空 → 文件。
fn pick(path: &str, embedded: &'static [u8]) -> SoundSource {
    if path.trim().is_empty() {
        SoundSource::Embedded(embedded)
    } else {
        SoundSource::File(PathBuf::from(path))
    }
}

#[cfg(windows)]
fn play(src: &SoundSource) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{
        PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT,
    };
    // SAFETY: PlaySoundW 的 FFI 调用:
    // - SND_MEMORY 分支:pszsound 被复用为指向 WAVE 字节镜像的指针(Win32 既定用法),
    //   字节为 'static(include_bytes!),SND_ASYNC 异步播放期间始终有效;PlaySound 按字节读取
    //   该镜像、不要求 u16 对齐,且 Rust 侧从不把它当 u16 解引用。
    // - SND_FILENAME 分支:系统在调用内打开并复制文件,文件名缓冲无需在调用返回后存活,
    //   因此局部 `wide` 可安全在函数结束时释放。
    let ok = unsafe {
        match src {
            SoundSource::Embedded(bytes) => PlaySoundW(
                PCWSTR(bytes.as_ptr() as *const u16),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            ),
            SoundSource::File(path) => {
                let wide: Vec<u16> =
                    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                PlaySoundW(
                    PCWSTR(wide.as_ptr()),
                    None,
                    SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
                )
            }
        }
    };
    if !ok.as_bool() {
        eprintln!("提示音播放失败");
    }
}

#[cfg(not(windows))]
fn play(_src: &SoundSource) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_picks_embedded_when_empty_else_file() {
        let cfg = SoundConfig::default(); // 两路径空
        let p = SoundPlayer::from_config(&cfg);
        assert!(matches!(p.start, SoundSource::Embedded(_)));
        assert!(matches!(p.end, SoundSource::Embedded(_)));

        let cfg2 = SoundConfig {
            enabled: true,
            start_sound: "C:\\a.wav".into(),
            end_sound: "C:\\b.wav".into(),
        };
        let p2 = SoundPlayer::from_config(&cfg2);
        assert!(matches!(p2.start, SoundSource::File(_)));
        assert!(matches!(p2.end, SoundSource::File(_)));
    }
}
