# ByType G2 — 录音浮窗(波形药丸)Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按住热键录音时,屏幕底部正中浮出半透明"秒计时+波形"药丸;鼠标点药丸或按 Esc 可中途取消并跳过 LLM;松手后转圈→绿✓→淡出,失败红✕→淡出。

**Architecture:** 核心 `voice_input` crate 加 `EngineObserver` trait + `ControlHandle`(CLI 用空实现,行为不变);Tauri 侧实现观察者,驱动一个独立的无边框/透明/置顶/**不抢焦点**(`WS_EX_NOACTIVATE`)浮窗,并提供 `cancel_recording` 命令。Esc 取消走键盘钩子状态机(吞键 + 伪装释放防开始菜单)。

**Tech Stack:** Rust(workspace:核心 crate + src-tauri)、Tauri 2、React + TypeScript + Vite(`src-ui/overlay.tsx`)、windows crate 0.58。

**设计文档:** `docs/superpowers/specs/2026-06-10-g2-recording-overlay-design.md`

---

## 构建环境前置(每个 cargo 命令前)

本机 `cargo` 不在新 shell 的 PATH 上,且 sherpa-rs 经 bindgen 需 libclang。用 **PowerShell**,每条 cargo 命令前先设置:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

前端命令(`npm ...`)在仓库根直接跑,Node 已在 PATH。所有路径以仓库根
`C:\Users\jklov\OneDrive\Documents\Claude code Project\voice-input` 为基准。分支为 `g2-recording-overlay`(已建,设计文档已提交)。

---

## Task 1: 状态机加 Esc 取消(TDD)

**Files:**
- Modify: `src/hotkey/state.rs`(`Event` 枚举、`HotkeyState` 字段、`handle` 的 4 个分支、加测试)

- [ ] **Step 1: 写失败测试**

在 `src/hotkey/state.rs` 的 `mod tests` 内、`repeated_primary_down_is_noop` 测试之后(`}` 之前)追加:

```rust
    #[test]
    fn esc_cancels_and_suppresses_during_recording() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        // 录音中按 Esc:取消,且吞掉 Esc(不透传给前台程序)
        assert_eq!(
            s.handle(Event::EscDown),
            Decision { action: Action::CancelRecording, suppress: true }
        );
        // 取消后松开主键:全程无透传键 → 走伪装释放(suppress:true)、不转录
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::None, suppress: true }
        );
    }

    #[test]
    fn esc_passes_through_when_idle() {
        let mut s = HotkeyState::default();
        // 没在录音时按 Esc:原样透传,不取消
        assert_eq!(
            s.handle(Event::EscDown),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn esc_after_modifier_still_disguises_release() {
        // Win+Alt 录音中按 Esc 取消:Alt 与 Esc 均被吞,松手仍需伪装释放防开始菜单
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        s.handle(Event::ModADown);
        assert_eq!(
            s.handle(Event::EscDown),
            Decision { action: Action::CancelRecording, suppress: true }
        );
        s.handle(Event::ModAUp);
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::None, suppress: true }
        );
    }
```

- [ ] **Step 2: 运行测试确认失败(编译错误:`EscDown` 不存在)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input hotkey::state 2>&1 | Select-Object -Last 15
```
Expected: 编译失败,`no variant named EscDown` / `no field passthrough_seen`。

- [ ] **Step 3: 实现**

3a. 在 `Event` 枚举里(`OtherDown,` 之后)加一行:

```rust
    /// 其他任意键按下
    OtherDown,
    /// Esc 按下(录音中作取消手势)
    EscDown,
```

3b. 给 `HotkeyState` 加字段(在 `mod_b_swallow: bool,` 之后):

```rust
    mod_a_swallow: bool,
    mod_b_swallow: bool,
    /// 本次按住期间是否有"透传给系统"的其他键(用于决定松手是否需伪装释放)
    passthrough_seen: bool,
}
```

3c. 把 `Event::PrimaryDown` 分支里新建会话的部分加上重置(在 `self.combo = false;` 之后):

```rust
                    self.pressed = true;
                    self.combo = false;
                    self.passthrough_seen = false;
                    self.mod_a_seen = self.mod_a_down;
                    self.mod_b_seen = self.mod_b_down;
                    Decision { action: Action::StartRecording, suppress: false }
```

3d. 把 `Event::OtherDown` 分支整体替换为(透传键置 `passthrough_seen`):

```rust
            Event::OtherDown => {
                if self.pressed && !self.combo {
                    self.combo = true;
                    self.passthrough_seen = true;
                    Decision { action: Action::CancelRecording, suppress: false }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::EscDown => {
                if self.pressed && !self.combo {
                    // 录音中:取消并吞掉 Esc(不打扰前台程序)
                    self.combo = true;
                    Decision { action: Action::CancelRecording, suppress: true }
                } else {
                    // 空闲:原样透传
                    Decision { action: Action::None, suppress: false }
                }
            }
```

3e. 把 `Event::PrimaryUp` 分支整体替换为(combo 收尾按是否有透传键决定伪装释放;并重置 `passthrough_seen`):

```rust
            Event::PrimaryUp { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                let passthrough = self.passthrough_seen;
                let a = self.mod_a_seen;
                let b = self.mod_b_seen;
                self.pressed = false;
                self.combo = false;
                self.passthrough_seen = false;
                self.mod_a_seen = false;
                self.mod_b_seen = false;
                if was_combo {
                    // 全程仅被吞键(如 Esc 取消)时,系统没见到任何中间键 →
                    // 需伪装释放防开始菜单;有透传键(OtherDown)则系统已见,直接放行。
                    Decision { action: Action::None, suppress: !passthrough }
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
```

- [ ] **Step 4: 运行测试确认通过**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input hotkey::state 2>&1 | Select-Object -Last 20
```
Expected: 全绿,包含新 3 个测试与原有 `other_key_cancels_as_combo`(后者仍 `suppress:false`)。

- [ ] **Step 5: 提交**

```powershell
git add src/hotkey/state.rs; git commit -m @'
feat(g2): 状态机加 Esc 取消(吞键+按需伪装释放)

录音中 EscDown → CancelRecording 且 suppress(不透传前台);
新增 passthrough_seen,使全程仅被吞键时松手走伪装释放防开始菜单;
空闲时 Esc 原样透传。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 2: 键盘钩子把 Esc 映射为 EscDown

**Files:**
- Modify: `src/hotkey/mod.rs`(导入 `VK_ESCAPE`;事件映射加 Esc 分支)

- [ ] **Step 1: 加导入**

把 `src/hotkey/mod.rs` 顶部的这段导入:

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL,
};
```

改为(追加 `VK_ESCAPE`):

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE,
};
```

- [ ] **Step 2: 事件映射加 Esc 分支**

把 `hook_proc` 里这段:

```rust
        } else if vk == ctx.mod_b_vk {
            Some(if is_down { Event::ModBDown } else { Event::ModBUp })
        } else if is_down {
            Some(Event::OtherDown)
        } else {
            None
        };
```

改为(在 OtherDown 兜底之前插入 Esc):

```rust
        } else if vk == ctx.mod_b_vk {
            Some(if is_down { Event::ModBDown } else { Event::ModBUp })
        } else if vk == VK_ESCAPE.0 && is_down {
            Some(Event::EscDown)
        } else if is_down {
            Some(Event::OtherDown)
        } else {
            None
        };
```

- [ ] **Step 3: 编译验证(钩子无单测,确保构建通过且全测试仍绿)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 8
```
Expected: 编译通过,`test result: ok`(全部单测通过)。

- [ ] **Step 4: 提交**

```powershell
git add src/hotkey/mod.rs; git commit -m @'
feat(g2): 键盘钩子把 Esc 映射为 EscDown

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 3: 配置加 `[overlay] enabled`(TDD)

**Files:**
- Modify: `src/config.rs`(新增 `OverlayConfig`;`Config` 加 `overlay` 字段;加测试)

- [ ] **Step 1: 写失败测试**

在 `src/config.rs` 的 `mod tests` 内,`empty_toml_is_all_defaults` 测试之后追加:

```rust
    #[test]
    fn overlay_defaults_enabled_true() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.overlay.enabled);
    }

    #[test]
    fn overlay_can_be_disabled() {
        let cfg: Config = toml::from_str("[overlay]\nenabled = false\n").unwrap();
        assert!(!cfg.overlay.enabled);
    }
```

- [ ] **Step 2: 运行确认失败**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input config:: 2>&1 | Select-Object -Last 15
```
Expected: 编译失败,`no field overlay`。

- [ ] **Step 3: 实现**

3a. 在 `Config` 结构体里(`app_style` 字段之后、结构体 `}` 之前)加:

```rust
    #[serde(default)]
    pub app_style: Vec<AppStyle>,
    pub overlay: OverlayConfig,
}
```

3b. 在 `Config` 的 `Default` 实现里(`app_style: Vec::new(),` 之后)加:

```rust
            app_style: Vec::new(),
            overlay: OverlayConfig::default(),
        }
    }
}
```

3c. 在 `InjectConfig` 定义之后(其 `Default` 实现之前或之后皆可,放在 `impl Default for InjectConfig` 之后)新增类型与默认:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    /// 是否显示录音浮窗。false 则完全不弹(引擎逻辑不受影响)。
    pub enabled: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig { enabled: true }
    }
}
```

- [ ] **Step 4: 运行确认通过**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input config:: 2>&1 | Select-Object -Last 20
```
Expected: 全绿(含两个新测试)。

- [ ] **Step 5: 更新配置模板并提交**

把 `config.example.toml` 末尾追加(若文件无 trailing 空行则先换行):

```toml

[overlay]
# 是否显示录音浮窗(底部正中的波形药丸)。false 则不弹。
enabled = true
```

```powershell
git add src/config.rs config.example.toml; git commit -m @'
feat(g2): 配置加 [overlay] enabled(默认 true)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 4: 引擎加 Observer/ControlHandle/OverlayState + bottom_center(TDD)

**Files:**
- Modify: `src/engine.rs`(整文件改写:加类型、`bottom_center`、`run`/`run_with`、各状态回调、`handle_command` 返回 Result、加测试)
- 不改 `src/main.rs`(继续调用 `run(config)` 包装器)

> 设计说明:保留 `pub fn run(config)` 作为薄包装(传 `NoopObserver`),新增 `pub fn run_with(config, observer)` 为真实入口。这样 `src/main.rs` 与现 `src-tauri` 调用都不破坏,workspace 始终可编译;Task 6 再把 src-tauri 切到 `run_with`。

- [ ] **Step 1: 用下述完整内容覆盖 `src/engine.rs`**

```rust
//! 听写引擎主循环:热键 → 录音 → ASR →(LLM 整理/翻译/命令)→ 注入。
//! 命令行 bin 与 Tauri 应用共用此入口。

use std::sync::Arc;
use std::{thread, time::Duration};

use arboard::Clipboard;
use crossbeam_channel::{unbounded, Sender};

use crate::asr::Transcriber;
use crate::audio::Recorder;
use crate::config::Config;
use crate::corrector::Corrector;
use crate::foreground::foreground_process_name;
use crate::hotkey::{self, HotkeyAction};
use crate::inject::inject_text;
use crate::keys::vk_from_name;

/// 浮窗要展示的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Recording,
    Processing,
    Done,
    Cancelled,
    Failed,
}

/// 引擎状态观察者(默认空实现,CLI 用)。
pub trait EngineObserver: Send + Sync {
    /// 引擎就绪后回调一次,交回可注入取消的句柄。
    fn on_ready(&self, _control: ControlHandle) {}
    /// 录音/处理状态变化。
    fn on_state(&self, _state: OverlayState) {}
}

/// 什么都不做的观察者(命令行版用,行为与旧版一致)。
pub struct NoopObserver;
impl EngineObserver for NoopObserver {}

/// 交给 GUI 的取消句柄:内含动作发送端克隆,可从外部注入"取消录音"。
pub struct ControlHandle {
    tx: Sender<HotkeyAction>,
}

impl ControlHandle {
    /// 请求取消当前录音(等价于按 Esc / 点药丸)。
    pub fn cancel(&self) {
        let _ = self.tx.send(HotkeyAction::CancelRecording);
    }
}

/// 计算浮窗在某显示器底部正中的左上角坐标(纯函数,便于测试)。
pub fn bottom_center(
    area_w: i32,
    area_h: i32,
    win_w: i32,
    win_h: i32,
    bottom_margin: i32,
) -> (i32, i32) {
    ((area_w - win_w) / 2, area_h - win_h - bottom_margin)
}

/// 阻塞运行听写引擎(命令行用:无浮窗)。
pub fn run(config: Config) -> anyhow::Result<()> {
    run_with(config, Arc::new(NoopObserver))
}

/// 阻塞运行听写引擎,并把状态变化回调给观察者(GUI 用)。
pub fn run_with(config: Config, observer: Arc<dyn EngineObserver>) -> anyhow::Result<()> {
    let primary = vk_from_name(&config.hotkey.primary)?;
    let mod_a = vk_from_name(&config.hotkey.translate_modifier)?;
    let mod_b = vk_from_name(&config.hotkey.command_modifier)?;

    let mut transcriber = Transcriber::load(&config.asr.model_dir, &config.asr.language)?;
    let corrector = Corrector::new(config.llm.clone())?;
    println!(
        "ByType 引擎就绪。{}=识别整理;{}+{}=翻译英文;{}+{}=对选中文字执行语音命令。",
        config.hotkey.primary,
        config.hotkey.primary,
        config.hotkey.translate_modifier,
        config.hotkey.primary,
        config.hotkey.command_modifier,
    );

    let (tx, rx) = unbounded::<HotkeyAction>();
    observer.on_ready(ControlHandle { tx: tx.clone() });
    thread::spawn(move || {
        if let Err(e) = hotkey::run(tx, primary, mod_a, mod_b) {
            eprintln!("钩子线程退出: {e}");
        }
    });

    let mut recorder: Option<Recorder> = None;
    for action in rx.iter() {
        match action {
            HotkeyAction::StartRecording => match Recorder::start() {
                Ok(r) => {
                    recorder = Some(r);
                    observer.on_state(OverlayState::Recording);
                }
                Err(e) => {
                    eprintln!("录音启动失败: {e}");
                    observer.on_state(OverlayState::Failed);
                }
            },
            HotkeyAction::CancelRecording | HotkeyAction::DiscardRecording => {
                recorder = None;
                observer.on_state(OverlayState::Cancelled);
            }
            action @ (HotkeyAction::StopAndTranscribe
            | HotkeyAction::StopAndTranslate
            | HotkeyAction::StopAndCommand) => {
                let Some(r) = recorder.take() else { continue };
                observer.on_state(OverlayState::Processing);
                let (samples, rate) = r.stop();
                let raw = match transcriber.transcribe(&samples, rate) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("识别失败: {e}");
                        observer.on_state(OverlayState::Failed);
                        continue;
                    }
                };
                println!("识别: {raw}");
                let style = foreground_process_name().and_then(|p| config.style_for(&p));
                let outcome: anyhow::Result<()> = match action {
                    HotkeyAction::StopAndCommand => handle_command(&corrector, &raw),
                    HotkeyAction::StopAndTranslate => {
                        let text = corrector.translate(&raw, style.as_deref());
                        if text != raw {
                            println!("翻译: {text}");
                        }
                        inject_text(&text)
                    }
                    _ => {
                        let text = corrector.correct(&raw, style.as_deref());
                        if text != raw {
                            println!("修整: {text}");
                        }
                        inject_text(&text)
                    }
                };
                match outcome {
                    Ok(()) => observer.on_state(OverlayState::Done),
                    Err(e) => {
                        eprintln!("注入失败: {e}");
                        observer.on_state(OverlayState::Failed);
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_command(corrector: &Corrector, instruction: &str) -> anyhow::Result<()> {
    let selected = match copy_selection() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取选中文本失败: {e}");
            String::new()
        }
    };
    if selected.trim().is_empty() {
        let text = corrector.correct(instruction, None);
        inject_text(&text)?;
        return Ok(());
    }
    let result = corrector.command(instruction, &selected);
    println!("命令: {instruction}\n结果: {result}");
    inject_text(&result)?;
    Ok(())
}

fn copy_selection() -> anyhow::Result<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL,
    };
    fn k(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: crate::INJECTED_TAG,
                },
            },
        }
    }
    let mut clipboard = Clipboard::new()?;
    let previous = clipboard.get_text().ok();
    let _ = clipboard.set_text(String::new());
    let inputs = [
        k(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        k(VK_C.0, KEYBD_EVENT_FLAGS(0)),
        k(VK_C.0, KEYEVENTF_KEYUP),
        k(VK_CONTROL.0, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    thread::sleep(Duration::from_millis(120));
    let selected = clipboard.get_text().unwrap_or_default();
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_center_centers_horizontally_and_offsets_bottom() {
        // 1920x1080 屏,240x64 窗,底部留 80
        let (x, y) = bottom_center(1920, 1080, 240, 64, 80);
        assert_eq!(x, (1920 - 240) / 2); // 840
        assert_eq!(y, 1080 - 64 - 80); // 936
    }

    #[test]
    fn bottom_center_handles_window_wider_than_area() {
        // 窗比屏宽时 x 可能为负(调用方可自行 clamp;此处只验证公式)
        let (x, _y) = bottom_center(100, 100, 240, 64, 10);
        assert_eq!(x, (100 - 240) / 2);
    }
}
```

- [ ] **Step 2: 运行测试确认通过(含 bottom_center 与全部既有单测)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 12
```
Expected: `test result: ok`,新增 `engine::tests::bottom_center_*` 通过。

- [ ] **Step 3: 确认 workspace 仍可编译(src-tauri 还在用 `run` 包装器)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build 2>&1 | Select-Object -Last 6
```
Expected: `Finished`(整 workspace 编译通过)。

- [ ] **Step 4: 提交**

```powershell
git add src/engine.rs; git commit -m @'
feat(g2): 引擎加 EngineObserver/ControlHandle/OverlayState + bottom_center

run_with(config, observer) 为真实入口并在 5 个状态点回调;
run(config) 退化为传 NoopObserver 的薄包装(CLI 行为不变);
handle_command 改为返回 Result 以区分成功/失败回调。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 5: 浮窗前端 + 窗口/能力脚手架

**Files:**
- Modify: `src-tauri/tauri.conf.json`(主窗口加 label,新增 overlay 窗口)
- Modify: `vite.config.ts`(多页入口)
- Modify: `package.json`(加 `@tauri-apps/api`)
- Create: `overlay.html`(仓库根)
- Create: `src-ui/overlay.tsx`
- Create: `src-tauri/capabilities/default.json`

- [ ] **Step 1: `tauri.conf.json` 的 `app.windows` 数组替换为两窗口**

把现有:

```json
  "app": {
    "windows": [
      {
        "title": "ByType",
        "width": 900,
        "height": 640,
        "visible": false
      }
    ]
  },
```

替换为:

```json
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "ByType",
        "width": 900,
        "height": 640,
        "visible": false
      },
      {
        "label": "overlay",
        "url": "overlay.html",
        "width": 240,
        "height": 64,
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "focus": false,
        "shadow": false
      }
    ]
  },
```

- [ ] **Step 2: `vite.config.ts` 改为多页入口**

整文件替换为:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  root: ".",
  build: {
    outDir: "dist",
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
      },
    },
  },
  server: { port: 1420, strictPort: true },
  clearScreen: false,
});
```

- [ ] **Step 3: 新建 `overlay.html`(仓库根)**

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <title>ByType Overlay</title>
    <style>
      html, body { margin: 0; height: 100%; background: transparent; overflow: hidden; }
      #overlay-root { height: 100vh; display: flex; align-items: center; justify-content: center; }
      .pill {
        display: inline-flex; align-items: center; gap: 9px;
        padding: 7px 14px; border-radius: 999px;
        background: rgba(16, 16, 22, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.10);
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
        -webkit-backdrop-filter: blur(8px); backdrop-filter: blur(8px);
        font-family: ui-sans-serif, system-ui, sans-serif;
        cursor: pointer; user-select: none;
        opacity: 0; transform: translateY(12px);
        transition: opacity .16s ease, transform .16s ease;
      }
      .pill.show { opacity: 1; transform: translateY(0); }
      .left { display: inline-flex; align-items: center; justify-content: center; min-width: 20px; }
      .time { color: #dbe1ea; font-size: 13px; font-variant-numeric: tabular-nums; }
      .check { color: #46d98a; font-size: 15px; font-weight: 700; }
      .cross { color: #ff6b6b; font-size: 15px; font-weight: 700; }
      .spin {
        width: 14px; height: 14px; border-radius: 50%;
        border: 2px solid rgba(255, 255, 255, 0.18); border-top-color: #7dd3fc;
        animation: spin .7s linear infinite;
      }
      @keyframes spin { to { transform: rotate(360deg); } }
      .wave { display: inline-flex; align-items: center; gap: 3px; height: 22px; }
      .wave i {
        display: block; width: 3px; height: 100%;
        background: linear-gradient(180deg, #7dd3fc, #38bdf8);
        border-radius: 2px; transform-origin: center;
        animation: bar .9s ease-in-out infinite;
      }
      .wave.static i { animation: none; opacity: .35; transform: scaleY(.4); }
      @keyframes bar { 0%, 100% { transform: scaleY(.22); } 50% { transform: scaleY(1); } }
    </style>
  </head>
  <body>
    <div id="overlay-root"></div>
    <script type="module" src="/src-ui/overlay.tsx"></script>
  </body>
</html>
```

- [ ] **Step 4: 新建 `src-ui/overlay.tsx`(完整行为)**

```tsx
import ReactDOM from "react-dom/client";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

type Phase = "idle" | "recording" | "processing" | "done" | "cancelled" | "failed";

function Pill() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [secs, setSecs] = useState(0);
  const [leaving, setLeaving] = useState(false);
  const tick = useRef<number | null>(null);
  const hideT = useRef<number | null>(null);

  const stopTick = () => {
    if (tick.current !== null) {
      clearInterval(tick.current);
      tick.current = null;
    }
  };
  const clearHide = () => {
    if (hideT.current !== null) {
      clearTimeout(hideT.current);
      hideT.current = null;
    }
  };

  // 先停留 holdMs 展示图标,再淡出,最后隐藏窗口
  const finishWithFade = (holdMs: number) => {
    clearHide();
    hideT.current = window.setTimeout(() => {
      setLeaving(true);
      window.setTimeout(() => {
        getCurrentWindow().hide();
        setPhase("idle");
        setLeaving(false);
        setSecs(0);
      }, 200);
    }, holdMs);
  };

  useEffect(() => {
    const un = listen<string>("bt:state", (e) => {
      const s = e.payload as Phase;
      if (s === "recording") {
        stopTick();
        clearHide();
        setLeaving(false);
        setSecs(0);
        const start = Date.now();
        tick.current = window.setInterval(
          () => setSecs(Math.floor((Date.now() - start) / 1000)),
          250
        );
        setPhase("recording");
      } else if (s === "processing") {
        stopTick();
        setLeaving(false);
        setPhase("processing");
      } else if (s === "done") {
        stopTick();
        setPhase("done");
        finishWithFade(500);
      } else if (s === "failed") {
        stopTick();
        setPhase("failed");
        finishWithFade(700);
      } else if (s === "cancelled") {
        stopTick();
        setPhase("cancelled");
        finishWithFade(0);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const onClick = () => {
    if (phase !== "recording") return;
    stopTick();
    setPhase("cancelled");
    invoke("cancel_recording").catch(() => {});
    finishWithFade(0);
  };

  const show = phase !== "idle" && !leaving;

  return (
    <div className={`pill ${show ? "show" : ""}`} onClick={onClick} title="点击取消">
      <span className="left">
        {phase === "processing" && <span className="spin" />}
        {phase === "done" && <span className="check">✓</span>}
        {phase === "failed" && <span className="cross">✕</span>}
        {(phase === "recording" || phase === "cancelled") && (
          <span className="time">{secs}</span>
        )}
      </span>
      <span className={`wave ${phase === "recording" ? "" : "static"}`}>
        {Array.from({ length: 8 }).map((_, i) => (
          <i key={i} style={{ animationDelay: `${-i * 0.1}s` }} />
        ))}
      </span>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("overlay-root")!).render(<Pill />);
```

- [ ] **Step 5: 新建 `src-tauri/capabilities/default.json`(放开浮窗的事件监听与隐藏)**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "ByType 主窗口与浮窗能力",
  "windows": ["main", "overlay"],
  "permissions": [
    "core:default",
    "core:window:allow-hide"
  ]
}
```

- [ ] **Step 6: 安装前端依赖**

```powershell
npm install @tauri-apps/api@^2
```
Expected: `package.json` 的 dependencies 出现 `@tauri-apps/api`,`package-lock.json` 更新,无报错。

- [ ] **Step 7: 构建前端验证两页都打包**

```powershell
npm run build 2>&1 | Select-Object -Last 15
```
Expected: 成功,`dist/` 下同时产出 `index.html` 与 `overlay.html`(以及各自的 JS 资源),无 TS/打包错误。

- [ ] **Step 8: 提交**

```powershell
git add src-tauri/tauri.conf.json vite.config.ts package.json package-lock.json overlay.html src-ui/overlay.tsx src-tauri/capabilities/default.json; git commit -m @'
feat(g2): 浮窗前端(波形药丸)+ 窗口/能力脚手架

新增 overlay 窗口(透明/无边框/置顶/不抢焦点)、overlay.html、
src-ui/overlay.tsx(listen bt:state、秒计时、转圈/✓/✕、点击取消);
vite 多页入口;@tauri-apps/api;capabilities 放开 event 监听与 window hide。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 6: Tauri 后端接线(观察者 + 取消命令 + 不抢焦点)

**Files:**
- Modify: `src-tauri/Cargo.toml`(加 `windows` 依赖)
- Modify: `src-tauri/src/lib.rs`(整文件改写:ControlSlot、TauriObserver、cancel_recording、apply_no_activate、setup 读配置并跑 `run_with`)

- [ ] **Step 1: `src-tauri/Cargo.toml` 加 windows 依赖**

在 `[dependencies]` 段(`anyhow = "1"` 之后)与 `[[bin]]` 之间插入:

```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.58"
features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging"]
```

- [ ] **Step 2: 用下述完整内容覆盖 `src-tauri/src/lib.rs`**

```rust
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent,
};

use voice_input::config::Config;
use voice_input::engine::{bottom_center, ControlHandle, EngineObserver, OverlayState};

/// 浮窗逻辑像素尺寸(须与 tauri.conf.json 的 overlay 窗口一致)。
const OVERLAY_W: i32 = 240;
const OVERLAY_H: i32 = 64;
/// 距屏幕底部的逻辑像素留白(避开任务栏)。
const OVERLAY_BOTTOM_MARGIN: f64 = 80.0;

/// 存放引擎交回的取消句柄,供 cancel_recording 命令使用。
#[derive(Default)]
struct ControlSlot(Mutex<Option<ControlHandle>>);

/// 前端点药丸时调用:请求取消当前录音(跳过 LLM)。
#[tauri::command]
fn cancel_recording(slot: State<ControlSlot>) {
    if let Some(c) = slot.0.lock().unwrap().as_ref() {
        c.cancel();
    }
}

/// 驱动浮窗的观察者:状态变化时定位/显示浮窗并向前端 emit。
struct TauriObserver {
    app: tauri::AppHandle,
    enabled: bool,
}

impl EngineObserver for TauriObserver {
    fn on_ready(&self, control: ControlHandle) {
        self.app.state::<ControlSlot>().0.lock().unwrap().replace(control);
    }

    fn on_state(&self, state: OverlayState) {
        if !self.enabled {
            return;
        }
        let tag = match state {
            OverlayState::Recording => "recording",
            OverlayState::Processing => "processing",
            OverlayState::Done => "done",
            OverlayState::Cancelled => "cancelled",
            OverlayState::Failed => "failed",
        };
        if matches!(state, OverlayState::Recording) {
            if let Some(w) = self.app.get_webview_window("overlay") {
                position_bottom_center(&w);
                let _ = w.show();
            }
        }
        let _ = self.app.emit_to("overlay", "bt:state", tag);
    }
}

/// 把浮窗摆到主屏底部正中(物理像素)。
fn position_bottom_center(w: &WebviewWindow) {
    if let Ok(Some(m)) = w.primary_monitor() {
        let scale = m.scale_factor();
        let ms = m.size();
        let win_w = (OVERLAY_W as f64 * scale) as i32;
        let win_h = (OVERLAY_H as f64 * scale) as i32;
        let margin = (OVERLAY_BOTTOM_MARGIN * scale) as i32;
        let (x, y) = bottom_center(ms.width as i32, ms.height as i32, win_w, win_h, margin);
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

/// 给浮窗 HWND 加 WS_EX_NOACTIVATE(点击/显示都不抢焦点)+ WS_EX_TOOLWINDOW(隐藏出 Alt-Tab)。
#[cfg(windows)]
fn apply_no_activate(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    if let Ok(h) = w.hwnd() {
        let hwnd = HWND(h.0);
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
            );
        }
    }
}

#[cfg(not(windows))]
fn apply_no_activate(_w: &WebviewWindow) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(ControlSlot::default())
        .invoke_handler(tauri::generate_handler![cancel_recording])
        .setup(|app| {
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &quit])?;
            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("ByType")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // 浮窗:加不抢焦点扩展样式
            if let Some(w) = app.get_webview_window("overlay") {
                apply_no_activate(&w);
            }

            // 读配置并在后台线程跑引擎(把状态回调到浮窗)
            let app_handle = app.handle().clone();
            match Config::load_resolved() {
                Ok(cfg) => {
                    let enabled = cfg.overlay.enabled;
                    let observer = Arc::new(TauriObserver { app: app_handle, enabled });
                    std::thread::spawn(move || {
                        if let Err(e) = voice_input::engine::run_with(cfg, observer) {
                            eprintln!("引擎退出: {e}");
                        }
                    });
                }
                Err(e) => eprintln!("加载配置失败: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ByType");
}
```

- [ ] **Step 3: 编译整 workspace**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build 2>&1 | Select-Object -Last 12
```
Expected: `Finished`。若 `w.hwnd()` 的 HWND 字段类型报错(tauri 的 windows 版本与 0.58 不一致),把 `HWND(h.0)` 改为 `HWND(h.0 as _)` 再编译。

- [ ] **Step 4: 跑核心单测确保未回归**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 8
```
Expected: `test result: ok`。

- [ ] **Step 5: 提交**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/lib.rs Cargo.lock; git commit -m @'
feat(g2): Tauri 接线浮窗——观察者驱动显示/emit + 取消命令 + 不抢焦点

setup 读配置在后台跑 engine::run_with(TauriObserver);
TauriObserver 在录音时定位主屏底部正中并 show(不 set_focus),各状态 emit bt:state;
cancel_recording 命令经 ControlHandle 注入 CancelRecording;
overlay HWND 加 WS_EX_NOACTIVATE + WS_EX_TOOLWINDOW。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

---

## Task 7: 真机端到端验证 + 全量测试

> 本任务是手动验证(OS/麦克风/LLM 集成,沿用项目惯例)。用我(助手)起 `tauri dev`,你(用户)实测。

- [ ] **Step 1: 全量单测 + 构建**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test -p voice-input 2>&1 | Select-Object -Last 8
```
Expected: 全绿。

- [ ] **Step 2: 启动应用(后台运行,日志可查)**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; npm run tauri dev
```
(以 run_in_background 方式起;读任务输出确认出现 `ByType 引擎就绪`。)

- [ ] **Step 3: 逐项实测(对照设计文档测试清单)**

逐一确认:
  1. 按住 Win 说话 → 药丸在屏幕底部正中淡入、秒数 0/1/2… 递增、波形跳动;**焦点不被抢**(目标程序仍有光标),松手后文字正常粘贴。
  2. 录制中**鼠标点药丸** → 立即淡出、不出字(未调用 LLM)。
  3. 录制中**按 Esc** → 同样取消、不出字,且**不弹开始菜单**、Esc 未触发前台程序行为。
  4. 正常松手 → 转圈 → 绿 ✓ → 淡出,文字注入。
  5. **翻译(Win+Alt)**、**命令(Win+Ctrl)** 也弹同一药丸;命令模式选中文字后能替换。
  6. 失败路径(可临时把 `config.toml` 的 `llm.base_url` 改错触发注入前失败,或断网):红 ✕ → 淡出(测完改回)。
  7. 把 `config.toml` 设 `[overlay] enabled=false` 重启 → 不弹浮窗,听写照常(测完按需改回)。

- [ ] **Step 4: 关闭后台 dev 进程**

- [ ] **Step 5: 若验证中有改动,补测并提交;无改动则本任务无提交**

```powershell
git add -A; git commit -m @'
fix(g2): 端到端验证发现的修正

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
'@
```

(若无改动跳过提交。)随后按 `superpowers:finishing-a-development-branch` 收尾合并 master。

---

## 实现顺序与依赖

1→2(状态机/钩子,Esc)、3(配置)、4(引擎 API)互相独立,可任意先后;**5 依赖 4**(前端 emit 约定);**6 依赖 4、5**(后端接线用 `run_with` 与 overlay 窗口/能力);**7 依赖全部**。建议按 1→2→3→4→5→6→7 顺序。
