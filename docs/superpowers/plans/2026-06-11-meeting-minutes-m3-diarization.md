# 会议纪要 M3(说话人分离)Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development。步骤用 `- [ ]`。

**Goal:** 把系统声音轨里的多个人按声纹分开,转写里「对方」细分成「对方·说话人1/2/3」;失败/模型缺失/关闭时退化为不分人。

**Architecture:** 系统轨改走 sherpa-rs 离线 diarization(pyannote 分段 + campplus 声纹 + 聚类)→ 得到带说话人 id 的时间段 → 逐段切片喂 SenseVoice → 标 `对方·说话人N`。麦克风轨仍走 VAD、标「我」。diarization 为集成,真机验证;Speaker 枚举/配置为 TDD。M5 清理在其后对全部行生效(组合无冲突)。

**Tech Stack:** sherpa-rs `diarize`(`Diarize::new(seg,emb,DiarizeConfig)` + `compute(samples)->Vec<Segment{start:f32,end:f32,speaker:i32}>`,秒、按起始排序)。模型已下到 `models/segmentation.onnx`(5.7MB)、`models/speaker_embedding.onnx`(27MB)。**Branch:** `2.x`。

---

## 范围
**做**:diarization 配置(分段/声纹模型路径 + 说话人数)+ 解析;`Speaker::OtherId(u32)`;`meeting::diarize` 封装;pipeline 系统轨改 diarization(失败回退 VAD+对方);session/lib 接线。
**不做**:声纹库认人名(只匿名"说话人N");麦克风端分人(麦克风=我);会议页里手动改说话人标签。

---

## Task 1: diarization 配置(TDD)

**Files:** `src/config.rs`(+测试)。

- [ ] **失败测试**(config tests):
```rust
#[test]
fn meeting_diarization_model_defaults() {
    let m = MeetingConfig::default();
    assert_eq!(m.segmentation_model, "./models/segmentation.onnx");
    assert_eq!(m.embedding_model, "./models/speaker_embedding.onnx");
    assert_eq!(m.diarization_speakers, 0); // 0=自动
}
```
- [ ] **实现**:`MeetingConfig` 在 `clean_transcript` 后加:
```rust
    /// 说话人分段模型(pyannote)路径。
    pub segmentation_model: String,
    /// 声纹嵌入模型路径。
    pub embedding_model: String,
    /// 期望说话人数;0/负=自动(按阈值聚类)。
    pub diarization_speakers: i32,
```
Default 追加:
```rust
            segmentation_model: "./models/segmentation.onnx".into(),
            embedding_model: "./models/speaker_embedding.onnx".into(),
            diarization_speakers: 0,
```
`load_resolved` 里 vad_model 解析行后追加:
```rust
        cfg.meeting.segmentation_model = resolve_model_dir(&base, &cfg.meeting.segmentation_model);
        cfg.meeting.embedding_model = resolve_model_dir(&base, &cfg.meeting.embedding_model);
```
- [ ] **通过 + 提交**:
```powershell
git add src/config.rs
git commit -m "feat(meeting): diarization 模型路径 + 说话人数 配置"
```

---

## Task 2: Speaker 加 OtherId(TDD)

**Files:** `src/meeting/transcript.rs`(+测试)。

- [ ] **失败测试**(transcript tests):
```rust
#[test]
fn speaker_otherid_label() {
    assert_eq!(Speaker::Me.label(), "我");
    assert_eq!(Speaker::Other.label(), "对方");
    assert_eq!(Speaker::OtherId(1).label(), "对方·说话人1");
    assert_eq!(Speaker::OtherId(3).label(), "对方·说话人3");
}
```
- [ ] **实现**:`Speaker` 枚举加变体 `OtherId(u32)`;`label()` 返回类型从 `&'static str` 改为 `String`:
```rust
pub enum Speaker {
    Me,
    Other,
    OtherId(u32),
}

impl Speaker {
    pub fn label(&self) -> String {
        match self {
            Speaker::Me => "我".to_string(),
            Speaker::Other => "对方".to_string(),
            Speaker::OtherId(n) => format!("对方·说话人{n}"),
        }
    }
}
```
(`label()` 现有调用都在 `format!` 里,String 兼容;`render_markdown_has_header_and_lines` 等旧测试不变仍过。)
- [ ] **通过 + 提交**:`cargo test -p voice-input meeting::transcript`
```powershell
git add src/meeting/transcript.rs
git commit -m "feat(meeting): Speaker::OtherId(对方·说话人N)+ label 返回 String"
```

---

## Task 3: diarize 封装(集成)

**Files:** Create `src/meeting/diarize.rs`;Modify `src/meeting/mod.rs`。无单测(依赖模型+音频),真机验证。

- [ ] **挂模块** `src/meeting/mod.rs` 追加:
```rust
pub mod diarize;
pub use diarize::{diarize_segments, DiarSeg};
```
- [ ] **创建 `src/meeting/diarize.rs`**:
```rust
use anyhow::{anyhow, Result};
use sherpa_rs::diarize::{Diarize, DiarizeConfig};

/// 一段说话人分离结果:起止毫秒 + 说话人编号(0 起)。
#[derive(Debug, Clone)]
pub struct DiarSeg {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: i32,
}

/// 对 16k 单声道样本做说话人分离。`speakers`>0 固定人数,否则自动(阈值聚类)。
pub fn diarize_segments(
    samples: &[f32],
    segmentation_model: &str,
    embedding_model: &str,
    speakers: i32,
) -> Result<Vec<DiarSeg>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let config = DiarizeConfig {
        num_clusters: Some(if speakers > 0 { speakers } else { -1 }),
        threshold: Some(0.5),
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    };
    let mut d = Diarize::new(segmentation_model, embedding_model, config)
        .map_err(|e| anyhow!("创建说话人分离失败(确认模型存在): {e}"))?;
    let segs = d
        .compute(samples.to_vec(), None)
        .map_err(|e| anyhow!("说话人分离计算失败: {e}"))?;
    Ok(segs
        .into_iter()
        .map(|s| DiarSeg {
            start_ms: (s.start * 1000.0) as u64,
            end_ms: (s.end * 1000.0) as u64,
            speaker: s.speaker,
        })
        .collect())
}
```
> 若 `sherpa_rs::diarize` 路径/字段不符,查 `~/.cargo/registry/src/*/sherpa-rs-*/src/diarize.rs` 对齐(`Diarize::new(seg,emb,DiarizeConfig)`、`compute(Vec<f32>, Option<..>)->Result<Vec<Segment{start:f32,end:f32,speaker:i32}>>`),**签名 `diarize_segments(&[f32],&str,&str,i32)->Result<Vec<DiarSeg>>` 不变**。
- [ ] **编译 + 提交**:`cargo build -p voice-input`
```powershell
git add src/meeting/mod.rs src/meeting/diarize.rs
git commit -m "feat(meeting): sherpa diarization 封装(samples -> 带说话人时间段)"
```

---

## Task 4: pipeline 系统轨改 diarization(集成)

**Files:** `src/meeting/pipeline.rs`。

- [ ] **改 `transcribe_meeting`**:加 diarization 参数,系统轨条件走分人。把现有签名与体改为:
```rust
use std::path::Path;
// (顶部已有的 use 保留)
use super::diarize::diarize_segments;
use super::transcript::{Line, Speaker, Transcript};

/// diarization 选项。
pub struct DiarOpts<'a> {
    pub enabled: bool,
    pub segmentation_model: &'a str,
    pub embedding_model: &'a str,
    pub speakers: i32,
}

pub fn transcribe_meeting(
    base: &str,
    mic_wav: &Path,
    system_wav: &Path,
    asr_model_dir: &str,
    language: &str,
    vad_model: &str,
    diar: DiarOpts,
) -> Result<Transcript> {
    let mut tr = SegmentTranscriber::load(asr_model_dir, language)?;
    let mic = read_wav_mono(mic_wav)?;
    let sys = read_wav_mono(system_wav)?;
    let mine = lines_for_track(&mic, Speaker::Me, vad_model, &mut tr)?;

    let theirs = if diar.enabled
        && !sys.is_empty()
        && Path::new(diar.segmentation_model).exists()
        && Path::new(diar.embedding_model).exists()
    {
        match lines_for_system_diarized(&sys, &diar, &mut tr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("说话人分离失败,退化为不分人:{e}");
                lines_for_track(&sys, Speaker::Other, vad_model, &mut tr)?
            }
        }
    } else {
        lines_for_track(&sys, Speaker::Other, vad_model, &mut tr)?
    };
    Ok(Transcript::merge(base, mine, theirs))
}

/// 系统轨:diarization 切段 → 逐段切片识别 → 标「对方·说话人N」。
fn lines_for_system_diarized(
    samples: &[f32],
    diar: &DiarOpts,
    tr: &mut SegmentTranscriber,
) -> Result<Vec<Line>> {
    let segs = diarize_segments(samples, diar.segmentation_model, diar.embedding_model, diar.speakers)?;
    let mut lines = Vec::new();
    let n = samples.len();
    for s in segs {
        let a = ((s.start_ms * 16) as usize).min(n); // ms * 16000/1000
        let b = ((s.end_ms * 16) as usize).min(n);
        if b <= a {
            continue;
        }
        let text = tr.transcribe_seg(&samples[a..b]);
        if text.trim().is_empty() {
            continue;
        }
        lines.push(Line {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            speaker: Speaker::OtherId(s.speaker as u32 + 1),
            text,
        });
    }
    Ok(lines)
}
```
(保留原有 `lines_for_track`、`read_wav_mono`、`clean_transcript`(M5)等不动。)
- [ ] **编译 + 提交**:`cargo build -p voice-input`
```powershell
git add src/meeting/pipeline.rs
git commit -m "feat(meeting): 系统轨走 diarization 分人(失败回退不分人)"
```

---

## Task 5: session/lib 接线(集成)

**Files:** `src/meeting/session.rs`、`src-tauri/src/lib.rs`、`examples/meeting_process.rs`。

- [ ] **session.rs `stop`**:再加四个参数 `diarization: bool, segmentation_model: String, embedding_model: String, diar_speakers: i32`(放 `clean: bool` 之后)。后台线程里调 `transcribe_meeting` 处补 `DiarOpts`:
```rust
            match super::pipeline::transcribe_meeting(
                &base, &paths.mic_wav, &paths.system_wav,
                &asr_model_dir, &language, &vad_model,
                super::pipeline::DiarOpts {
                    enabled: diarization,
                    segmentation_model: &segmentation_model,
                    embedding_model: &embedding_model,
                    speakers: diar_speakers,
                },
            ) {
```
(这些 String/bool/i32 owned,随 move 进闭包。)
- [ ] **lib.rs `stop_meeting`**:`sess.stop(...)` 末尾按顺序补:
```rust
            cfg.meeting.diarization,
            cfg.meeting.segmentation_model.clone(),
            cfg.meeting.embedding_model.clone(),
            cfg.meeting.diarization_speakers,
```
- [ ] **examples/meeting_process.rs**:`transcribe_meeting(...)` 调用补一个 `DiarOpts`(example 默认开分人,便于离线验证):
```rust
        voice_input::meeting::pipeline::DiarOpts {
            enabled: true,
            segmentation_model: "./models/segmentation.onnx",
            embedding_model: "./models/speaker_embedding.onnx",
            speakers: 0,
        },
```
> 若 `DiarOpts` 未从 `voice_input::meeting` 顶层导出,用 `voice_input::meeting::pipeline::DiarOpts`;需要的话在 `mod.rs` re-export `pub use pipeline::DiarOpts;`(连同 transcribe_meeting/clean_transcript)。保证 example 编译。
- [ ] **编译 + 测试 + 提交**:仓库根 `cargo build` 通过;`cargo test -p voice-input` 无回归。
```powershell
git add src/meeting/session.rs src/meeting/mod.rs src-tauri/src/lib.rs examples/meeting_process.rs
git commit -m "feat(meeting): stop/命令接线 diarization 参数,会议分人端到端"
```

---

## Task 6: 合并真机验收(M5 纠错 + M3 分人)

- [ ] dev 跑一场:**麦克风+系统声音**,系统放**两个不同人**说话的音频 + 你自己说话 → 结束。
- [ ] 等 `会议成稿`。会议页打开:
  - 完整转写里「对方」应细分成 **对方·说话人1 / 说话人2**,「我」是你;
  - 文本干净(M5 纠错生效:少错字、有标点);
  - 纪要据此生成。
- [ ] 边界:`[meeting] diarization=false` → 系统侧回退为「对方」(不分人);模型缺失时同样回退、不崩。
- [ ] 已知局限:分人靠声纹聚类,**数量/边界可能不完美**(同一人偶尔分成两个、或两人合一);`diarization_speakers` 设成已知人数可提升准确度。

---

## 自检
- 覆盖:配置(T1)、Speaker::OtherId(T2)、diarize 封装(T3)、pipeline 系统轨分人+回退(T4)、接线(T5)。
- 类型一致:`diarize_segments(&[f32],&str,&str,i32)->Result<Vec<DiarSeg>>`、`DiarSeg{start_ms,end_ms,speaker}`、`Speaker::OtherId(u32)`/`label()->String`、`DiarOpts`、`transcribe_meeting(...,DiarOpts)`、`stop(...,clean,diarization,seg,emb,diar_speakers)`。
- 风险:① sherpa diarize API 对齐(T3 标了查法);② diarization 段可能较长——SenseVoice 逐段识别,极长段质量略降(可接受;后续可加 max 切分);③ 失败/缺模型/关闭三重回退,绝不崩。④ 与 M5 组合:transcribe_meeting 出分人原始行 → stop 里 clean_transcript 清理全部行 → 出 json/纪要。
