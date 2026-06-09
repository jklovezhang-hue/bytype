# ByType G3 — 录音提示音 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 录音开始播一个"叮"、松手结束播一个"咚"(可开关、可换自定义 wav),取消/短按丢弃不响。

**Architecture:** 新增核心模块 `src/sound.rs`,用 Windows `PlaySoundW` 异步播放 wav(内置默认音 `include_bytes!` 嵌入,自定义路径走 `SND_FILENAME`)。引擎在已有的 `StartRecording` / `Stop*` 触发点调用,由 `[sound] enabled` 控制。默认音由 `examples/gen_sounds.rs` 用 `hound` 合成并提交。

**Tech Stack:** Rust 核心 crate;`windows` crate(加 `Win32_Media_Audio`);`hound`(已是 dev-dependency,仅 example 用)。

**设计文档:** `docs/superpowers/specs/2026-06-10-g3-sound-cues-design.md`

---

## 构建环境前置(每个 cargo 命令前,PowerShell)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

仓库根:`C:\Users\jklov\OneDrive\Documents\Claude code Project\voice-input`。分支 `g3-sound-cues`(已建,设计文档已提交)。

---

## Task 1: 配置 `[sound]`(TDD)

**Files:**
- Modify: `src/config.rs`(新增 `SoundConfig`;`Config` 加 `sound` 字段;`resolve_sound_path` + `load_resolved` 解析;加测试)
- Modify: `config.example.toml`

- [ ] **Step 1: 写失败测试**

在 `src/config.rs` 的 `mod tests` 内,`overlay_can_be_disabled` 测试之后追加:

```rust
    #[test]
    fn sound_defaults_enabled_paths_empty() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.sound.enabled);
        assert!(cfg.sound.start_sound.is_empty());
        assert!(cfg.sound.end_sound.is_empty());
    }

    #[test]
    fn sound_can_be_disabled_and_pathed() {
        let cfg: Config =
            toml::from_str("[sound]\nenabled = false\nstart_sound = \"a.wav\"\n").unwrap();
        assert!(!cfg.sound.enabled);
        assert_eq!(cfg.sound.start_sound, "a.wav");
    }

    #[test]
    fn resolve_sound_path_empty_stays_empty_else_absolute() {
        let base = Path::new("C:\\base");
        assert_eq!(resolve_sound_path(base, ""), "");
        assert_eq!(resolve_sound_path(base, "   "), "");
        let r = resolve_sound_path(base, "snd\\a.wav");
        assert!(Path::new(&r).is_absolute());
        assert!(r.contains("base"));
        // 绝对路径原样返回
        assert_eq!(resolve_sound_path(base, "C:\\x\\a.wav"), "C:\\x\\a.wav");
    }
```

- [ ] **Step 2: 运行确认失败**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input config:: 2>&1 | Select-Object -Last 15
```
Expected: 编译失败,`no field sound` / `cannot find function resolve_sound_path`。

- [ ] **Step 3: 实现**

3a. 在 `Config` 结构体里,`overlay` 字段之后(结构体 `}` 之前)加:

```rust
    pub overlay: OverlayConfig,
    pub sound: SoundConfig,
}
```

3b. 在 `impl Default for Config` 里,`overlay: OverlayConfig::default(),` 之后加:

```rust
            overlay: OverlayConfig::default(),
            sound: SoundConfig::default(),
        }
    }
}
```

3c. 在 `OverlayConfig` 的 `Default` 实现之后,新增类型与默认:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SoundConfig {
    /// 是否播放录音开始/结束提示音。
    pub enabled: bool,
    /// 自定义开始音 wav 路径;留空用内置默认。
    pub start_sound: String,
    /// 自定义结束音 wav 路径;留空用内置默认。
    pub end_sound: String,
}

impl Default for SoundConfig {
    fn default() -> Self {
        SoundConfig {
            enabled: true,
            start_sound: String::new(),
            end_sound: String::new(),
        }
    }
}
```

3d. 在 `resolve_model_dir` 函数之后,新增提示音路径解析(空保持空):

```rust
/// 解析提示音路径:空字符串保持空(用内置默认);非空相对 base 解析为绝对。
pub fn resolve_sound_path(base: &Path, p: &str) -> String {
    if p.trim().is_empty() {
        String::new()
    } else {
        resolve_model_dir(base, p)
    }
}
```

3e. 在 `load_resolved` 里,`cfg.asr.model_dir = resolve_model_dir(&base, &cfg.asr.model_dir);` 之后加两行:

```rust
        cfg.asr.model_dir = resolve_model_dir(&base, &cfg.asr.model_dir);
        cfg.sound.start_sound = resolve_sound_path(&base, &cfg.sound.start_sound);
        cfg.sound.end_sound = resolve_sound_path(&base, &cfg.sound.end_sound);
        Ok(cfg)
```

- [ ] **Step 4: 运行确认通过**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input config:: 2>&1 | Select-Object -Last 20
```
Expected: 全绿(含三个新测试)。

- [ ] **Step 5: 更新模板并提交**

把以下追加到 `config.example.toml` 末尾(先确保前面有空行):

```toml

[sound]
# 录音开始/结束提示音。enabled = false 关闭。
# start_sound / end_sound 留空用内置默认;填 wav 路径(相对 config 目录或绝对)可覆盖。
enabled = true
start_sound = ""
end_sound = ""
```

```powershell
git add src/config.rs config.example.toml; git commit -m @'
feat(g3): 配置加 [sound](enabled/start_sound/end_sound)

默认 enabled=true、路径空;load_resolved 把非空提示音路径相对 config 目录解析为绝对。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 2: 合成默认提示音(example + 提交 wav 资产)

**Files:**
- Create: `examples/gen_sounds.rs`
- Create: `assets/sounds/start.wav`, `assets/sounds/end.wav`(由 example 生成后提交)

- [ ] **Step 1: 写 `examples/gen_sounds.rs`**

```rust
//! 合成两个柔和提示音并写入 assets/sounds/。
//! 重新生成:cargo run --example gen_sounds
//! 开始音=上行两音(亮),结束音=下行两音(沉);正弦 + 快起音 + 指数衰减,柔和电平。

use std::f32::consts::PI;

fn main() -> anyhow::Result<()> {
    std::fs::create_dir_all("assets/sounds")?;
    // (频率 Hz, 时长 s)序列,依次拼接
    write_tone("assets/sounds/start.wav", &[(880.0, 0.07), (1318.5, 0.10)])?; // A5 → E6 上行
    write_tone("assets/sounds/end.wav", &[(659.3, 0.07), (440.0, 0.12)])?; // E5 → A4 下行
    println!("wrote assets/sounds/start.wav and assets/sounds/end.wav");
    Ok(())
}

fn write_tone(path: &str, notes: &[(f32, f32)]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    let sr = 44_100.0_f32;
    let peak = 0.3_f32; // 柔和电平
    let attack = 0.005_f32; // 5ms 起音,避免爆音
    for &(freq, dur) in notes {
        let n = (dur * sr) as usize;
        for s in 0..n {
            let t = s as f32 / sr;
            let env = if t < attack {
                t / attack
            } else {
                (-(t - attack) * 6.0).exp() // 指数衰减
            };
            let sample = (2.0 * PI * freq * t).sin() * env * peak;
            w.write_sample((sample * i16::MAX as f32) as i16)?;
        }
    }
    w.finalize()?;
    Ok(())
}
```

> `hound` 是 dev-dependency,`anyhow` 是普通依赖;example 两者都能用。

- [ ] **Step 2: 生成 wav**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo run --example gen_sounds 2>&1 | Select-Object -Last 5
```
Expected: 打印 `wrote assets/sounds/start.wav and assets/sounds/end.wav`。

- [ ] **Step 3: 校验文件存在且非空**

```powershell
Get-ChildItem assets/sounds
```
Expected: `start.wav` 与 `end.wav` 都存在,大小数 KB(非 0)。

- [ ] **Step 4: 提交 example 与资产**

```powershell
git add examples/gen_sounds.rs assets/sounds/start.wav assets/sounds/end.wav; git commit -m @'
feat(g3): 合成默认提示音(gen_sounds example + assets/sounds/*.wav)

开始音上行(A5→E6)、结束音下行(E5→A4),正弦+衰减包络,柔和电平,16-bit 单声道 44.1k。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 3: `src/sound.rs` —— PlaySoundW 播放(选择逻辑 TDD)

**Files:**
- Create: `src/sound.rs`
- Modify: `src/lib.rs`(`pub mod sound;`)
- Modify: `Cargo.toml`(windows features 加 `Win32_Media_Audio`)

- [ ] **Step 1: `Cargo.toml` 给 windows 加 feature**

在根 `Cargo.toml` 的 `[dependencies.windows]` 的 `features` 数组里,`"Win32_Security",` 之后加一行:

```toml
    "Win32_Security",
    "Win32_Media_Audio",
]
```

- [ ] **Step 2: 写 `src/sound.rs`**(含选择逻辑的失败测试 + 实现)

```rust
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
```

> 若 `PlaySoundW(..., None, ...)` 编译报 hmod 参数类型不符(windows 0.58 期望 `HMODULE` 而非 `Option`),
> 把两处 `None` 改为 `windows::Win32::Foundation::HMODULE::default()` 再编译。若 `ok.as_bool()` 报错,
> 改为 `ok.0 != 0`。

- [ ] **Step 3: `src/lib.rs` 注册模块**

在 `src/lib.rs` 顶部模块声明区(`pub mod engine;` 等附近)加:

```rust
pub mod sound;
```

- [ ] **Step 4: 编译 + 跑测试**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 10
```
Expected: 编译通过,`test result: ok`,含 `sound::tests::from_config_picks_embedded_when_empty_else_file`。

- [ ] **Step 5: 提交**

```powershell
git add src/sound.rs src/lib.rs Cargo.toml Cargo.lock; git commit -m @'
feat(g3): src/sound.rs —— PlaySoundW 异步播放提示音

内置默认 wav 用 SND_MEMORY、自定义路径用 SND_FILENAME;空路径选内置默认。
windows crate 加 Win32_Media_Audio。best-effort,失败仅记日志。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 4: 引擎接线(在开始/结束触发点播放)

**Files:**
- Modify: `src/engine.rs`

- [ ] **Step 1: 加导入**

在 `src/engine.rs` 顶部的 `use crate::...` 块里,`use crate::keys::vk_from_name;` 之后加:

```rust
use crate::keys::vk_from_name;
use crate::sound::SoundPlayer;
```

- [ ] **Step 2: 循环前构建 player**

把 `run_with` 里这一行:

```rust
    let mut recorder: Option<Recorder> = None;
```

改为(前面插入按 enabled 构建的 player):

```rust
    let player = if config.sound.enabled {
        Some(SoundPlayer::from_config(&config.sound))
    } else {
        None
    };
    let mut recorder: Option<Recorder> = None;
```

- [ ] **Step 3: 开始录音时播开始音**

把 `StartRecording` 的 `Ok` 分支:

```rust
                Ok(r) => {
                    recorder = Some(r);
                    observer.on_state(OverlayState::Recording);
                }
```

改为:

```rust
                Ok(r) => {
                    recorder = Some(r);
                    if let Some(p) = &player {
                        p.play_start();
                    }
                    observer.on_state(OverlayState::Recording);
                }
```

- [ ] **Step 4: 取到录音(松手结束)时播结束音**

把 `Stop*` 分支里这两行:

```rust
                let Some(r) = recorder.take() else { continue };
                observer.on_state(OverlayState::Processing);
```

改为(在 take 成功后、Processing 前播放):

```rust
                let Some(r) = recorder.take() else { continue };
                if let Some(p) = &player {
                    p.play_end();
                }
                observer.on_state(OverlayState::Processing);
```

- [ ] **Step 5: 编译 + 跑测试**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 6
```
Expected: `test result: ok`。再确认整 workspace 可编译:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build 2>&1 | Select-Object -Last 6
```
Expected: `Finished`。

- [ ] **Step 6: 提交**

```powershell
git add src/engine.rs; git commit -m @'
feat(g3): 引擎在录音开始/结束播放提示音

按 [sound] enabled 构建 SoundPlayer;StartRecording 成功→play_start,
Stop* 取到录音→play_end;取消/丢弃不响。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 5: 真机端到端验证

> 手动验证(声音是 OS 集成)。用我(助手)起 `tauri dev`,你(用户)实测。

- [ ] **Step 1: 全量单测**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 6
```
Expected: 全绿。

- [ ] **Step 2: 启动应用(后台)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; npm run tauri dev
```
(run_in_background;读后台任务 `.output` 文件确认 `ByType 引擎就绪`。)

- [ ] **Step 3: 逐项实测**

  1. 按住热键 → 听到**开始音**(叮,上行);说完松手 → 听到**结束音**(咚,下行)。
  2. 录音中**点药丸 / 按 Esc 取消** → **不应有结束音**(且不出字)。
  3. **极短按一下**(< 0.3s 丢弃)→ 只可能有开始音、无结束音(可接受)。
  4. 把 `config.toml` 设 `[sound] enabled = false` 重启 → **全程无提示音**,听写照常(测完改回)。
  5. (可选)`start_sound = "相对/绝对路径.wav"` 指向一个自定义 wav 重启 → 开始音变成它。
  6. 确认**识别内容未被开始音污染**(出字正确)。

- [ ] **Step 4: 关闭后台 dev 进程**

- [ ] **Step 5: 若验证中有改动,补测并提交;无改动则无提交。** 随后按 `superpowers:finishing-a-development-branch` 收尾合并 master。

---

## 实现顺序与依赖

1(配置)独立先做;**2(生成 wav)必须在 3 之前**(`src/sound.rs` 的 `include_bytes!` 需要资产存在);3 依赖 1(`SoundConfig`)+2;4 依赖 1、3;5 依赖全部。顺序:1→2→3→4→5。
