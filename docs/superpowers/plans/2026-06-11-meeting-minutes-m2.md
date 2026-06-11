# 会议纪要 M2(离线转写流水线)Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 把录好的 `mic.wav`/`system.wav` 离线转写成带时间戳与说话人(我/对方)的 `<base>.md` + `<base>.json`,接进 `MeetingSession::stop`,并提供 `meeting_process` 离线 example。

**Architecture:** 新增 `meeting::{segment, transcribe, transcript, pipeline}`。Silero VAD 把每条轨切成带采样点的语音段 → SenseVoice 逐段识别 → 按起始时间归并(mic=「我」、system=「对方」,**M2 不分人,分人留 M3**)→ 渲染 md/json。纯逻辑(归并/渲染/时间格式/配置)TDD;VAD/ASR/流水线为集成,用离线 example + 真机验证。

**Tech Stack:** sherpa-rs(silero_vad + sense_voice)、hound(读 WAV)、SenseVoice(已装)、Silero VAD 模型(~2MB,本里程碑下载到 `models/`)。

**Branch:** `2.x`(版本已为 2.0.0)。**Spec:** `docs/superpowers/specs/2026-06-11-meeting-minutes-design.md`。

---

## 范围与不做(M2)

**做**:获取 Silero VAD 模型 + 配置项;VAD 分段;SenseVoice 逐段识别;按时间归并(我/对方);渲染 `<base>.md`(转写)+ `<base>.json`(结构化);`meeting_process` 离线 example;接进 `MeetingSession::stop`(在删原始轨**之前**跑,后台线程 + 日志进度)。

**不做(后续)**:说话人聚类分人(M3)、LLM 纪要(M4,M4 会在 `<base>.md` 前面补纪要)、会议页 UI / 进度条(M4)、模型惰性下载向导(M4/后续;M2 用配置路径 + 一个下载任务把模型放到位)。

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `src/meeting/segment.rs` | `vad_segments(samples:&[f32], model:&str) -> Result<Vec<Segment>>`,Segment{start_sample,samples} |
| `src/meeting/transcribe.rs` | `SegmentTranscriber`:封装 `asr::Transcriber`,`transcribe_seg(&[f32]) -> String` |
| `src/meeting/transcript.rs` | 类型 `Speaker`/`Line`/`Transcript`;归并两条带说话人段列表(TDD);渲染 md/json(TDD);时间 `mm:ss`(TDD) |
| `src/meeting/pipeline.rs` | `transcribe_meeting(mic_wav, system_wav, asr_model_dir, lang, vad_model) -> Result<Transcript>` 编排 |
| `examples/meeting_process.rs` | CLI:两 WAV + 模型 → 写 `<base>.md`/`.json` |

**改动**:`src/meeting/mod.rs`(挂模块 + re-export);`src/config.rs`(`MeetingConfig` 加 `vad_model`);`src/meeting/session.rs`(`stop` 接流水线)。

---

## Task 1: Silero VAD 模型到位 + 配置项(TDD 配置)

**Files:** Modify `src/config.rs`;新增模型文件到 `models/`。

- [ ] **Step 1: 下载 Silero VAD 模型**

设置构建环境后,在仓库根 PowerShell:
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
Invoke-WebRequest -Uri "https://hf-mirror.com/csukuangfj/sherpa-onnx-silero-vad/resolve/main/silero_vad.onnx" -OutFile "models\silero_vad.onnx"
# 校验:文件应 ~2MB
(Get-Item "models\silero_vad.onnx").Length
```
预期:`models\silero_vad.onnx` 存在,大小约 1.8–2.3 MB。若该镜像 404,改用 `https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx`。`models/` 已 gitignore,不入库。

- [ ] **Step 2: 写失败测试(配置默认)**

在 `src/config.rs` 的 `#[cfg(test)] mod tests` 内追加:
```rust
#[test]
fn meeting_config_has_vad_model_default() {
    let m = MeetingConfig::default();
    assert_eq!(m.vad_model, "./models/silero_vad.onnx");
}
```
Run: `cargo test -p voice-input meeting_config_has_vad_model_default` → 编译失败(无 `vad_model` 字段)。

- [ ] **Step 3: 实现**

在 `MeetingConfig` 结构体加字段(放 `archive_bitrate` 之后):
```rust
    /// Silero VAD 模型路径(相对路径按 resolve 规则解析)。
    pub vad_model: String,
```
在 `impl Default for MeetingConfig` 加:
```rust
            vad_model: "./models/silero_vad.onnx".into(),
```

- [ ] **Step 4: 通过 + 提交**

Run: `cargo test -p voice-input meeting_config` → 全 PASS。
```powershell
git add src/config.rs
git commit -m "feat(meeting): 加 vad_model 配置项(Silero VAD 模型路径)"
```

---

## Task 2: 转写数据类型 + 归并 + 渲染(TDD)

**Files:** Create `src/meeting/transcript.rs`;Modify `src/meeting/mod.rs`;Test 同文件。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 追加:
```rust
pub mod transcript;
pub use transcript::{Line, Speaker, Transcript};
```

- [ ] **Step 2: 写失败测试**

创建 `src/meeting/transcript.rs`:
```rust
use serde::{Deserialize, Serialize};

/// 说话人(M2:仅区分我/对方;M3 再细分对方个体)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speaker {
    Me,
    Other,
}

impl Speaker {
    pub fn label(&self) -> &'static str {
        match self {
            Speaker::Me => "我",
            Speaker::Other => "对方",
        }
    }
}

/// 一行转写:起止毫秒 + 说话人 + 文本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Speaker,
    pub text: String,
}

/// 一场会议的完整转写。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub base: String,
    pub lines: Vec<Line>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: u64, sp: Speaker, t: &str) -> Line {
        Line { start_ms, end_ms: start_ms + 1000, speaker: sp, text: t.into() }
    }

    #[test]
    fn merge_orders_by_start_time() {
        let mine = vec![line(0, Speaker::Me, "你好"), line(5000, Speaker::Me, "好的")];
        let theirs = vec![line(2000, Speaker::Other, "在吗")];
        let t = Transcript::merge("m", mine, theirs);
        let starts: Vec<u64> = t.lines.iter().map(|l| l.start_ms).collect();
        assert_eq!(starts, vec![0, 2000, 5000]);
        assert_eq!(t.lines[1].speaker, Speaker::Other);
        assert_eq!(t.base, "m");
    }

    #[test]
    fn merge_skips_empty_text() {
        let mine = vec![line(0, Speaker::Me, "  "), line(1000, Speaker::Me, "在")];
        let t = Transcript::merge("m", mine, vec![]);
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.lines[0].text, "在");
    }

    #[test]
    fn ms_to_clock_formats() {
        assert_eq!(super::ms_to_clock(0), "00:00");
        assert_eq!(super::ms_to_clock(5000), "00:05");
        assert_eq!(super::ms_to_clock(65000), "01:05");
        assert_eq!(super::ms_to_clock(3_725_000), "1:02:05");
    }

    #[test]
    fn render_markdown_has_header_and_lines() {
        let t = Transcript {
            base: "2026-06-11_120000".into(),
            lines: vec![line(0, Speaker::Me, "你好"), line(2000, Speaker::Other, "在")],
        };
        let md = t.to_markdown();
        assert!(md.contains("# 会议转写 2026-06-11_120000"));
        assert!(md.contains("`[00:00]` **我**:你好"));
        assert!(md.contains("`[00:02]` **对方**:在"));
    }

    #[test]
    fn render_json_roundtrips() {
        let t = Transcript {
            base: "b".into(),
            lines: vec![line(0, Speaker::Me, "hi")],
        };
        let json = t.to_json();
        let back: Transcript = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
```

- [ ] **Step 3: 跑确认失败**

Run: `cargo test -p voice-input meeting::transcript` → 编译失败(`merge`/`to_markdown`/`to_json`/`ms_to_clock` 未定义)。

- [ ] **Step 4: 实现(加在 `Transcript` 定义之后、tests 之前)**

```rust
/// 毫秒 → 时钟串:>=1 小时为 H:MM:SS,否则 MM:SS。
pub fn ms_to_clock(ms: u64) -> String {
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

impl Transcript {
    /// 归并两条带说话人的行列表,按起始时间升序;丢弃空文本行(trim 后为空)。
    pub fn merge(base: &str, mine: Vec<Line>, theirs: Vec<Line>) -> Transcript {
        let mut lines: Vec<Line> = mine
            .into_iter()
            .chain(theirs)
            .filter(|l| !l.text.trim().is_empty())
            .collect();
        lines.sort_by_key(|l| l.start_ms);
        Transcript { base: base.to_string(), lines }
    }

    /// 渲染为 Markdown(M2:仅转写;M4 会在前面补纪要)。
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# 会议转写 {}\n\n", self.base);
        for l in &self.lines {
            out.push_str(&format!(
                "`[{}]` **{}**:{}\n\n",
                ms_to_clock(l.start_ms),
                l.speaker.label(),
                l.text.trim()
            ));
        }
        out
    }

    /// 渲染为结构化 JSON(供重新生成纪要)。
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}
```

- [ ] **Step 5: 通过 + 提交**

Run: `cargo test -p voice-input meeting::transcript` → 全 PASS。
```powershell
git add src/meeting/mod.rs src/meeting/transcript.rs
git commit -m "feat(meeting): 转写类型 + 归并(我/对方)+ md/json 渲染(TDD)"
```

---

## Task 3: VAD 分段(集成)

**Files:** Create `src/meeting/segment.rs`;Modify `src/meeting/mod.rs`。无单测(依赖模型+音频),用 example/真机验证。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 追加:
```rust
pub mod segment;
pub use segment::{vad_segments, Segment};
```

- [ ] **Step 2: 实现**

创建 `src/meeting/segment.rs`:
```rust
use anyhow::{anyhow, Result};
use sherpa_rs::vad::{SileroVad, SileroVadConfig};

/// 一段语音:起始采样点(16k 域)+ 单声道样本。
#[derive(Debug, Clone)]
pub struct Segment {
    pub start_sample: usize,
    pub samples: Vec<f32>,
}

impl Segment {
    pub fn start_ms(&self) -> u64 {
        (self.start_sample as u64) * 1000 / 16000
    }
    pub fn end_ms(&self) -> u64 {
        ((self.start_sample + self.samples.len()) as u64) * 1000 / 16000
    }
}

/// 用 Silero VAD 把 16k 单声道样本切成语音段。`model` 为 silero_vad.onnx 路径。
pub fn vad_segments(samples: &[f32], model: &str) -> Result<Vec<Segment>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let config = SileroVadConfig {
        model: model.to_string(),
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
        max_speech_duration: 15.0, // 单段最长 15s,避免一句太长
        threshold: 0.5,
        sample_rate: 16000,
        window_size: 512,
        provider: None,
        num_threads: Some(1),
        debug: false,
    };
    // buffer 取音频总时长 + 余量,确保容得下。
    let buffer_secs = (samples.len() as f32 / 16000.0) + 5.0;
    let mut vad = SileroVad::new(config, buffer_secs)
        .map_err(|e| anyhow!("创建 Silero VAD 失败(确认模型存在): {e}"))?;

    let mut out = Vec::new();
    let win = 512usize;
    let mut i = 0;
    while i < samples.len() {
        let end = (i + win).min(samples.len());
        vad.accept_waveform(samples[i..end].to_vec());
        while !vad.is_empty() {
            let seg = vad.front();
            out.push(Segment { start_sample: seg.start.max(0) as usize, samples: seg.samples });
            vad.pop();
        }
        i = end;
    }
    vad.flush();
    while !vad.is_empty() {
        let seg = vad.front();
        out.push(Segment { start_sample: seg.start.max(0) as usize, samples: seg.samples });
        vad.pop();
    }
    Ok(out)
}
```
> 注:`sherpa_rs` 的 VAD 模块路径可能是 `sherpa_rs::vad::*` 或 `sherpa_rs::silero_vad::*`。若 `use` 报错,运行 `cargo doc -p sherpa-rs` 或查 `~/.cargo/registry/.../sherpa-rs-*/src/lib.rs` 的 `pub mod` 确认真实路径并改 import,**类型/函数签名不变**。`SpeechSegment.start` 是 i32 采样点。

- [ ] **Step 3: 编译**

Run: `cargo build -p voice-input` → 通过。

- [ ] **Step 4: 提交**

```powershell
git add src/meeting/mod.rs src/meeting/segment.rs
git commit -m "feat(meeting): Silero VAD 分段(samples -> 带时间戳语音段)"
```

---

## Task 4: 逐段 SenseVoice 识别(集成)

**Files:** Create `src/meeting/transcribe.rs`;Modify `src/meeting/mod.rs`。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 追加:
```rust
pub mod transcribe;
pub use transcribe::SegmentTranscriber;
```

- [ ] **Step 2: 实现**

创建 `src/meeting/transcribe.rs`:
```rust
use anyhow::Result;
use crate::asr::Transcriber;

/// 会议转写器:封装一个已加载的 SenseVoice,对每段 16k 单声道样本识别。
pub struct SegmentTranscriber {
    inner: Transcriber,
}

impl SegmentTranscriber {
    /// 加载 SenseVoice(同听写引擎用的模型目录与语言)。
    pub fn load(model_dir: &str, language: &str) -> Result<SegmentTranscriber> {
        Ok(SegmentTranscriber { inner: Transcriber::load(model_dir, language)? })
    }

    /// 识别一段 16k 单声道样本;失败或空返回空串(调用方据空串过滤)。
    pub fn transcribe_seg(&mut self, samples: &[f32]) -> String {
        if samples.is_empty() {
            return String::new();
        }
        self.inner.transcribe(samples, 16000).unwrap_or_default().trim().to_string()
    }
}
```

- [ ] **Step 3: 编译 + 提交**

Run: `cargo build -p voice-input` → 通过。
```powershell
git add src/meeting/mod.rs src/meeting/transcribe.rs
git commit -m "feat(meeting): 逐段 SenseVoice 识别封装"
```

---

## Task 5: 流水线编排(集成)

**Files:** Create `src/meeting/pipeline.rs`;Modify `src/meeting/mod.rs`。

- [ ] **Step 1: 挂模块**

`src/meeting/mod.rs` 追加:
```rust
pub mod pipeline;
pub use pipeline::transcribe_meeting;
```

- [ ] **Step 2: 实现**

创建 `src/meeting/pipeline.rs`:
```rust
use std::path::Path;
use anyhow::{Context, Result};

use super::segment::{vad_segments, Segment};
use super::transcribe::SegmentTranscriber;
use super::transcript::{Line, Speaker, Transcript};

/// 读 16k 单声道 WAV 为 f32 样本(不存在则返回空)。
fn read_wav_mono(path: &Path) -> Result<Vec<f32>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut r = hound::WavReader::open(path)
        .with_context(|| format!("读 WAV 失败: {}", path.display()))?;
    let samples: Vec<f32> = r
        .samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / 32768.0)
        .collect();
    Ok(samples)
}

/// 把一条轨切段 + 识别 + 打说话人标签,产出行列表。
fn lines_for_track(
    samples: &[f32],
    speaker: Speaker,
    vad_model: &str,
    tr: &mut SegmentTranscriber,
) -> Result<Vec<Line>> {
    let segs: Vec<Segment> = vad_segments(samples, vad_model)?;
    let mut lines = Vec::new();
    for seg in segs {
        let text = tr.transcribe_seg(&seg.samples);
        if text.trim().is_empty() {
            continue;
        }
        lines.push(Line {
            start_ms: seg.start_ms(),
            end_ms: seg.end_ms(),
            speaker: speaker.clone(),
            text,
        });
    }
    Ok(lines)
}

/// 离线转写一场会议:mic→「我」,system→「对方」,按时间归并。
/// 任一轨 WAV 不存在则该轨为空(只录单源时正常)。
pub fn transcribe_meeting(
    base: &str,
    mic_wav: &Path,
    system_wav: &Path,
    asr_model_dir: &str,
    language: &str,
    vad_model: &str,
) -> Result<Transcript> {
    let mut tr = SegmentTranscriber::load(asr_model_dir, language)?;
    let mic = read_wav_mono(mic_wav)?;
    let sys = read_wav_mono(system_wav)?;
    let mine = lines_for_track(&mic, Speaker::Me, vad_model, &mut tr)?;
    let theirs = lines_for_track(&sys, Speaker::Other, vad_model, &mut tr)?;
    Ok(Transcript::merge(base, mine, theirs))
}
```

- [ ] **Step 3: 编译 + 提交**

Run: `cargo build -p voice-input` → 通过。
```powershell
git add src/meeting/mod.rs src/meeting/pipeline.rs
git commit -m "feat(meeting): 离线转写流水线(VAD→识别→归并)"
```

---

## Task 6: 离线 example(可不用真采集验证流水线)

**Files:** Create `examples/meeting_process.rs`。

- [ ] **Step 1: 实现**

创建 `examples/meeting_process.rs`:
```rust
//! 离线把一场会议的 WAV 转写成 md/json。
//! 用法: cargo run --example meeting_process -- <dir-with-base.mic.wav/.system.wav> <base>
//! 例:  cargo run --example meeting_process -- target/debug/meetings/2026-06-11_122419 2026-06-11_122419
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).expect("用法: meeting_process <dir> <base>"));
    let base = args.get(2).expect("缺 base").to_string();
    let mic = dir.join(format!("{base}.mic.wav"));
    let sys = dir.join(format!("{base}.system.wav"));
    let t = voice_input::meeting::transcribe_meeting(
        &base,
        &mic,
        &sys,
        "./models/sensevoice",
        "auto",
        "./models/silero_vad.onnx",
    )?;
    std::fs::write(dir.join(format!("{base}.md")), t.to_markdown())?;
    std::fs::write(dir.join(format!("{base}.json")), t.to_json())?;
    println!("已写 {base}.md / {base}.json,共 {} 行", t.lines.len());
    Ok(())
}
```

- [ ] **Step 2: 真机验证(需一份留存的双轨录音)**

先用托盘录一场(把 `config.toml` 的 `[meeting] audio_retention` 设 `"tracks"` 以保留 mic/system WAV),记下 `<base>`。然后:
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo run --example meeting_process -- "target\debug\meetings\<base>" "<base>"
```
打开生成的 `<base>.md`,确认:有时间戳、说话人(我/对方)、文本基本正确。

- [ ] **Step 3: 提交**

```powershell
git add examples/meeting_process.rs
git commit -m "feat(meeting): meeting_process 离线转写 example"
```

---

## Task 7: 接进 MeetingSession::stop(集成)

**Files:** Modify `src/meeting/session.rs`。

> `stop` 当前顺序:停采集 → 读 WAV 混音 → MP3 → 按保留档删原始轨。M2 要在**删原始轨之前**加转写步骤(转写需要 mic/system WAV)。转写耗时长(分钟级),放**后台线程**跑,主流程不阻塞托盘;转写完写 `<base>.md`/`.json` 并按保留档删 WAV。

- [ ] **Step 1: 重构 `stop` 让转写在删轨前、后台进行**

把 `MeetingSession::stop` 改为:停采集 → 混音 MP3(同现在)→ **spawn 后台线程**做「转写 → 写 md/json → 删原始轨(按保留档)」。返回 mp3 路径(立即),转写在后台完成并日志。

将 `stop` 签名扩展为接收转写所需的配置(asr 模型目录、语言、vad 模型路径):
```rust
    /// 结束:停采集 → 混音 MP3 → 后台转写(写 md/json)→ 按保留档删原始轨。
    /// 立即返回 mp3 路径;转写在后台线程完成并打印日志。
    pub fn stop(
        self,
        retention: AudioRetention,
        bitrate: u32,
        asr_model_dir: String,
        language: String,
        vad_model: String,
    ) -> Result<PathBuf> {
        if let Some(m) = self.mic { m.stop()?; }
        if let Some(s) = self.system { s.stop()?; }

        // 读回各轨 → 混音 → MP3(同步,快)。
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

        // 后台:转写 → 写 md/json → 删原始轨(转写依赖原始轨,故删除放最后)。
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
                Ok(t) => {
                    let md = paths.dir.join(format!("{base}.md"));
                    let json = paths.dir.join(format!("{base}.json"));
                    let _ = std::fs::write(&md, t.to_markdown());
                    let _ = std::fs::write(&json, t.to_json());
                    eprintln!("会议转写完成:{}({} 行)", md.display(), t.lines.len());
                }
                Err(e) => eprintln!("会议转写失败(录音与 MP3 已保留):{e}"),
            }
            // 转写结束后再按保留档删原始轨。
            for p in plan_retention(&paths, retention) {
                let _ = std::fs::remove_file(p);
            }
            if retention == AudioRetention::None {
                let _ = std::fs::remove_file(&paths.mp3);
            }
        });

        Ok(self.paths.mp3.clone())
    }
```
注意:`MeetingPaths` 要可 `clone()`(它已 `#[derive(Clone)]`)。删除 `stop` 旧版里同步的 `plan_retention` 循环(已移进后台线程)。`use super::pipeline;` 若未自动可见,加到文件顶部 import。

- [ ] **Step 2: 更新调用方(src-tauri/src/lib.rs 的 stop_meeting)**

`stop_meeting` 里 `sess.stop(...)` 现在要多传三个参数。改为:
```rust
    if let Some(sess) = sess {
        match sess.stop(
            cfg.meeting.audio_retention,
            cfg.meeting.archive_bitrate,
            cfg.asr.model_dir.clone(),
            cfg.asr.language.clone(),
            cfg.meeting.vad_model.clone(),
        ) {
            Ok(mp3) => eprintln!("会议结束,已存: {}", mp3.display()),
            Err(e) => eprintln!("会议结束处理失败: {e}"),
        }
    }
```

- [ ] **Step 3: 编译(整 workspace)+ 全量测试**

Run(仓库根):
```powershell
cargo build
cargo test -p voice-input
```
预期:workspace 编译通过;既有 + 新增(transcript)测试全绿。

- [ ] **Step 4: 提交**

```powershell
git add src/meeting/session.rs src-tauri/src/lib.rs
git commit -m "feat(meeting): MeetingSession.stop 接后台转写,产出 md/json"
```

---

## Task 8: 真机端到端验收(M2)

**Files:** 无(纯验证)。

- [ ] **Step 1: dev 跑一场会**

仓库根 `npm run tauri dev`(先退掉别的 ByType 实例)。托盘 → 开始会议 → 麦克风+系统;放点中文音频 + 自己说几句中文 → 结束会议。

- [ ] **Step 2: 看日志 + 产物**

引擎日志应依次出现:`会议结束,已存: ...mp3` → `会议转写中…` →(数十秒后)`会议转写完成:...md(N 行)`。
打开会议文件夹里的 `<base>.md`:确认有 `[mm:ss] 我:…` / `[mm:ss] 对方:…` 时间线,文本基本正确;`<base>.json` 为结构化数据。

- [ ] **Step 3: 边界抽查**

- 只录麦克风模式:`.md` 应只有「我」。
- 默认 `mixed` 档:转写完成后 mic/system WAV 被删、`<base>.mp3`/`.md`/`.json` 保留。

---

## 自检(对照 spec)

**1. Spec 覆盖**:VAD 分段(T3)、SenseVoice 识别(T4)、按时间归并我/对方(T2+T5)、transcript.md/json(T2+T7)、接进 stop 后台处理(T7)、离线 example(T6)、模型到位(T1)。**不含**分人(M3)、LLM 纪要(M4)、会议页(M4)—— 已在范围声明。
**2. 占位符扫描**:无 TBD;集成任务(T3/T4/T5/T7)给真实 API 代码 + 真机验收;VAD 模块路径与模型 URL 标了"以实际为准"的回退。
**3. 类型一致性**:`Segment{start_sample,samples}`+`start_ms/end_ms`、`vad_segments(&[f32],&str)`、`SegmentTranscriber::{load,transcribe_seg}`、`Speaker{Me,Other}`+`label`、`Line{start_ms,end_ms,speaker,text}`、`Transcript::{merge,to_markdown,to_json}`+`ms_to_clock`、`transcribe_meeting(base,mic,system,asr_dir,lang,vad)`、`MeetingSession::stop(retention,bitrate,asr_dir,lang,vad)` —— 跨任务一致。
**4. 关键风险**:① sherpa-rs VAD 模块导入路径(T3 标了回退查法);② SenseVoice 对每段重复 `transcribe` 的性能(1 小时会议分钟级,已放后台线程);③ 转写需原始轨,故 `stop` 中删轨移到转写之后(T7 已处理)。
