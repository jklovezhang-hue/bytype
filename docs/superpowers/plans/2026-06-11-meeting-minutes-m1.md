# 会议纪要 M1(采集地基)Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 ByType 能从托盘选三种录音模式开会、把麦克风/系统声音录成 16k WAV、结束时混音并编码成 `<base>.mp3` 存档,用麦模式期间自动挂起听写。

**Architecture:** 核心 crate 新增独立 `src/meeting/` 子系统(与 `engine.rs` 听写循环解耦);纯逻辑(重采样/混音/模式映射/命名/配置)走 TDD,OS 集成(WASAPI 环回、cpal 采集、MP3 编码、托盘、听写挂起)按项目惯例用真机验证。Tauri 层加托盘菜单驱动 `MeetingSession` 生命周期,并通过 `ControlHandle` 上的共享标志挂起听写。

**Tech Stack:** Rust(核心 crate `voice_input`)、cpal(麦克风)、`wasapi` crate(系统环回)、`hound`(WAV)、`mp3lame-encoder`(LAME 编码 MP3)、Tauri 2(托盘)。

**Branch:** 在 `2.x` 分支上实施(`main` 停在稳定 1.1.0)。

**Spec:** `docs/superpowers/specs/2026-06-11-meeting-minutes-design.md`(本计划只覆盖 M1;M2 离线流水线、M3 分人、M4 纪要+会议页另行计划)。

---

## 范围与不做(M1)

**做**:依赖接入、`[meeting]` 配置、`RecordMode` 模式与行为映射、时间戳命名、16k 线性重采样、多轨混音、流式 WAV 写入、MP3 编码、麦克风采集、WASAPI 环回采集、`MeetingSession` 生命周期、`dictation_suspended` 挂起听写、托盘三模式开始/结束 + 最小录制中提示。

**不做(后续里程碑)**:VAD 分段、SenseVoice 转写、diarization 分人、LLM 纪要、会议页 UI、历史列表、保留天数自动清理、崩溃恢复扫描、模型惰性下载。M1 结束产物只到 `<base>.mp3` + 原始 WAV(按 `audio_retention` 留删)。

---

## 文件结构

**核心 crate(`voice-input`)**

| 文件 | 职责 |
|---|---|
| `src/meeting/mod.rs` | 子系统入口,`pub mod` 各文件并 re-export 关键类型 |
| `src/meeting/mode.rs` | `RecordMode` 枚举 + `ModeBehavior`(录哪些轨/是否分人/是否挂起听写)纯映射 |
| `src/meeting/naming.rs` | 由时间戳零件生成会议基名 `YYYY-MM-DD_HHMMSS` |
| `src/meeting/resample.rs` | 线性重采样到 16k 单声道 |
| `src/meeting/mix.rs` | 多条等长 i16 轨逐采样相加 + 防削波归一 |
| `src/meeting/wav.rs` | 流式 WAV 写入器(16k 单声道 i16,基于 hound) |
| `src/meeting/mp3.rs` | 16k 单声道 i16 → MP3(mp3lame-encoder) |
| `src/meeting/capture_mic.rs` | cpal 麦克风采集 → 重采样 16k → 写 WAV |
| `src/meeting/capture_system.rs` | WASAPI 环回采集 → 重采样 16k → 写 WAV |
| `src/meeting/session.rs` | `MeetingSession`:start(mode,dir)/stop() 生命周期 + 混音 + MP3 + 留删 |

**核心 crate 既有文件改动**

| 文件 | 改动 |
|---|---|
| `src/lib.rs` | `pub mod meeting;` |
| `src/config.rs` | 加 `MeetingConfig` + `RecordMode`/`AudioRetention` 序列化枚举 + 默认值;`Config` 加 `meeting` 字段 |
| `src/engine.rs` | `ControlHandle` 加 `dictation_suspended: Arc<AtomicBool>` + setter;循环在 `StartRecording` 前检查该标志 |
| `Cargo.toml`(根) | 加 `wasapi`、`mp3lame-encoder`;`hound` 从 dev 转正 |

**Tauri 层(`bytype`)**

| 文件 | 改动 |
|---|---|
| `src-tauri/src/lib.rs` | 托盘加"开始会议"子菜单(三模式)/"结束会议";`MeetingSlot` 持有运行中的 `MeetingSession`;开始/结束时翻转 `ControlHandle` 的挂起标志 |

---

## Task 1: 依赖接入 + `[meeting]` 配置段(TDD)

**Files:**
- Modify: `Cargo.toml`(根,依赖区)
- Modify: `src/config.rs`(加枚举与 `MeetingConfig`,挂到 `Config`)
- Test: `src/config.rs`(`#[cfg(test)]` 内新增)

- [ ] **Step 1: 加依赖到根 `Cargo.toml`**

把 `[dependencies]` 区加入 3 行,并从 `[dev-dependencies]` 删除 `hound`(转正):

```toml
# 在 [dependencies] 末尾追加:
hound = "3"
wasapi = "0.15"
mp3lame-encoder = "0.2"
```

```toml
# [dev-dependencies] 改为(删掉 hound 行,保留 image):
[dev-dependencies]
image = "0.25"
```

- [ ] **Step 2: 写失败测试(配置默认值 + 往返)**

在 `src/config.rs` 的 `#[cfg(test)] mod tests` 内追加:

```rust
#[test]
fn meeting_config_defaults() {
    let m = MeetingConfig::default();
    assert_eq!(m.output_dir, "./meetings");
    assert_eq!(m.default_mode, RecordMode::MicSystem);
    assert!(m.diarization);
    assert_eq!(m.audio_retention, AudioRetention::Mixed);
    assert_eq!(m.audio_retention_days, 7);
    assert_eq!(m.archive_bitrate, 48);
}

#[test]
fn meeting_config_partial_toml_uses_defaults() {
    // 只给一个字段,其余应回落默认(serde(default))
    let cfg: Config = toml::from_str("[meeting]\naudio_retention = \"none\"\n").unwrap();
    assert_eq!(cfg.meeting.audio_retention, AudioRetention::None);
    assert_eq!(cfg.meeting.default_mode, RecordMode::MicSystem); // 默认
}

#[test]
fn record_mode_serde_roundtrip() {
    for (m, s) in [
        (RecordMode::MicSystem, "mic_system"),
        (RecordMode::System, "system"),
        (RecordMode::Mic, "mic"),
    ] {
        let toml_s = toml::to_string(&MeetingConfig { default_mode: m, ..Default::default() }).unwrap();
        assert!(toml_s.contains(s), "{toml_s} should contain {s}");
    }
}
```

- [ ] **Step 3: 运行测试确认失败**

设置构建环境(每个新 shell 都要):
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```
Run: `cargo test -p voice-input meeting_config_defaults`
Expected: 编译失败(`MeetingConfig`/`RecordMode`/`AudioRetention` 未定义)。

- [ ] **Step 4: 实现枚举与配置结构**

在 `src/config.rs` 加(放在 `ModelConfig` 之后):

```rust
/// 会议录音模式。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    /// 麦克风 + 系统声音。
    MicSystem,
    /// 只录系统声音。
    System,
    /// 只录麦克风。
    Mic,
}

/// 音频保留档。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioRetention {
    /// 都删,只剩转写+纪要。
    None,
    /// 只留 <base>.mp3 存档(默认)。
    Mixed,
    /// 留 mp3 + 双轨 WAV。
    Tracks,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct MeetingConfig {
    /// 会议文件夹根目录(相对路径按 resolve 规则解析)。
    pub output_dir: String,
    /// 开始弹窗预选的默认模式。
    pub default_mode: RecordMode,
    /// 是否分说话人(有系统轨时生效;M1 不实现,仅存配置)。
    pub diarization: bool,
    /// 音频保留档。
    pub audio_retention: AudioRetention,
    /// 自动删几天前音频(0=永久;M1 仅存配置)。
    pub audio_retention_days: u32,
    /// 存档 MP3 比特率(kbps,单声道)。
    pub archive_bitrate: u32,
}

impl Default for MeetingConfig {
    fn default() -> Self {
        MeetingConfig {
            output_dir: "./meetings".into(),
            default_mode: RecordMode::MicSystem,
            diarization: true,
            audio_retention: AudioRetention::Mixed,
            audio_retention_days: 7,
            archive_bitrate: 48,
        }
    }
}
```

把 `meeting` 挂到 `Config`:在 `struct Config { ... }` 字段末尾加 `pub meeting: MeetingConfig,`;在 `impl Default for Config` 的构造里加 `meeting: MeetingConfig::default(),`。

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p voice-input meeting_config`
Expected: `meeting_config_defaults`、`meeting_config_partial_toml_uses_defaults`、`record_mode_serde_roundtrip` 全 PASS。

- [ ] **Step 6: 提交**

```powershell
git add Cargo.toml Cargo.lock src/config.rs
git commit -m "feat(meeting): 加依赖 + [meeting] 配置段(模式/保留/比特率)"
```

---

## Task 2: `RecordMode` 行为映射(TDD)

**Files:**
- Create: `src/meeting/mod.rs`、`src/meeting/mode.rs`
- Modify: `src/lib.rs`(加 `pub mod meeting;`)
- Test: `src/meeting/mode.rs`(`#[cfg(test)]`)

- [ ] **Step 1: 挂模块**

在 `src/lib.rs` 的模块声明区(其它 `pub mod` 附近)加:
```rust
pub mod meeting;
```

创建 `src/meeting/mod.rs`:
```rust
//! 会议纪要子系统(与听写循环解耦)。M1:采集 + 混音 + MP3 存档。
pub mod mode;

pub use mode::{ModeBehavior, record_behavior};
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/mode.rs`,先只放测试:
```rust
use crate::config::RecordMode;

#[cfg(test)]
mod tests {
    use super::super::mode::record_behavior;
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
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test -p voice-input meeting::mode`
Expected: 编译失败(`record_behavior`/`ModeBehavior` 未定义)。

- [ ] **Step 4: 实现映射**

在 `src/meeting/mode.rs` 顶部(测试模块之前、`use` 之后)加:
```rust
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
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p voice-input meeting::mode`
Expected: 三个测试 PASS。

- [ ] **Step 6: 提交**

```powershell
git add src/lib.rs src/meeting/mod.rs src/meeting/mode.rs
git commit -m "feat(meeting): RecordMode 行为映射(录轨/分人/挂起听写)"
```

---

## Task 3: 会议基名(时间戳)(TDD)

**Files:**
- Create: `src/meeting/naming.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/naming.rs`

> 设计:命名函数接受"已分解的时间零件"(年月日时分秒),纯函数、可测;真正取当前时间放在调用处(`session.rs`),避免把不可测的时钟塞进纯逻辑。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod naming;
pub use naming::meeting_base_name;
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/naming.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::meeting_base_name;

    #[test]
    fn formats_zero_padded() {
        assert_eq!(meeting_base_name(2026, 6, 11, 9, 5, 3), "2026-06-11_090503");
    }

    #[test]
    fn formats_full_width() {
        assert_eq!(meeting_base_name(2026, 12, 31, 23, 59, 59), "2026-12-31_235959");
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::naming`
Expected: 编译失败(`meeting_base_name` 未定义)。

- [ ] **Step 4: 实现**

在 `src/meeting/naming.rs` 顶部加:
```rust
/// 由时间零件生成会议基名 `YYYY-MM-DD_HHMMSS`(文件夹名与夹内文件共用此基名)。
pub fn meeting_base_name(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> String {
    format!("{year:04}-{month:02}-{day:02}_{hour:02}{min:02}{sec:02}")
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p voice-input meeting::naming`
Expected: 两个测试 PASS。

- [ ] **Step 6: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/naming.rs
git commit -m "feat(meeting): 时间戳会议基名生成"
```

---

## Task 4: 线性重采样到 16k(TDD)

**Files:**
- Create: `src/meeting/resample.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/resample.rs`

> M1 自写线性重采样(简单、可测、零额外依赖);若日后音质需要再换 `rubato`。目标统一到 16000Hz 单声道 f32。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod resample;
pub use resample::resample_to_16k;
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/resample.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::resample_to_16k;

    #[test]
    fn passthrough_when_already_16k() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let out = resample_to_16k(&input, 16000);
        assert_eq!(out, input);
    }

    #[test]
    fn halves_length_from_32k() {
        // 32k → 16k:输出长度约为输入一半
        let input: Vec<f32> = (0..3200).map(|i| i as f32).collect();
        let out = resample_to_16k(&input, 32000);
        assert!((out.len() as i32 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn upsamples_from_8k() {
        let input: Vec<f32> = (0..800).map(|i| i as f32).collect();
        let out = resample_to_16k(&input, 8000);
        assert!((out.len() as i32 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn empty_input_yields_empty() {
        assert!(resample_to_16k(&[], 48000).is_empty());
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::resample`
Expected: 编译失败(`resample_to_16k` 未定义)。

- [ ] **Step 4: 实现**

在 `src/meeting/resample.rs` 顶部加:
```rust
/// 线性插值重采样单声道 f32 到 16000Hz。`src_rate` 为输入采样率。
pub fn resample_to_16k(input: &[f32], src_rate: u32) -> Vec<f32> {
    const DST: u32 = 16000;
    if input.is_empty() {
        return Vec::new();
    }
    if src_rate == DST {
        return input.to_vec();
    }
    let ratio = src_rate as f64 / DST as f64;
    let out_len = ((input.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p voice-input meeting::resample`
Expected: 四个测试 PASS。

- [ ] **Step 6: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/resample.rs
git commit -m "feat(meeting): 线性重采样到 16k 单声道"
```

---

## Task 5: 多轨混音 + 防削波归一(TDD)

**Files:**
- Create: `src/meeting/mix.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/mix.rs`

> 输入若干条 i16 单声道轨(已是 16k),逐采样相加;若峰值超 i16 上限,整体按比例缩放(防削波)。轨可不等长,按最长补零。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod mix;
pub use mix::mix_tracks;
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/mix.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::mix_tracks;

    #[test]
    fn single_track_passthrough() {
        let a = vec![100i16, -200, 300];
        assert_eq!(mix_tracks(&[a.clone()]), a);
    }

    #[test]
    fn sums_two_tracks_without_clipping() {
        let a = vec![1000i16, 2000, -1000];
        let b = vec![500i16, -500, 1000];
        assert_eq!(mix_tracks(&[a, b]), vec![1500i16, 1500, 0]);
    }

    #[test]
    fn pads_shorter_track_with_zeros() {
        let a = vec![100i16, 100, 100];
        let b = vec![50i16];
        assert_eq!(mix_tracks(&[a, b]), vec![150i16, 100, 100]);
    }

    #[test]
    fn scales_down_to_prevent_clipping() {
        // 两条接近满幅 → 和会溢出 i16,应整体缩放到不溢出且保留相对关系
        let a = vec![30000i16];
        let b = vec![20000i16];
        let out = mix_tracks(&[a, b]);
        assert_eq!(out.len(), 1);
        assert!(out[0] <= 32767 && out[0] >= 32000, "got {}", out[0]); // 接近满幅但不溢出
    }

    #[test]
    fn empty_yields_empty() {
        let empty: Vec<Vec<i16>> = vec![];
        assert!(mix_tracks(&empty).is_empty());
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::mix`
Expected: 编译失败(`mix_tracks` 未定义)。

- [ ] **Step 4: 实现**

在 `src/meeting/mix.rs` 顶部加:
```rust
/// 把若干条 i16 单声道轨混音成一条:逐采样相加(i32 累加防中途溢出),
/// 若峰值超出 i16 范围则整体线性缩放(防削波),最后量化回 i16。短轨按最长补零。
pub fn mix_tracks(tracks: &[Vec<i16>]) -> Vec<i16> {
    let len = tracks.iter().map(|t| t.len()).max().unwrap_or(0);
    if len == 0 {
        return Vec::new();
    }
    let mut acc = vec![0i32; len];
    for t in tracks {
        for (i, &s) in t.iter().enumerate() {
            acc[i] += s as i32;
        }
    }
    let peak = acc.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    let limit = i16::MAX as u32; // 32767
    let scale = if peak > limit { limit as f64 / peak as f64 } else { 1.0 };
    acc.iter()
        .map(|&v| {
            let scaled = (v as f64 * scale).round();
            scaled.clamp(i16::MIN as f64, i16::MAX as f64) as i16
        })
        .collect()
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p voice-input meeting::mix`
Expected: 五个测试 PASS。

- [ ] **Step 6: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/mix.rs
git commit -m "feat(meeting): 多轨混音 + 防削波归一"
```

---

## Task 6: 流式 WAV 写入器(hound)(集成,roundtrip 测试)

**Files:**
- Create: `src/meeting/wav.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/wav.rs`(写临时文件再读回)

> 16k 单声道 16-bit WAV。封装 hound 的 `WavWriter`,提供边采集边追加 f32(内部量化为 i16)、结束 finalize。测试用 `std::env::temp_dir()` 写读回环。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod wav;
pub use wav::WavSink;
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/wav.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::WavSink;

    #[test]
    fn writes_and_reads_back_16k_mono() {
        let path = std::env::temp_dir().join("bytype_test_wav_roundtrip.wav");
        let _ = std::fs::remove_file(&path);

        let mut sink = WavSink::create(&path).unwrap();
        sink.append_f32(&[0.0, 0.5, -0.5, 1.0]);
        sink.append_f32(&[-1.0]);
        sink.finalize().unwrap();

        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[3], i16::MAX); // 1.0 → 满幅
        assert_eq!(samples[4], i16::MIN); // -1.0 → 负满幅

        let _ = std::fs::remove_file(&path);
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::wav`
Expected: 编译失败(`WavSink` 未定义)。

- [ ] **Step 4: 实现**

在 `src/meeting/wav.rs` 顶部加:
```rust
use std::path::Path;
use anyhow::{Context, Result};

/// 16k 单声道 16-bit WAV 流式写入器。边采集边 `append_f32`,结束 `finalize`。
pub struct WavSink {
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
}

impl WavSink {
    /// 在 `path` 新建(覆盖)一个 16k 单声道 16-bit WAV。
    pub fn create(path: &Path) -> Result<WavSink> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(path, spec)
            .with_context(|| format!("创建 WAV 失败: {}", path.display()))?;
        Ok(WavSink { writer })
    }

    /// 追加一批单声道 f32 采样([-1,1]),内部量化为 i16。
    pub fn append_f32(&mut self, samples: &[f32]) {
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            let _ = self.writer.write_sample(v);
        }
    }

    /// 收尾(写入正确的头长度)。
    pub fn finalize(self) -> Result<()> {
        self.writer.finalize().context("WAV finalize 失败")?;
        Ok(())
    }
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p voice-input meeting::wav`
Expected: PASS（采样率/声道/位深正确,1.0→32767,-1.0→-32768）。

- [ ] **Step 6: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/wav.rs
git commit -m "feat(meeting): 16k 单声道流式 WAV 写入器(hound)"
```

---

## Task 7: MP3 编码(mp3lame-encoder)(集成)

**Files:**
- Create: `src/meeting/mp3.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/mp3.rs`(编码非空、产物以 MP3 帧头开头)

> 把 16k 单声道 i16 编码成 MP3 文件,比特率取 `archive_bitrate`。`mp3lame-encoder` 是 LAME 的 Rust 绑定;首次构建需在 Windows MSVC 上确认能编过(C 库)。验收除单测外,M1 末尾真机播放确认。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod mp3;
pub use mp3::encode_mp3_file;
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/mp3.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::encode_mp3_file;

    #[test]
    fn encodes_mono_16k_to_nonempty_mp3() {
        let path = std::env::temp_dir().join("bytype_test_mp3.mp3");
        let _ = std::fs::remove_file(&path);

        // 1 秒 440Hz 正弦
        let samples: Vec<i16> = (0..16000)
            .map(|i| ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 8000.0) as i16)
            .collect();
        encode_mp3_file(&path, &samples, 48).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 200, "mp3 太小: {}", bytes.len());
        // MP3 帧同步字:首帧以 0xFF 开头,次字节高 3 位为 1(0xE0 掩码)
        let has_frame = bytes.windows(2).any(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0);
        assert!(has_frame, "未发现 MP3 帧同步字");

        let _ = std::fs::remove_file(&path);
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::mp3`
Expected: 编译失败(`encode_mp3_file` 未定义)。

- [ ] **Step 4: 实现**

在 `src/meeting/mp3.rs` 顶部加(API 依 `mp3lame-encoder` 0.2;若该版本 API 名有出入,以 `cargo doc -p mp3lame-encoder --open` 为准微调,保持函数签名不变):
```rust
use std::path::Path;
use anyhow::{anyhow, Context, Result};
use mp3lame_encoder::{Builder, FlushNoGap, MonoPcm};

/// 把 16k 单声道 i16 PCM 编码为 MP3 写到 `path`,`bitrate_kbps` 如 48。
pub fn encode_mp3_file(path: &Path, pcm: &[i16], bitrate_kbps: u32) -> Result<()> {
    let mut builder = Builder::new().ok_or_else(|| anyhow!("LAME builder 创建失败"))?;
    builder.set_num_channels(1).map_err(|e| anyhow!("set_num_channels: {e:?}"))?;
    builder.set_sample_rate(16000).map_err(|e| anyhow!("set_sample_rate: {e:?}"))?;
    builder
        .set_brate(bitrate_to_lame(bitrate_kbps))
        .map_err(|e| anyhow!("set_brate: {e:?}"))?;
    let mut encoder = builder.build().map_err(|e| anyhow!("LAME build: {e:?}"))?;

    let mut out: Vec<u8> = Vec::with_capacity(pcm.len() / 2 + 7200);
    let input = MonoPcm(pcm);
    let mut buf = vec![std::mem::MaybeUninit::new(0u8); mp3lame_encoder::max_required_buffer_size(pcm.len())];
    let n = encoder
        .encode(input, &mut buf)
        .map_err(|e| anyhow!("encode: {e:?}"))?;
    out.extend(unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, n) });

    let n = encoder
        .flush::<FlushNoGap>(&mut buf)
        .map_err(|e| anyhow!("flush: {e:?}"))?;
    out.extend(unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, n) });

    std::fs::write(path, &out).with_context(|| format!("写 MP3 失败: {}", path.display()))?;
    Ok(())
}

/// 把 kbps 映射到 LAME 的 Bitrate 枚举(取最接近的常用档)。
fn bitrate_to_lame(kbps: u32) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate::*;
    match kbps {
        0..=24 => Kbps24,
        25..=32 => Kbps32,
        33..=40 => Kbps40,
        41..=48 => Kbps48,
        49..=64 => Kbps64,
        _ => Kbps96,
    }
}
```

> 注:`mp3lame-encoder` 0.2 的具体类型(`Builder`/`MonoPcm`/`Bitrate`/`max_required_buffer_size`)以实际版本为准;若编译报名称不符,运行 `cargo doc -p mp3lame-encoder` 查实际 API 并对齐,**函数签名 `encode_mp3_file(&Path, &[i16], u32)` 保持不变**(下游 Task 10 依赖它)。

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p voice-input meeting::mp3`
Expected: PASS(产物非空且含 MP3 帧同步字)。若 LAME 在 MSVC 编译失败,记录错误并升级为 BLOCKED(这是 spec 标注的验证点之一)。

- [ ] **Step 6: 提交**

```powershell
git add Cargo.lock src/meeting/mod.rs src/meeting/mp3.rs
git commit -m "feat(meeting): i16 PCM → MP3 编码(LAME)"
```

---

## Task 8: 麦克风采集到 16k WAV(cpal,OS 集成)

**Files:**
- Create: `src/meeting/capture_mic.rs`
- Modify: `src/meeting/mod.rs`
- 验证:真机(无单测;cpal 流回调依赖真实设备,按项目惯例真机验证)

> 复用 `src/audio.rs` 既有 cpal 模式(下混单声道、三种采样格式分支),但目标是**边采集边重采样到 16k 并写入 `WavSink`**,而非内存累积。提供 `MicCapture::start(path) -> MicCapture` 与 `stop()`。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod capture_mic;
pub use capture_mic::MicCapture;
```

- [ ] **Step 2: 实现采集器**

创建 `src/meeting/capture_mic.rs`:
```rust
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use super::resample::resample_to_16k;
use super::wav::WavSink;

/// 麦克风采集:cpal 默认输入设备 → 下混单声道 → 重采样 16k → 写 WAV。
pub struct MicCapture {
    stop_tx: Sender<()>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl MicCapture {
    /// 开始采集并写入 `path`(16k 单声道 WAV)。
    pub fn start(path: PathBuf) -> Result<MicCapture> {
        let (stop_tx, stop_rx) = channel::<()>();
        let (ready_tx, ready_rx) = channel::<Result<()>>();
        // cpal Stream 非 Send,必须在自己的线程里建+持有。
        let handle = std::thread::spawn(move || -> Result<()> {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err(anyhow!("没有可用麦克风")));
                    return Err(anyhow!("没有可用麦克风"));
                }
            };
            let supported = device.default_input_config()?;
            let src_rate = supported.sample_rate().0;
            let channels = supported.channels() as usize;
            let fmt = supported.sample_format();
            let config: cpal::StreamConfig = supported.into();

            let sink = std::sync::Arc::new(std::sync::Mutex::new(WavSink::create(&path)?));
            let err_fn = |e| eprintln!("麦克风流错误: {e}");

            // 把多声道帧下混单声道 → 重采样 16k → 写 sink。
            macro_rules! make_stream {
                ($t:ty, $conv:expr) => {{
                    let sink = sink.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[$t], _: &_| {
                            let mut mono = Vec::with_capacity(data.len() / channels);
                            for frame in data.chunks(channels) {
                                let sum: f32 = frame.iter().map(|&s| ($conv)(s)).sum();
                                mono.push(sum / channels as f32);
                            }
                            let r = resample_to_16k(&mono, src_rate);
                            if let Ok(mut s) = sink.lock() {
                                s.append_f32(&r);
                            }
                        },
                        err_fn,
                        None,
                    )?
                }};
            }
            let stream = match fmt {
                SampleFormat::F32 => make_stream!(f32, |s: f32| s),
                SampleFormat::I16 => make_stream!(i16, |s: i16| s as f32 / 32768.0),
                SampleFormat::U16 => make_stream!(u16, |s: u16| (s as f32 - 32768.0) / 32768.0),
                other => {
                    let _ = ready_tx.send(Err(anyhow!("不支持的麦克风格式: {other:?}")));
                    return Err(anyhow!("不支持的麦克风格式"));
                }
            };
            stream.play()?;
            let _ = ready_tx.send(Ok(()));
            // 阻塞到收到停止信号
            let _ = stop_rx.recv();
            drop(stream);
            // 取出 sink finalize
            let sink = std::sync::Arc::try_unwrap(sink)
                .map_err(|_| anyhow!("sink 仍被引用"))?
                .into_inner()
                .map_err(|_| anyhow!("sink 锁中毒"))?;
            sink.finalize()?;
            Ok(())
        });
        // 等待采集线程确认启动成功(或失败)
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(MicCapture { stop_tx, handle: Some(handle) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("麦克风采集线程未启动")),
        }
    }

    /// 停止采集并 finalize WAV。
    pub fn stop(mut self) -> Result<()> {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() {
            h.join().map_err(|_| anyhow!("麦克风线程 join 失败"))??;
        }
        Ok(())
    }
}
```

- [ ] **Step 3: 编译检查**

Run: `cargo build -p voice-input`
Expected: 编译通过(无单测;采集逻辑真机验证)。

- [ ] **Step 4: 真机冒烟验证(临时 example)**

创建临时文件 `examples/cap_mic.rs`:
```rust
fn main() -> anyhow::Result<()> {
    let path = std::path::PathBuf::from("mic_test.wav");
    let cap = voice_input::meeting::MicCapture::start(path.clone())?;
    println!("录音 3 秒...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    cap.stop()?;
    println!("已写 {}", path.display());
    Ok(())
}
```
Run: `cargo run --example cap_mic`
对着麦克风说话,用任意播放器打开 `mic_test.wav` 确认:能听到清晰人声、采样率 16k。验证后删除 `examples/cap_mic.rs` 与 `mic_test.wav`。

- [ ] **Step 5: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/capture_mic.rs
git commit -m "feat(meeting): 麦克风采集 → 16k WAV(cpal,真机验证)"
```

---

## Task 9: WASAPI 环回采集到 16k WAV(SPIKE,OS 集成)⚠️ 最大未知

**Files:**
- Create: `src/meeting/capture_system.rs`
- Modify: `src/meeting/mod.rs`
- 验证:真机(无单测)

> 这是整个功能最大的不确定点。用 `wasapi` crate 对**默认渲染设备做 loopback 捕获**,拿到的 PCM 下混单声道 → 重采样 16k → 写 WAV。接口形态与 `MicCapture` 对齐(`SystemCapture::start(path)` / `stop()`),便于 `MeetingSession` 统一使用。先以一个独立 spike example 跑通,再固化进模块。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod capture_system;
pub use capture_system::SystemCapture;
```

- [ ] **Step 2: 先写 spike example 跑通 loopback**

创建临时 `examples/cap_sys.rs`,基于 `wasapi` crate 的 loopback 流程(参考其官方 `examples/loopback.rs`):
```rust
// 目标:从"你听到的声音"(默认渲染设备 loopback)抓 3 秒 PCM,写成 16k 单声道 WAV。
// 关键步骤(wasapi 0.15):
//   wasapi::initialize_mta()?;
//   let device = wasapi::get_default_device(&wasapi::Direction::Render)?;
//   let mut client = device.get_iaudioclient()?;
//   let format = client.get_mixformat()?;            // 设备混音格式(常见 48k f32 立体声)
//   client.initialize_client(&format, /*loopback=*/true, &wasapi::Direction::Capture, &wasapi::ShareMode::Shared)?;
//   let h_event = client.set_get_eventhandle()?;
//   let capture = client.get_audiocaptureclient()?;
//   client.start_stream()?;
//   loop { h_event.wait_for_event(1000)?; capture.read_from_device_to_deque(&mut queue)?; ... }
// 注:不同 wasapi 版本 initialize_client 参数顺序/类型略有差异,以 `cargo doc -p wasapi` 为准。
fn main() -> anyhow::Result<()> {
    println!("请先播放一段音频(如浏览器视频),再运行。抓 3 秒系统声音...");
    voice_input::meeting::SystemCapture::spike_to_wav(
        std::path::PathBuf::from("sys_test.wav"),
        std::time::Duration::from_secs(3),
    )?;
    println!("已写 sys_test.wav");
    Ok(())
}
```

- [ ] **Step 3: 实现 `SystemCapture`**

创建 `src/meeting/capture_system.rs`。结构与 `MicCapture` 对齐:线程内建立 loopback 客户端,事件驱动读取,下混单声道 → `resample_to_16k` → `WavSink`;收到停止信号后 finalize。提供 `start(path) -> Result<SystemCapture>`、`stop() -> Result<()>`,以及供 spike 用的 `spike_to_wav(path, dur)`。核心读取循环骨架:
```rust
// 伪流程(真实代码以 wasapi 0.15 文档对齐;字节→f32 依 format.get_bitspersample 与 sample type):
// 1) initialize_mta
// 2) get_default_device(Render) → get_iaudioclient → get_mixformat
// 3) initialize_client(loopback=true, Direction::Capture, ShareMode::Shared)
// 4) set_get_eventhandle + get_audiocaptureclient + start_stream
// 5) 循环:wait_for_event → read_from_device_to_deque → 取出字节按 format 解码成 f32 帧
//          → 下混单声道(按 format.get_nchannels)→ resample_to_16k(.., format.get_samplespersec)
//          → WavSink::append_f32
// 6) 停止:stop_stream → sink.finalize
```
> 实现要点(避免常见坑):
> - **静音段也要补零**:loopback 在无声音播放时可能不产生数据包(`read` 返回 0 帧);M1 简单处理——没有数据就不写(WAV 会比真实时长短)。时间轴精确对齐留待 M2(用共同起始时间戳)。M1 验收只看"有声音时能录到声音"。
> - 设备混音格式通常是 f32 立体声 48k;按 `format` 实际值解码,别硬编码。
> - COM 必须在采集线程内初始化(`initialize_mta`),与 cpal 线程隔离。

- [ ] **Step 4: 真机验证 spike**

Run: 先播放一段浏览器视频,另开终端 `cargo run --example cap_sys`
用播放器打开 `sys_test.wav`,确认能听到刚才播放的系统声音、16k 采样率。
通过后:把 spike 逻辑固化进 `SystemCapture::start/stop`(与 `MicCapture` 同形),删除 `examples/cap_sys.rs` 与 `sys_test.wav`。

- [ ] **Step 5: 编译检查**

Run: `cargo build -p voice-input`
Expected: 通过。

- [ ] **Step 6: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/capture_system.rs
git commit -m "feat(meeting): WASAPI 环回采集 → 16k WAV(真机验证)"
```

> 若 loopback 在真机始终拿不到数据 / `wasapi` API 不通:升级为 BLOCKED,回到 spec 重新评估(备选:用 `windows` crate 直接写 WASAPI,即设计里的方案 B)。

---

## Task 10: `MeetingSession` 生命周期(集成)

**Files:**
- Create: `src/meeting/session.rs`
- Modify: `src/meeting/mod.rs`
- Test: `src/meeting/session.rs`(纯逻辑:产物路径规划 + 留删决策)

> `MeetingSession::start(mode, output_root)`:取当前时间生成 `<base>`、建文件夹、按 `ModeBehavior` 起 `MicCapture`/`SystemCapture`。`stop(retention, bitrate)`:停采集 → 读回各轨 WAV 样本 → `mix_tracks` → `encode_mp3_file` 写 `<base>.mp3` → 按 `audio_retention` 删/留原始 WAV。当前时间用 `std::time::SystemTime` + 本地分解(在 session 内,不进纯逻辑)。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 加:
```rust
pub mod session;
pub use session::{MeetingSession, MeetingPaths, plan_retention};
```

- [ ] **Step 2: 写失败测试(路径规划 + 留删决策,纯逻辑)**

创建 `src/meeting/session.rs`,先放纯逻辑 + 测试:
```rust
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
        assert!(p.mp3.ends_with("2026-06-11_143005/2026-06-11_143005.mp3"));
        assert!(p.mic_wav.ends_with("2026-06-11_143005.mic.wav"));
        assert!(p.system_wav.ends_with("2026-06-11_143005.system.wav"));
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
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p voice-input meeting::session`
Expected: 编译失败(类型未定义)。

- [ ] **Step 4: 运行确认通过(纯逻辑先绿)**

实现上面已给出;运行:
Run: `cargo test -p voice-input meeting::session`
Expected: 三个测试 PASS。

- [ ] **Step 5: 加 `MeetingSession`(集成部分,无单测)**

在 `src/meeting/session.rs` 追加(`tests` 模块之前):
```rust
use anyhow::{Context, Result};
use super::capture_mic::MicCapture;
use super::capture_system::SystemCapture;
use super::mode::record_behavior;
use super::mix::mix_tracks;
use super::mp3::encode_mp3_file;
use super::naming::meeting_base_name;
use crate::config::{AudioRetention, RecordMode};

/// 进行中的一场会议。
pub struct MeetingSession {
    paths: MeetingPaths,
    mic: Option<MicCapture>,
    system: Option<SystemCapture>,
}

impl MeetingSession {
    /// 取当前本地时间生成基名(放调用边界,纯逻辑不碰时钟)。
    fn now_base() -> String {
        // 用 chrono 之外的零依赖方式:SystemTime → 本地分解过于繁琐,
        // 这里用 time crate 风格的简单本地时间。M1 直接用 std 的 UTC 秒 + 平台本地偏移过重,
        // 故采用 `时:分:秒` 由 SystemTime 推。为可读性与稳定,使用 chrono(若未引入则在 Cargo 加 chrono)。
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
        use chrono::{Datelike, Timelike};
        let _ = (Datelike::year, Timelike::hour); // 占位说明:trait 引入见下
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

    /// 结束:停采集 → 混音 → MP3 → 按保留档删原始轨。返回 mp3 路径。
    pub fn stop(self, retention: AudioRetention, bitrate: u32) -> Result<PathBuf> {
        if let Some(m) = self.mic {
            m.stop()?;
        }
        if let Some(s) = self.system {
            s.stop()?;
        }
        // 读回各轨 i16 → 混音
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
        // 保留策略:删原始轨(none/mixed);none 额外删 mp3
        for p in plan_retention(&self.paths, retention) {
            let _ = std::fs::remove_file(p);
        }
        if retention == AudioRetention::None {
            let _ = std::fs::remove_file(&self.paths.mp3);
        }
        Ok(self.paths.mp3.clone())
    }
}
```

> 说明:`now_base` 用 `chrono`(本地时间最稳)。在根 `Cargo.toml` 加依赖 `chrono = "0.4"`,并在 `session.rs` 顶部 `use chrono::{Datelike, Timelike};`(删掉 Step 5 里那行占位 `let _ = ...`)。

- [ ] **Step 6: 编译 + 全量测试**

Run:
```powershell
cargo test -p voice-input
```
Expected: 既有 + 新增测试全 PASS,`MeetingSession` 编译通过。

- [ ] **Step 7: 提交**

```powershell
git add Cargo.toml Cargo.lock src/meeting/mod.rs src/meeting/session.rs
git commit -m "feat(meeting): MeetingSession 生命周期(采集→混音→MP3→留删)"
```

---

## Task 11: 听写挂起标志(engine + ControlHandle)(TDD 可测部分)

**Files:**
- Modify: `src/engine.rs`(`ControlHandle` 加共享标志 + setter;循环检查)
- Test: `src/engine.rs`(`#[cfg(test)]`:标志读写语义)

> 用 `Arc<AtomicBool>` 在 Tauri 层与引擎间共享"是否挂起听写"。引擎收到 `StartRecording` 时若已挂起则忽略(不开录音)。`ControlHandle` 已被 Tauri 通过 `ControlSlot` 持有,天然是设置入口。

- [ ] **Step 1: 写失败测试**

在 `src/engine.rs` 的 `#[cfg(test)] mod tests` 内追加:
```rust
#[test]
fn control_handle_toggles_dictation_suspend() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let flag = Arc::new(AtomicBool::new(false));
    let (tx, _rx) = crossbeam_channel::unbounded();
    let h = ControlHandle::new_for_test(tx, flag.clone());
    assert!(!flag.load(Ordering::SeqCst));
    h.set_dictation_suspended(true);
    assert!(flag.load(Ordering::SeqCst));
    h.set_dictation_suspended(false);
    assert!(!flag.load(Ordering::SeqCst));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p voice-input control_handle_toggles`
Expected: 编译失败(`new_for_test` / `set_dictation_suspended` 未定义)。

- [ ] **Step 3: 改 `ControlHandle` 与引擎循环**

在 `src/engine.rs`:把 `ControlHandle` 改为持有共享标志,并在循环里检查。

```rust
// 顶部 use 增补:
use std::sync::atomic::{AtomicBool, Ordering};

// 替换 ControlHandle 定义与 impl:
pub struct ControlHandle {
    tx: Sender<HotkeyAction>,
    dictation_suspended: Arc<AtomicBool>,
}

impl ControlHandle {
    /// 请求取消当前录音(等价于按 Esc / 点药丸)。
    pub fn cancel(&self) {
        let _ = self.tx.send(HotkeyAction::CancelRecording);
    }
    /// 设置是否挂起听写(会议用麦期间置位)。
    pub fn set_dictation_suspended(&self, on: bool) {
        self.dictation_suspended.store(on, Ordering::SeqCst);
    }
    /// 仅测试用构造。
    #[cfg(test)]
    pub fn new_for_test(tx: Sender<HotkeyAction>, flag: Arc<AtomicBool>) -> ControlHandle {
        ControlHandle { tx, dictation_suspended: flag }
    }
}
```

在 `run_with` 内创建共享标志并传入 `ControlHandle`,循环里 `StartRecording` 分支前置检查:
```rust
// 在 let (tx, rx) = unbounded(); 之后:
let dictation_suspended = Arc::new(AtomicBool::new(false));
observer.on_ready(ControlHandle { tx: tx.clone(), dictation_suspended: dictation_suspended.clone() });
```
```rust
// StartRecording 分支开头加挂起检查:
HotkeyAction::StartRecording => {
    if dictation_suspended.load(Ordering::SeqCst) {
        continue; // 会议占麦期间,忽略听写开始
    }
    match Recorder::start() {
        // ...原有逻辑不变...
    }
}
```
> 注意:原 `observer.on_ready(ControlHandle { tx: tx.clone() });` 那行要替换为上面带标志的版本。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p voice-input control_handle_toggles`
Expected: PASS。再跑 `cargo test -p voice-input` 确认既有测试不回归。

- [ ] **Step 5: 提交**

```powershell
git add src/engine.rs
git commit -m "feat(engine): ControlHandle 加听写挂起标志,会议用麦期间忽略听写"
```

---

## Task 12: 托盘三模式开始/结束 + 接线(Tauri,OS 集成)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- 验证:真机

> 托盘菜单加:"开始会议"(展开三模式子项)/"结束会议"。开始时按模式建 `MeetingSession` 存入 `MeetingSlot`,并按 `record_behavior(mode).suspend_dictation` 调 `ControlHandle::set_dictation_suspended(true)`;结束时 `stop(retention, bitrate)` 写 mp3、解除挂起、弹出完成提示。最小录制中提示:用 tray tooltip 或一条 `tauri_plugin_dialog` 消息(M1 不做会议页)。

- [ ] **Step 1: 加 `MeetingSlot` 状态**

在 `src-tauri/src/lib.rs` 顶部状态区(`EngineStarted` 附近)加:
```rust
/// 进行中的会议(None = 空闲)。
#[derive(Default)]
struct MeetingSlot(Mutex<Option<voice_input::meeting::MeetingSession>>);
```
并在 `run()` 的 `.manage(...)` 链里加 `.manage(MeetingSlot::default())`。

- [ ] **Step 2: 加开始/结束函数**

在 `src-tauri/src/lib.rs` 加(供菜单事件调用):
```rust
fn start_meeting(app: &tauri::AppHandle, mode: voice_input::config::RecordMode) {
    let cfg = match Config::load_resolved() {
        Ok(c) => c,
        Err(e) => { eprintln!("会议:加载配置失败 {e}"); return; }
    };
    // 会议根目录:相对 output_dir 落在 config 同级(与模型同规则,简化为 exe 目录旁)
    let root = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(cfg.meeting.output_dir.trim_start_matches("./"));
    match voice_input::meeting::MeetingSession::start(mode, &root) {
        Ok(sess) => {
            // 用麦模式 → 挂起听写
            if voice_input::meeting::record_behavior(mode).suspend_dictation {
                if let Some(c) = app.state::<ControlSlot>().0
                    .lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
                    c.set_dictation_suspended(true);
                }
            }
            app.state::<MeetingSlot>().0
                .lock().unwrap_or_else(|p| p.into_inner()).replace(sess);
            eprintln!("会议已开始: {mode:?}");
        }
        Err(e) => eprintln!("会议启动失败: {e}"),
    }
}

fn stop_meeting(app: &tauri::AppHandle) {
    let cfg = Config::load_resolved().unwrap_or_default();
    let sess = app.state::<MeetingSlot>().0
        .lock().unwrap_or_else(|p| p.into_inner()).take();
    // 解除听写挂起
    if let Some(c) = app.state::<ControlSlot>().0
        .lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        c.set_dictation_suspended(false);
    }
    if let Some(sess) = sess {
        match sess.stop(cfg.meeting.audio_retention, cfg.meeting.archive_bitrate) {
            Ok(mp3) => eprintln!("会议结束,已存: {}", mp3.display()),
            Err(e) => eprintln!("会议结束处理失败: {e}"),
        }
    }
}
```

- [ ] **Step 3: 托盘菜单加项**

把 `.setup(...)` 里建菜单那段改为含会议子菜单(用 `Submenu`):
```rust
use tauri::menu::{Menu, MenuItem, Submenu};
// ...
let m_mic_sys = MenuItem::with_id(app, "meet_mic_sys", "麦克风+系统声音", true, None::<&str>)?;
let m_sys = MenuItem::with_id(app, "meet_sys", "只录系统声音", true, None::<&str>)?;
let m_mic = MenuItem::with_id(app, "meet_mic", "只录麦克风", true, None::<&str>)?;
let start_meet = Submenu::with_items(app, "开始会议", true, &[&m_mic_sys, &m_sys, &m_mic])?;
let stop_meet = MenuItem::with_id(app, "meet_stop", "结束会议", true, None::<&str>)?;
let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&start_meet, &stop_meet, &settings, &quit])?;
```
并在 `.on_menu_event(...)` 的 `match` 里加分支:
```rust
"meet_mic_sys" => start_meeting(app, voice_input::config::RecordMode::MicSystem),
"meet_sys" => start_meeting(app, voice_input::config::RecordMode::System),
"meet_mic" => start_meeting(app, voice_input::config::RecordMode::Mic),
"meet_stop" => stop_meeting(app),
```

- [ ] **Step 4: 编译**

Run(在仓库根):
```powershell
cargo build
```
Expected: workspace 编译通过(core + src-tauri)。

- [ ] **Step 5: 真机端到端(见 Task 13)**

本任务的真机验证并入 Task 13。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat(meeting): 托盘三模式开始/结束会议 + 听写挂起接线"
```

---

## Task 13: 真机端到端验证(M1 验收)

**Files:** 无(纯验证)

> 在真机跑 `npm run tauri dev`(它把 CWD 设为 `src-tauri/`,会议根目录因此落在 `src-tauri/` 旁,属预期 dev 行为)。逐项确认。

- [ ] **Step 1: 启动 dev**

设置环境后,仓库根运行:
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
npm run tauri dev
```

- [ ] **Step 2: 模式一 — 麦克风+系统声音**

托盘 → 开始会议 → 麦克风+系统声音。播放一段视频 + 自己说话十几秒 → 结束会议。
确认:① 生成 `meetings/<base>/<base>.mp3`;② 播放能**同时**听到你的声音和视频声音;③ 录制期间按住 Win 说话**不出字**(听写被挂起);④ 结束后听写恢复正常。

- [ ] **Step 3: 模式二 — 只录系统声音**

开始会议 → 只录系统声音。播放视频十几秒,其间按住 Win 说一句话 → 结束。
确认:① mp3 只含视频声音(无你的话);② 录制期间听写**仍可用**(你那句话被正常注入到当前焦点)。

- [ ] **Step 4: 模式三 — 只录麦克风**

开始会议 → 只录麦克风。说话十几秒 → 结束。
确认:① mp3 只含你的声音;② 录制期间听写被挂起;③ 结束恢复。

- [ ] **Step 5: 保留档抽查**

把 `config.toml` 的 `[meeting] audio_retention` 改为 `"tracks"` 重跑一场:确认文件夹内同时留有 `<base>.mp3`、`<base>.mic.wav`、`<base>.system.wav`;改回 `"mixed"` 重跑:确认只剩 `<base>.mp3`(两条原始 WAV 被删)。

- [ ] **Step 6: 记录结果并提交(若有修复)**

把验证结论记在提交信息或 PR 描述里。若过程中改了代码:
```powershell
git add -A
git commit -m "fix(meeting): M1 真机验证发现的修复"
```

---

## 自检(写完计划后对照 spec)

**1. Spec 覆盖**
- 三模式选择 + 矩阵 → Task 2(行为映射)+ Task 12(托盘三项)✓
- 双轨/单轨按模式采集 → Task 8/9 + Task 10(MeetingSession 按 behavior 起轨)✓
- 16k 单声道 WAV、就地重采样 → Task 4 + Task 6 + Task 8/9 ✓
- 结束混音 + MP3 存档(默认保留)→ Task 5 + Task 7 + Task 10 ✓
- 同名配对 / 时间戳基名 → Task 3 + Task 10(MeetingPaths)✓
- `[meeting]` 配置(含 `audio_retention` 三档、`archive_bitrate`)→ Task 1 ✓
- 听写共存(用麦挂起、只系统放行)→ Task 11(engine 标志)+ Task 12(按 behavior 翻转)✓
- 托盘控制 → Task 12 ✓
- 保留三档留删 → Task 10(plan_retention)+ Task 13 抽查 ✓
- **M1 不含**:VAD/识别/分人/纪要/会议页/惰性下载/保留天数自动清理/崩溃恢复 —— 按 spec 属 M2–M4,已在"范围与不做"声明 ✓

**2. 占位符扫描**:无 TBD/TODO;OS 集成任务(8/9/12)给了真实 API 结构 + 真机验收步骤(符合项目"OS 代码真机验证"惯例),非空泛占位。WASAPI(Task 9)与 LAME(Task 7)是 spec 标注的两个验证点,均带 BLOCKED 升级路径。

**3. 类型一致性**:`RecordMode`/`AudioRetention`(config)、`ModeBehavior`/`record_behavior`、`meeting_base_name`、`resample_to_16k`、`mix_tracks`、`WavSink`、`encode_mp3_file(&Path,&[i16],u32)`、`MicCapture`/`SystemCapture`(`start(path)`/`stop()`)、`MeetingPaths`/`plan_retention`、`MeetingSession`(`start(mode,&Path)`/`stop(retention,bitrate)`)、`ControlHandle::set_dictation_suspended` —— 跨任务签名一致。

> **新增依赖汇总**(Task 1 与 Task 10):`hound`(转正)、`wasapi`、`mp3lame-encoder`、`chrono`。打包时这些若产出 DLL,需按 `scripts/build-installer.ps1` 的 Route B 一并 bundle(留待发布前处理,M1 dev 不涉及)。
