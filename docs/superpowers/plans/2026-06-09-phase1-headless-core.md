# 语音输入工具 — 阶段一(无头核心)实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 打通"按住 Win 说话 → 松手 → 文字粘贴进当前应用"这条无头链路(不含 LLM、不含 UI)。

**Architecture:** 单个 Rust 二进制。纯逻辑(热键状态机、注入文本清理)用 TDD 单测覆盖;
与硬件/模型相关的模块(键盘钩子、cpal 录音、sherpa ASR、剪贴板注入)做最小封装,
靠编译 + 手动端到端验证。`main.rs` 用 channel 把各模块串起来并跑 Win32 消息循环。

**Tech Stack:** Rust、`windows`(WH_KEYBOARD_LL + SendInput)、`cpal`(录音)、
`sherpa-rs`(SenseVoice ONNX 离线识别)、`arboard`(剪贴板)。

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `Cargo.toml` | 依赖与二进制定义 |
| `src/main.rs` | 入口:装钩子、起消息循环、用 channel 串联录音→ASR→注入 |
| `src/hotkey/mod.rs` | 安装 WH_KEYBOARD_LL 钩子,把原始按键翻译成 `Event`,执行 suppress,转发 `Action` |
| `src/hotkey/state.rs` | **纯状态机**:`(Event, 状态) → Decision{action, suppress}`(可单测) |
| `src/audio.rs` | cpal 麦克风录音,缓冲样本,返回 `(Vec<f32>, sample_rate)` 单声道 |
| `src/asr.rs` | `sherpa-rs` 离线识别封装(SenseVoice),`transcribe(samples, rate) -> String` |
| `src/inject.rs` | 剪贴板备份/写入/还原 + 发 Ctrl+V;含纯函数 `prepare_text`(可单测) |
| `src/lib.rs` | 导出模块,供测试引用 |

阶段一不做配置文件:模型目录用常量 `const MODEL_DIR`,阶段二再抽到 TOML。

---

## Task 1: 项目脚手架

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "voice-input"
version = "0.1.0"
edition = "2021"

[lib]
name = "voice_input"
path = "src/lib.rs"

[[bin]]
name = "voice-input"
path = "src/main.rs"

[dependencies]
cpal = "0.15"
arboard = "3"
sherpa-rs = "0.6"
crossbeam-channel = "0.5"

[dependencies.windows]
version = "0.58"
features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_System_LibraryLoader",
]
```

> 注:`sherpa-rs` 的确切版本与 SenseVoice API 在 Task 5 第 1 步先核实,必要时调整版本号。

- [ ] **Step 2: 创建占位 src/lib.rs**

```rust
pub mod hotkey;
pub mod audio;
pub mod asr;
pub mod inject;
```

- [ ] **Step 3: 创建占位 src/main.rs**

```rust
fn main() {
    println!("voice-input skeleton");
}
```

- [ ] **Step 4: 建子模块占位文件,确保能编译**

创建空文件 `src/hotkey/mod.rs`、`src/audio.rs`、`src/asr.rs`、`src/inject.rs`,
并在 `src/hotkey/mod.rs` 顶部加 `pub mod state;`,创建空 `src/hotkey/state.rs`。

- [ ] **Step 5: 编译验证**

Run: `cargo build`
Expected: 编译成功(可能有未使用警告,允许)。

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/
git commit -m "chore: Rust 项目脚手架"
```

---

## Task 2: 热键状态机(纯逻辑,TDD)

**Files:**
- Modify: `src/hotkey/state.rs`
- Test: `src/hotkey/state.rs`(`#[cfg(test)]` 内联模块)

- [ ] **Step 1: 写失败的测试**

把以下内容写入 `src/hotkey/state.rs`(测试在前,类型先空缺以制造失败):

```rust
/// 进入状态机的归一化事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 配置的热键(Win)按下
    Down,
    /// 按住热键期间,任意其他键按下
    Other,
    /// 配置的热键释放,附带本次按住时长(毫秒)
    Up { held_ms: u64 },
}

/// 状态机给出的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    DiscardRecording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    /// 是否在钩子层吞掉该事件(return 1)
    pub suppress: bool,
}

/// 低于该时长的"单独 Win"按住视为误触。
pub const MIN_HOLD_MS: u64 = 300;

#[derive(Debug, Default)]
pub struct HotkeyState {
    pressed: bool,
    combo: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_press_then_release_transcribes_and_suppresses_up() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::Down),
            Decision { action: Action::StartRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::Up { held_ms: 1000 }),
            Decision { action: Action::StopAndTranscribe, suppress: true }
        );
    }

    #[test]
    fn too_short_lone_press_discards_but_still_suppresses() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Up { held_ms: 100 }),
            Decision { action: Action::DiscardRecording, suppress: true }
        );
    }

    #[test]
    fn other_key_makes_it_a_combo_and_cancels() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Other),
            Decision { action: Action::CancelRecording, suppress: false }
        );
        // 组合键的 Win 释放必须放行,不能吞
        assert_eq!(
            s.handle(Event::Up { held_ms: 1000 }),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn repeated_down_while_held_is_noop() {
        let mut s = HotkeyState::default();
        s.handle(Event::Down);
        assert_eq!(
            s.handle(Event::Down),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn other_key_when_not_pressed_is_ignored() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::Other),
            Decision { action: Action::None, suppress: false }
        );
    }
}
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cargo test --lib hotkey::state`
Expected: 编译失败 —— `handle` 方法未定义。

- [ ] **Step 3: 实现 handle**

在 `src/hotkey/state.rs` 的 `impl` 中加入(放在 `tests` 模块之前):

```rust
impl HotkeyState {
    pub fn handle(&mut self, event: Event) -> Decision {
        match event {
            Event::Down => {
                if self.pressed {
                    Decision { action: Action::None, suppress: false }
                } else {
                    self.pressed = true;
                    self.combo = false;
                    Decision { action: Action::StartRecording, suppress: false }
                }
            }
            Event::Other => {
                if self.pressed && !self.combo {
                    self.combo = true;
                    Decision { action: Action::CancelRecording, suppress: false }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::Up { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                self.pressed = false;
                self.combo = false;
                if was_combo {
                    Decision { action: Action::None, suppress: false }
                } else if held_ms >= MIN_HOLD_MS {
                    Decision { action: Action::StopAndTranscribe, suppress: true }
                } else {
                    Decision { action: Action::DiscardRecording, suppress: true }
                }
            }
        }
    }
}
```

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test --lib hotkey::state`
Expected: 5 个测试全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/hotkey/state.rs
git commit -m "feat: 热键状态机(单独Win判定/组合键放行/误触丢弃)"
```

---

## Task 3: 注入文本清理(纯逻辑,TDD)

**Files:**
- Modify: `src/inject.rs`
- Test: `src/inject.rs`(`#[cfg(test)]` 内联模块)

- [ ] **Step 1: 写失败的测试**

把以下写入 `src/inject.rs`:

```rust
/// 注入前清理识别/纠错产生的多余包裹与空白。
pub fn prepare_text(raw: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_surrounding_whitespace_and_newlines() {
        assert_eq!(prepare_text("  你好世界 \n"), "你好世界");
    }

    #[test]
    fn strips_wrapping_double_quotes() {
        assert_eq!(prepare_text("\"hello world\""), "hello world");
    }

    #[test]
    fn strips_code_fences() {
        assert_eq!(prepare_text("```\n打开文件\n```"), "打开文件");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(prepare_text("   "), "");
    }

    #[test]
    fn keeps_internal_content_untouched() {
        assert_eq!(prepare_text("今天 weather 不错"), "今天 weather 不错");
    }
}
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cargo test --lib inject`
Expected: panic `not yet implemented`(todo!)。

- [ ] **Step 3: 实现 prepare_text**

替换 `prepare_text` 的函数体:

```rust
pub fn prepare_text(raw: &str) -> String {
    let mut s = raw.trim();
    // 去掉 ``` 代码围栏(可能带语言标注)
    if s.starts_with("```") {
        s = s.trim_start_matches("```");
        if let Some(idx) = s.find('\n') {
            s = &s[idx + 1..];
        }
        s = s.trim_end_matches("```").trim();
    }
    // 去掉成对的首尾英文/中文双引号
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        let is_pair = (first == '"' && last == '"') || (first == '“' && last == '”');
        if is_pair {
            return chars[1..chars.len() - 1].iter().collect::<String>().trim().to_string();
        }
    }
    s.to_string()
}
```

- [ ] **Step 4: 运行测试,确认通过**

Run: `cargo test --lib inject`
Expected: 5 个测试全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/inject.rs
git commit -m "feat: 注入前文本清理(去围栏/去引号/去空白)"
```

---

## Task 4: cpal 录音封装

**Files:**
- Modify: `src/audio.rs`

无法对硬件录音做有意义的单测,因此本任务靠编译 + 手动验证。

- [ ] **Step 1: 实现 Recorder**

把以下写入 `src/audio.rs`:

```rust
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// 一次录音会话:持有 cpal 流,边录边把样本下混成单声道累积。
pub struct Recorder {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

impl Recorder {
    /// 在默认输入设备上开始录音。
    pub fn start() -> anyhow::Result<Recorder> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有可用的麦克风输入设备"))?;
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let buf_cb = buffer.clone();

        let err_fn = |e| eprintln!("录音流错误: {e}");
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut b = buf_cb.lock().unwrap();
                // 下混成单声道:每 channels 个样本取平均
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().sum();
                    b.push(sum / channels as f32);
                }
            },
            err_fn,
            None,
        )?;
        stream.play()?;

        Ok(Recorder { _stream: stream, buffer, sample_rate })
    }

    /// 停止并取出 (单声道样本, 采样率)。sherpa-onnx 会内部重采样到 16k。
    pub fn stop(self) -> (Vec<f32>, u32) {
        drop(self._stream);
        let samples = self.buffer.lock().unwrap().clone();
        (samples, self.sample_rate)
    }
}
```

> 注:此处假设默认输入格式为 `f32`。若 Step 2 手测时报采样格式不符,
> 需按 `config.sample_format()` 分支处理 i16/u16 并转 f32(届时补充)。

- [ ] **Step 2: 加 anyhow 依赖并编译**

在 `Cargo.toml` 的 `[dependencies]` 加 `anyhow = "1"`,然后:

Run: `cargo build`
Expected: 编译成功。

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml src/audio.rs
git commit -m "feat: cpal 录音封装(下混单声道)"
```

---

## Task 5: sherpa-rs / SenseVoice 识别封装

**Files:**
- Modify: `src/asr.rs`

- [ ] **Step 1: 先核实 sherpa-rs 的 SenseVoice API(必做)**

阅读 `sherpa-rs` 当前版本文档与示例,确认:
1. 离线识别器对 **SenseVoice** 模型的构造方式与配置字段;
2. 喂音频的方法签名(样本切片 + 采样率)与取文本的字段;
3. 若该 crate 未直接暴露 SenseVoice,改用底层 `sherpa-rs-sys` 的 offline recognizer 配置。

把确认到的真实 API 记录在本步骤下方注释,再写 Step 2(下方代码为预期形态,
按真实 API 调整字段名)。同时下载 SenseVoice-Small ONNX 模型到
`./models/sensevoice/`(含 `model.onnx` 与 `tokens.txt`)。

- [ ] **Step 2: 实现 Transcriber**

把以下写入 `src/asr.rs`(字段名以 Step 1 核实结果为准):

```rust
use sherpa_rs::transcribe::sense_voice::{SenseVoice, SenseVoiceConfig};

pub const MODEL_DIR: &str = "./models/sensevoice";

/// 持有已加载的 SenseVoice 识别器。
pub struct Transcriber {
    inner: SenseVoice,
}

impl Transcriber {
    pub fn load() -> anyhow::Result<Transcriber> {
        let config = SenseVoiceConfig {
            model: format!("{MODEL_DIR}/model.onnx"),
            tokens: format!("{MODEL_DIR}/tokens.txt"),
            language: "auto".into(),
            use_itn: true,
            ..Default::default()
        };
        let inner = SenseVoice::new(config)?;
        Ok(Transcriber { inner })
    }

    /// 整段识别。sherpa-onnx 内部会重采样到 16k。
    pub fn transcribe(&mut self, samples: &[f32], sample_rate: u32) -> anyhow::Result<String> {
        let text = self.inner.transcribe(sample_rate, samples);
        Ok(text)
    }
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo build`
Expected: 编译成功(API 若不符,据 Step 1 调整后再编译)。

- [ ] **Step 4: Commit**

```bash
git add src/asr.rs
git commit -m "feat: SenseVoice 离线识别封装"
```

---

## Task 6: 剪贴板注入(Ctrl+V)

**Files:**
- Modify: `src/inject.rs`

- [ ] **Step 1: 实现 inject_text**

在 `src/inject.rs` 顶部(`prepare_text` 上方)加入:

```rust
use std::{thread, time::Duration};
use arboard::Clipboard;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_V,
};

/// 备份剪贴板 → 写入文本 → 发 Ctrl+V → 还原剪贴板。
pub fn inject_text(text: &str) -> anyhow::Result<()> {
    let text = prepare_text(text);
    if text.is_empty() {
        return Ok(());
    }
    let mut clipboard = Clipboard::new()?;
    let previous = clipboard.get_text().ok();

    clipboard.set_text(text)?;
    thread::sleep(Duration::from_millis(30));
    send_ctrl_v();
    thread::sleep(Duration::from_millis(80));

    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(())
}

fn key_input(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_ctrl_v() {
    let inputs = [
        key_input(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_V.0, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_V.0, KEYEVENTF_KEYUP),
        key_input(VK_CONTROL.0, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
```

- [ ] **Step 2: 编译并确认已有单测仍通过**

Run: `cargo test --lib inject`
Expected: `prepare_text` 的 5 个测试仍 PASS,编译成功。

- [ ] **Step 3: Commit**

```bash
git add src/inject.rs
git commit -m "feat: 剪贴板注入 + 模拟 Ctrl+V"
```

---

## Task 7: 键盘钩子 + 消息循环

**Files:**
- Modify: `src/hotkey/mod.rs`

钩子涉及全局静态与 unsafe Win32,无法单测,靠手动验证。

- [ ] **Step 1: 实现钩子**

把以下写入 `src/hotkey/mod.rs`:

```rust
pub mod state;

use std::sync::OnceLock;
use std::time::Instant;
use crossbeam_channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LWIN, VK_RWIN};

use state::{Action, Decision, Event, HotkeyState};

/// 钩子触发后向主线程发送的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    DiscardRecording,
}

struct HookCtx {
    state: HotkeyState,
    down_at: Option<Instant>,
    sender: Sender<HotkeyAction>,
}

// 低级钩子回调只能用全局状态。
static CTX: OnceLock<std::sync::Mutex<HookCtx>> = OnceLock::new();

fn is_hotkey(vk: u16) -> bool {
    vk == VK_LWIN.0 // 阶段一固定左 Win;阶段二改为可配置
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode as u16;
    let msg = wparam.0 as u32;

    let event = if is_hotkey(vk) {
        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(EventKind::Down),
            WM_KEYUP | WM_SYSKEYUP => Some(EventKind::Up),
            _ => None,
        }
    } else if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) {
        Some(EventKind::Other)
    } else {
        None
    };

    if let Some(kind) = event {
        if let Some(ctx_lock) = CTX.get() {
            let mut ctx = ctx_lock.lock().unwrap();
            let ev = match kind {
                EventKind::Down => {
                    if ctx.down_at.is_none() {
                        ctx.down_at = Some(Instant::now());
                    }
                    Event::Down
                }
                EventKind::Other => Event::Other,
                EventKind::Up => {
                    let held = ctx
                        .down_at
                        .take()
                        .map(|t| t.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    Event::Up { held_ms: held }
                }
            };
            let Decision { action, suppress } = ctx.state.handle(ev);
            dispatch(&ctx.sender, action);
            if suppress {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

enum EventKind { Down, Other, Up }

fn dispatch(sender: &Sender<HotkeyAction>, action: Action) {
    let mapped = match action {
        Action::StartRecording => Some(HotkeyAction::StartRecording),
        Action::CancelRecording => Some(HotkeyAction::CancelRecording),
        Action::StopAndTranscribe => Some(HotkeyAction::StopAndTranscribe),
        Action::DiscardRecording => Some(HotkeyAction::DiscardRecording),
        Action::None => None,
    };
    if let Some(a) = mapped {
        let _ = sender.send(a);
    }
}

/// 安装钩子并进入消息循环(阻塞当前线程)。
pub fn run(sender: Sender<HotkeyAction>) -> anyhow::Result<()> {
    let _ = CTX.set(std::sync::Mutex::new(HookCtx {
        state: HotkeyState::default(),
        down_at: None,
        sender,
    }));
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)?;
        if hook.is_invalid() {
            anyhow::bail!("安装键盘钩子失败");
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    }
    Ok(())
}
```

> 注:`SetWindowsHookExW` 的 `hmod` 参数对 LL 钩子可传 `None`。若运行期报错,
> 改用 `GetModuleHandleW(None)` 传入模块句柄。

- [ ] **Step 2: 编译验证**

Run: `cargo build`
Expected: 编译成功(状态机单测不受影响)。

- [ ] **Step 3: Commit**

```bash
git add src/hotkey/mod.rs
git commit -m "feat: WH_KEYBOARD_LL 钩子 + 消息循环"
```

---

## Task 8: main.rs 串联 + 端到端手测

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: 实现串联**

替换 `src/main.rs`:

```rust
use crossbeam_channel::unbounded;
use voice_input::asr::Transcriber;
use voice_input::audio::Recorder;
use voice_input::hotkey::{self, HotkeyAction};
use voice_input::inject::inject_text;

fn main() -> anyhow::Result<()> {
    let mut transcriber = Transcriber::load()?;
    println!("SenseVoice 已加载。按住左 Win 说话,松开输出。");

    let (tx, rx) = unbounded::<HotkeyAction>();

    // 钩子需在自己的线程跑消息循环。
    std::thread::spawn(move || {
        if let Err(e) = hotkey::run(tx) {
            eprintln!("钩子线程退出: {e}");
        }
    });

    let mut recorder: Option<Recorder> = None;

    for action in rx.iter() {
        match action {
            HotkeyAction::StartRecording => match Recorder::start() {
                Ok(r) => recorder = Some(r),
                Err(e) => eprintln!("录音启动失败: {e}"),
            },
            HotkeyAction::CancelRecording | HotkeyAction::DiscardRecording => {
                recorder = None; // drop 即停止
            }
            HotkeyAction::StopAndTranscribe => {
                if let Some(r) = recorder.take() {
                    let (samples, rate) = r.stop();
                    match transcriber.transcribe(&samples, rate) {
                        Ok(text) => {
                            println!("识别: {text}");
                            if let Err(e) = inject_text(&text) {
                                eprintln!("注入失败: {e}");
                            }
                        }
                        Err(e) => eprintln!("识别失败: {e}"),
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 编译**

Run: `cargo build --release`
Expected: 编译成功。

- [ ] **Step 3: 手动端到端验证**

前置:`./models/sensevoice/` 下已有模型文件。

Run: `cargo run --release`
操作:打开记事本,按住左 Win,说"今天天气不错 hello world",松开。
Expected:
- 控制台打印识别文本;
- 记事本出现该文本;
- 松开 Win **不弹出开始菜单**;
- 单独快速点一下 Win(<300ms)不输出、也不弹开始菜单;
- 按 Win+E 仍能正常打开资源管理器(组合键放行)。

发现问题按对应模块定位修复(录音格式 → Task 4 注;ASR API → Task 5;
钩子吞键 → Task 7)。

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: 串联热键→录音→ASR→注入,阶段一端到端打通"
```

---

## 阶段一完成标准

- [ ] `cargo test` 全绿(状态机 + 文本清理共 10 个单测)。
- [ ] `cargo run --release` 下,按住左 Win 说话能把中英文混合文本粘贴进任意应用。
- [ ] 单独 Win 不弹开始菜单;Win+组合键不受影响;短按不误触。

阶段二(LLM 纠错 + TOML 配置)、阶段三(Tauri 托盘 + 浮窗)另立计划。
