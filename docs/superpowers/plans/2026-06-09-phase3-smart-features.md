# Phase 3 — 运行时智能功能 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 加入自定义词库、选中文字语音命令编辑(Win+Ctrl)、随应用切换风格、可配置热键,全部由 config.toml 驱动。

**Architecture:** 状态机从"主键+1修饰键"泛化为"主键+2修饰键(A=翻译,B=命令)";钩子改为从配置读 VK;corrector 支持把"词库行+应用风格"拼到系统提示词并新增 command();新增 keys(键名→VK)与 foreground(前台进程名)两个小模块;main 串联命令模式的复制-替换流程与应用风格。

**Tech Stack:** Rust、windows(Threading/前台窗口、键盘)、serde/toml、reqwest。

---

## 运行环境(执行者必读)
- cargo 经 PowerShell 跑;每条命令前置:
  `$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo ...`
- 不要 `cd`;工作目录已是项目根。git 用 bash。
- 不要运行二进制(会抢全局键盘钩子)。涉及 Win32 的模块只编译验证。
- 当前分支 `phase3-smart-features`(控制器已建)。

## 文件结构

| 文件 | 职责 | 改动 |
|------|------|------|
| `src/keys.rs` | 键名→VK 解析(纯函数) | 新建 |
| `src/foreground.rs` | 取前台窗口进程名 | 新建 |
| `src/config.rs` | `[hotkey]` 表、vocabulary、command_prompt、`[[app_style]]`,及 vocabulary_line/style_for/effective_command_prompt | 改 |
| `src/corrector.rs` | compose_system_prompt + correct/translate 加 style 参数 + 新增 command | 改 |
| `src/hotkey/state.rs` | 泛化为主键+2修饰键,新增 StopAndCommand | 改 |
| `src/hotkey/mod.rs` | 从配置读 VK、识别 ModB、伪装释放传入主键 VK | 改 |
| `src/main.rs` | 解析热键、前台风格、命令模式复制-替换流程 | 改 |
| `src/lib.rs` | 导出 keys、foreground | 改 |
| `config.example.toml` / `config.toml` | 新增配置项 | 改 |
| `Cargo.toml` | windows 加 Threading 特性 | 改 |

---

## Task 1: 键名→VK 解析(TDD)

**Files:** Create `src/keys.rs`;Modify `src/lib.rs`

- [ ] **Step 1: lib.rs 导出**

在 `src/lib.rs` 模块声明区加(与其他 `pub mod` 并列):
```rust
pub mod keys;
pub mod foreground;
```
并创建空文件 `src/foreground.rs`(0 字节占位,Task 5 填)以保证编译。

- [ ] **Step 2: 写 keys.rs(含失败测试)**

写入 `src/keys.rs`:
```rust
//! 把配置里的按键名(如 "LWin")解析成 Win32 虚拟键码。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};

/// 解析按键名 → VK 码。大小写不敏感。未知名报错。
pub fn vk_from_name(name: &str) -> anyhow::Result<u16> {
    let vk = match name.trim().to_ascii_uppercase().as_str() {
        "LWIN" => VK_LWIN,
        "RWIN" => VK_RWIN,
        "LALT" => VK_LMENU,
        "RALT" => VK_RMENU,
        "LCTRL" => VK_LCONTROL,
        "RCTRL" => VK_RCONTROL,
        "LSHIFT" => VK_LSHIFT,
        "RSHIFT" => VK_RSHIFT,
        other => anyhow::bail!(
            "未知热键名: {other}(可用:LWin RWin LAlt RAlt LCtrl RCtrl LShift RShift)"
        ),
    };
    Ok(vk.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LCONTROL, VK_LMENU, VK_LWIN};

    #[test]
    fn parses_known_names() {
        assert_eq!(vk_from_name("LWin").unwrap(), VK_LWIN.0);
        assert_eq!(vk_from_name("LAlt").unwrap(), VK_LMENU.0);
        assert_eq!(vk_from_name("LCtrl").unwrap(), VK_LCONTROL.0);
    }

    #[test]
    fn is_case_insensitive_and_trims() {
        assert_eq!(vk_from_name("  lwin ").unwrap(), VK_LWIN.0);
    }

    #[test]
    fn unknown_name_errors() {
        assert!(vk_from_name("Banana").is_err());
    }
}
```

- [ ] **Step 3: 测试**
`...; cargo test --lib keys`
Expected: 3 通过。

- [ ] **Step 4: Commit**
```bash
git add src/lib.rs src/keys.rs src/foreground.rs && git commit -m "feat: 键名→VK 解析模块"
```

---

## Task 2: 配置扩展(TDD)

**Files:** Modify `src/config.rs`、`Cargo.toml`

- [ ] **Step 1: Cargo.toml 加 windows Threading 特性**

在 `Cargo.toml` 的 `[dependencies.windows]` 的 `features` 列表里追加这三项(供 Task 5 前台进程名用):
```toml
    "Win32_System_Threading",
    "Win32_System_ProcessStatus",
    "Win32_Security",
```

- [ ] **Step 2: 替换 config.rs 中的 hotkey 字段与 LlmConfig,新增结构与方法**

在 `src/config.rs`:

(a) 把 `Config` 里的 `pub hotkey: String,` 改为:
```rust
    pub hotkey: HotkeyConfig,
```

(b) 在 `Config` 的 `Default` 实现里,把 `hotkey: "LWin".into(),` 改为:
```rust
            hotkey: HotkeyConfig::default(),
```

(c) 在 `Config` 结构体里,`inject: InjectConfig,` 之后加一行字段:
```rust
    #[serde(default)]
    pub app_style: Vec<AppStyle>,
```
并在 `Config` 的 `Default` 实现里 `inject: InjectConfig::default(),` 之后加:
```rust
            app_style: Vec::new(),
```

(d) 在 `LlmConfig` 结构体里,`skip_if_shorter_than: usize,` 之后加两字段:
```rust
    /// 专有名词;非空时注入提示词,优先按此拼写。
    pub vocabulary: Vec<String>,
    /// 命令模式提示词;留空用内置默认。
    pub command_prompt: String,
```
并在 `LlmConfig` 的 `Default` 实现里 `skip_if_shorter_than: 4,` 之后加:
```rust
            vocabulary: Vec::new(),
            command_prompt: String::new(),
```

(e) 在文件中(任意顶层位置,建议 InjectConfig 之后)新增结构:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub primary: String,
    pub translate_modifier: String,
    pub command_modifier: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            primary: "LWin".into(),
            translate_modifier: "LAlt".into(),
            command_modifier: "LCtrl".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppStyle {
    /// 前台进程名包含此串(不区分大小写)即命中。
    #[serde(rename = "match")]
    pub match_: String,
    /// 命中后追加到提示词的风格指令。
    pub style: String,
}
```

(f) 在 `impl LlmConfig` 中(已有 effective_system_prompt 等)新增:
```rust
    /// 词库提示行;词库为空返回 None。
    pub fn vocabulary_line(&self) -> Option<String> {
        let terms: Vec<&str> = self
            .vocabulary
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if terms.is_empty() {
            None
        } else {
            Some(format!("以下专有名词若出现请按此拼写:{}。", terms.join("、")))
        }
    }

    /// 命令模式实际提示词:自定义优先,否则内置默认。
    pub fn effective_command_prompt(&self) -> String {
        if !self.command_prompt.trim().is_empty() {
            self.command_prompt.clone()
        } else {
            PROMPT_COMMAND.into()
        }
    }
```

(g) 新增 PROMPT_COMMAND 常量(放在其他 PROMPT_* 常量附近):
```rust
const PROMPT_COMMAND: &str = "你是文本编辑器。用户选中了一段文本,并口述了一条修改指令。\
请把指令应用到这段文本,只输出修改后的文本本身,不要解释、不要引号或代码块。\
若指令要求翻译,按要求翻译;否则保持与原文一致的语言。";
```

(h) 在 `impl Config` 中新增按前台进程名匹配风格:
```rust
    /// 按前台进程名匹配应用风格,返回首条命中的 style。
    pub fn style_for(&self, process_name: &str) -> Option<String> {
        let pname = process_name.to_ascii_lowercase();
        self.app_style
            .iter()
            .find(|a| !a.match_.trim().is_empty() && pname.contains(&a.match_.to_ascii_lowercase()))
            .map(|a| a.style.clone())
    }
```

- [ ] **Step 3: 更新/新增测试**

config.rs 已有的 `parses_full_config` 测试里有顶层 `hotkey = "RWin"`(字符串),现在 hotkey 是表,会解析失败。把该测试中的:
```
hotkey = "RWin"
```
整行删除(其余不动;hotkey 缺失会用默认 HotkeyConfig)。并把该测试末尾的断言 `assert_eq!(cfg.hotkey, "RWin");`(如存在)删除。

在 `#[cfg(test)] mod tests` 中追加:
```rust
    #[test]
    fn hotkey_table_parses_with_defaults() {
        let cfg: Config = toml::from_str(
            r#"
[hotkey]
command_modifier = "RCtrl"
"#,
        )
        .unwrap();
        assert_eq!(cfg.hotkey.primary, "LWin"); // 默认
        assert_eq!(cfg.hotkey.translate_modifier, "LAlt"); // 默认
        assert_eq!(cfg.hotkey.command_modifier, "RCtrl"); // 给定
    }

    #[test]
    fn vocabulary_line_joins_or_none() {
        let mut llm = LlmConfig::default();
        assert!(llm.vocabulary_line().is_none());
        llm.vocabulary = vec!["Kubernetes".into(), " ".into(), "OneDrive".into()];
        assert_eq!(
            llm.vocabulary_line().unwrap(),
            "以下专有名词若出现请按此拼写:Kubernetes、OneDrive。"
        );
    }

    #[test]
    fn style_for_matches_first_by_substring_ci() {
        let cfg: Config = toml::from_str(
            r#"
[[app_style]]
match = "OUTLOOK"
style = "正式"
[[app_style]]
match = "code"
style = "技术"
"#,
        )
        .unwrap();
        assert_eq!(cfg.style_for("OUTLOOK.EXE").as_deref(), Some("正式"));
        assert_eq!(cfg.style_for("Code.exe").as_deref(), Some("技术"));
        assert_eq!(cfg.style_for("notepad.exe"), None);
    }

    #[test]
    fn effective_command_prompt_default_then_custom() {
        let mut llm = LlmConfig::default();
        assert!(llm.effective_command_prompt().contains("文本编辑器"));
        llm.command_prompt = "自定义命令".into();
        assert_eq!(llm.effective_command_prompt(), "自定义命令");
    }
```

- [ ] **Step 4: 测试**
`...; cargo test --lib config`
Expected: 原有 + 4 新增全部通过。

- [ ] **Step 5: Commit**
```bash
git add Cargo.toml src/config.rs && git commit -m "feat: 配置加 [hotkey]/词库/命令提示词/应用风格"
```

---

## Task 3: 状态机泛化为主键+两修饰键(TDD)

**Files:** Modify `src/hotkey/state.rs`(整体替换)

- [ ] **Step 1: 整体替换 state.rs**

把 `src/hotkey/state.rs` 全文替换为:
```rust
/// 进入状态机的归一化事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 主键按下
    PrimaryDown,
    /// 主键释放,附本次按住时长(毫秒)
    PrimaryUp { held_ms: u64 },
    /// 修饰键 A(翻译)按下/释放
    ModADown,
    ModAUp,
    /// 修饰键 B(命令)按下/释放
    ModBDown,
    ModBUp,
    /// 其他任意键按下
    OtherDown,
}

/// 状态机给出的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    StopAndTranslate,
    StopAndCommand,
    DiscardRecording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    pub suppress: bool,
}

/// 低于该时长的"单独按住"视为误触。
pub const MIN_HOLD_MS: u64 = 300;

#[derive(Debug, Default)]
pub struct HotkeyState {
    pressed: bool,
    combo: bool,
    mod_a_seen: bool,
    mod_b_seen: bool,
    mod_a_down: bool,
    mod_b_down: bool,
    mod_a_swallow: bool,
    mod_b_swallow: bool,
}

impl HotkeyState {
    pub fn handle(&mut self, event: Event) -> Decision {
        match event {
            Event::PrimaryDown => {
                if self.pressed {
                    Decision { action: Action::None, suppress: false }
                } else {
                    self.pressed = true;
                    self.combo = false;
                    // 若修饰键已先按住,本次即对应模式。
                    self.mod_a_seen = self.mod_a_down;
                    self.mod_b_seen = self.mod_b_down;
                    Decision { action: Action::StartRecording, suppress: false }
                }
            }
            Event::ModADown => {
                self.mod_a_down = true;
                if self.pressed && !self.combo {
                    self.mod_a_seen = true;
                    self.mod_a_swallow = true;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModAUp => {
                self.mod_a_down = false;
                if self.mod_a_swallow {
                    self.mod_a_swallow = false;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModBDown => {
                self.mod_b_down = true;
                if self.pressed && !self.combo {
                    self.mod_b_seen = true;
                    self.mod_b_swallow = true;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::ModBUp => {
                self.mod_b_down = false;
                if self.mod_b_swallow {
                    self.mod_b_swallow = false;
                    Decision { action: Action::None, suppress: true }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::OtherDown => {
                if self.pressed && !self.combo {
                    self.combo = true;
                    Decision { action: Action::CancelRecording, suppress: false }
                } else {
                    Decision { action: Action::None, suppress: false }
                }
            }
            Event::PrimaryUp { held_ms } => {
                if !self.pressed {
                    return Decision { action: Action::None, suppress: false };
                }
                let was_combo = self.combo;
                let a = self.mod_a_seen;
                let b = self.mod_b_seen;
                self.pressed = false;
                self.combo = false;
                self.mod_a_seen = false;
                self.mod_b_seen = false;
                if was_combo {
                    Decision { action: Action::None, suppress: false }
                } else if held_ms >= MIN_HOLD_MS {
                    // 优先级:命令 > 翻译 > 普通
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_alone_transcribes() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::PrimaryDown),
            Decision { action: Action::StartRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranscribe, suppress: true }
        );
    }

    #[test]
    fn too_short_discards() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 100 }),
            Decision { action: Action::DiscardRecording, suppress: true }
        );
    }

    #[test]
    fn primary_plus_mod_a_translates_and_swallows() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::ModADown),
            Decision { action: Action::None, suppress: true }
        );
        assert_eq!(
            s.handle(Event::ModAUp),
            Decision { action: Action::None, suppress: true }
        );
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranslate, suppress: true }
        );
    }

    #[test]
    fn primary_plus_mod_b_commands_and_swallows() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::ModBDown),
            Decision { action: Action::None, suppress: true }
        );
        assert_eq!(
            s.handle(Event::ModBUp),
            Decision { action: Action::None, suppress: true }
        );
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::StopAndCommand, suppress: true }
        );
    }

    #[test]
    fn command_beats_translate_when_both_held() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        s.handle(Event::ModADown);
        s.handle(Event::ModBDown);
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::StopAndCommand, suppress: true }
        );
    }

    #[test]
    fn mod_a_held_before_primary_translates() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::ModADown),
            Decision { action: Action::None, suppress: false }
        );
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::StopAndTranslate, suppress: true }
        );
    }

    #[test]
    fn other_key_cancels_as_combo() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::OtherDown),
            Decision { action: Action::CancelRecording, suppress: false }
        );
        assert_eq!(
            s.handle(Event::PrimaryUp { held_ms: 1000 }),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn mods_outside_hold_pass_through() {
        let mut s = HotkeyState::default();
        assert_eq!(
            s.handle(Event::ModADown),
            Decision { action: Action::None, suppress: false }
        );
        assert_eq!(
            s.handle(Event::ModBDown),
            Decision { action: Action::None, suppress: false }
        );
        assert_eq!(
            s.handle(Event::ModAUp),
            Decision { action: Action::None, suppress: false }
        );
        assert_eq!(
            s.handle(Event::ModBUp),
            Decision { action: Action::None, suppress: false }
        );
    }

    #[test]
    fn repeated_primary_down_is_noop() {
        let mut s = HotkeyState::default();
        s.handle(Event::PrimaryDown);
        assert_eq!(
            s.handle(Event::PrimaryDown),
            Decision { action: Action::None, suppress: false }
        );
    }
}
```

- [ ] **Step 2: 测试(此时 mod.rs 尚未适配,只测 state)**
`...; cargo test --lib hotkey::state`
Expected: 9 个测试通过。(注意:整体 `cargo build` 此时会因 mod.rs 用旧事件名而失败,正常;Task 4/6 修复。先只跑 state 测试。)

- [ ] **Step 3: Commit**
```bash
git add src/hotkey/state.rs && git commit -m "feat: 状态机泛化为主键+两修饰键(翻译/命令)"
```

---

## Task 4: corrector 支持上下文注入 + 命令模式(TDD)

**Files:** Modify `src/corrector.rs`

- [ ] **Step 1: 替换 impl 与 build_request_body,新增 compose_system_prompt 与 command**

在 `src/corrector.rs`:

(a) 把现有的 `correct`/`translate`/`process`/`try_process` 方法整体替换为:
```rust
    /// 普通整理。`style` 为可选的应用风格指令。失败回退原文。
    pub fn correct(&self, raw: &str, style: Option<&str>) -> String {
        let sys = compose_system_prompt(
            &self.cfg.effective_system_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            style,
        );
        self.process(raw, &sys)
    }

    /// 翻译成英文。`style` 为可选的应用风格指令。失败回退原文。
    pub fn translate(&self, raw: &str, style: Option<&str>) -> String {
        let sys = compose_system_prompt(
            &self.cfg.effective_translate_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            style,
        );
        self.process(raw, &sys)
    }

    /// 命令模式:把 `instruction` 应用到 `selected`。失败回退原选中文本。
    pub fn command(&self, instruction: &str, selected: &str) -> String {
        if !self.cfg.enabled {
            return selected.to_string();
        }
        let sys = compose_system_prompt(
            &self.cfg.effective_command_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            None,
        );
        let user = format!("指令:{}\n\n文本:\n{}", instruction.trim(), selected);
        match self.try_chat(&sys, &user) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => selected.to_string(),
            Err(e) => {
                eprintln!("LLM 命令失败,保留原文: {e}");
                selected.to_string()
            }
        }
    }

    /// 用给定系统提示词处理文本(用户消息即文本本身);失败回退原文。
    fn process(&self, raw: &str, system_prompt: &str) -> String {
        let trimmed = raw.trim();
        if !self.cfg.enabled || trimmed.chars().count() < self.cfg.skip_if_shorter_than {
            return raw.to_string();
        }
        match self.try_chat(system_prompt, trimmed) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => raw.to_string(),
            Err(e) => {
                eprintln!("LLM 处理失败,回退原文: {e}");
                raw.to_string()
            }
        }
    }

    fn try_chat(&self, system_prompt: &str, user_text: &str) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let body = build_request_body(&self.cfg, system_prompt, user_text);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.cfg.api_key)
            .json(&body)
            .send()?
            .error_for_status()?;
        let value: Value = resp.json()?;
        parse_response(&value)
            .ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))
    }
```

(b) 在 `build_request_body` 上方新增 compose:
```rust
/// 把词库行、应用风格依次拼到基础系统提示词后面(空项跳过)。
pub fn compose_system_prompt(
    base: &str,
    vocabulary_line: Option<&str>,
    style: Option<&str>,
) -> String {
    let mut s = base.to_string();
    for extra in [vocabulary_line, style].into_iter().flatten() {
        if !extra.trim().is_empty() {
            s.push_str("\n\n");
            s.push_str(extra.trim());
        }
    }
    s
}
```

(c) `build_request_body` 与 `parse_response` 保持不变(签名 `build_request_body(cfg, system_prompt, raw)`)。

- [ ] **Step 2: 更新测试**

corrector.rs 的测试里:
- `request_body_has_model_and_two_messages` 保持(它直接调 build_request_body,不变)。
- `disabled_returns_raw` 把 `corrector.correct("原始文本")` 改为 `corrector.correct("原始文本", None)`。
- `too_short_returns_raw` 把 `corrector.correct("嗯")` 改为 `corrector.correct("嗯", None)`。

新增测试:
```rust
    #[test]
    fn compose_appends_vocab_and_style() {
        let s = compose_system_prompt("BASE", Some("VOCAB"), Some("STYLE"));
        assert!(s.starts_with("BASE"));
        assert!(s.contains("VOCAB"));
        assert!(s.contains("STYLE"));
    }

    #[test]
    fn compose_skips_empty() {
        assert_eq!(compose_system_prompt("BASE", None, Some("  ")), "BASE");
    }

    #[test]
    fn command_disabled_returns_selected() {
        let mut c = cfg();
        c.enabled = false;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.command("改短", "一段很长的文本"), "一段很长的文本");
    }
```

- [ ] **Step 3: 测试**
`...; cargo test --lib corrector`
Expected: 全部通过。

- [ ] **Step 4: Commit**
```bash
git add src/corrector.rs && git commit -m "feat: corrector 上下文注入(词库/风格)+ 命令模式"
```

---

## Task 5: 前台进程名模块(编译验证)

**Files:** Modify `src/foreground.rs`

- [ ] **Step 1: 实现**

写入 `src/foreground.rs`:
```rust
//! 取当前前台窗口所属进程的可执行文件名(小写)。失败返回 None。

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// 返回前台进程的文件名(如 "outlook.exe"),小写。
pub fn foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = path
            .rsplit(|c| c == '\\' || c == '/')
            .next()
            .unwrap_or(&path)
            .to_ascii_lowercase();
        Some(name)
    }
}
```

- [ ] **Step 2: 编译**
`...; cargo build`
Expected:此时 mod.rs/main.rs 仍未适配会报错,但本任务关注 foreground 自身。可改用 `cargo build 2>&1 | Select-String "foreground"` 确认 foreground.rs 无自身错误;整体编译在 Task 6/7 后通过。
若 windows 0.58 的 `OpenProcess` 返回类型/`QueryFullProcessImageNameW` 签名不同,按 crate 源(`C:\Users\jklov\.cargo\registry\src\...\windows-0.58*`)适配,保持函数签名 `foreground_process_name() -> Option<String>` 不变。

- [ ] **Step 3: Commit**
```bash
git add src/foreground.rs && git commit -m "feat: 前台窗口进程名"
```

---

## Task 6: 钩子配置化 VK + 命令动作(编译验证)

**Files:** Modify `src/hotkey/mod.rs`(整体替换)

- [ ] **Step 1: 整体替换 mod.rs**

把 `src/hotkey/mod.rs` 全文替换为:
```rust
pub mod state;

use std::sync::OnceLock;
use std::time::Instant;

use crossbeam_channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use state::{Decision, Event, HotkeyState};

/// 钩子触发后向主线程发送的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    StopAndTranslate,
    StopAndCommand,
    DiscardRecording,
}

struct HookCtx {
    state: HotkeyState,
    down_at: Option<Instant>,
    sender: Sender<HotkeyAction>,
    primary_vk: u16,
    mod_a_vk: u16,
    mod_b_vk: u16,
}

static CTX: OnceLock<std::sync::Mutex<HookCtx>> = OnceLock::new();

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    if kb.dwExtraInfo == crate::INJECTED_TAG {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let vk = kb.vkCode as u16;
    let msg = wparam.0 as u32;
    let is_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
    if !is_down && !is_up {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let mut suppress = false;
    let mut primary_vk = 0u16;
    let mut is_primary_up = false;

    if let Some(ctx_lock) = CTX.get() {
        let mut ctx = ctx_lock.lock().unwrap_or_else(|p| p.into_inner());
        primary_vk = ctx.primary_vk;
        let event = if vk == ctx.primary_vk {
            if is_down {
                if ctx.down_at.is_none() {
                    ctx.down_at = Some(Instant::now());
                }
                Some(Event::PrimaryDown)
            } else {
                let held = ctx
                    .down_at
                    .take()
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                is_primary_up = true;
                Some(Event::PrimaryUp { held_ms: held })
            }
        } else if vk == ctx.mod_a_vk {
            Some(if is_down { Event::ModADown } else { Event::ModAUp })
        } else if vk == ctx.mod_b_vk {
            Some(if is_down { Event::ModBDown } else { Event::ModBUp })
        } else if is_down {
            Some(Event::OtherDown)
        } else {
            None
        };

        if let Some(event) = event {
            let decision: Decision = ctx.state.handle(event);
            dispatch(&ctx.sender, decision.action);
            suppress = decision.suppress;
        }
    }

    if suppress {
        if is_primary_up {
            disguise_release_key(primary_vk);
        }
        return LRESULT(1);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn dispatch(sender: &Sender<HotkeyAction>, action: state::Action) {
    use state::Action;
    let mapped = match action {
        Action::StartRecording => Some(HotkeyAction::StartRecording),
        Action::CancelRecording => Some(HotkeyAction::CancelRecording),
        Action::StopAndTranscribe => Some(HotkeyAction::StopAndTranscribe),
        Action::StopAndTranslate => Some(HotkeyAction::StopAndTranslate),
        Action::StopAndCommand => Some(HotkeyAction::StopAndCommand),
        Action::DiscardRecording => Some(HotkeyAction::DiscardRecording),
        Action::None => None,
    };
    if let Some(a) = mapped {
        let _ = sender.send(a);
    }
}

/// 吞掉物理主键弹起后:补发 Ctrl 轻敲(防开始菜单)+ 合成主键弹起(解除卡键)。
unsafe fn disguise_release_key(primary_vk: u16) {
    let inputs = [
        tagged_key(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        tagged_key(VK_CONTROL.0, KEYEVENTF_KEYUP),
        tagged_key(primary_vk, KEYEVENTF_KEYUP),
    ];
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

fn tagged_key(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
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

/// 安装钩子并进入消息循环(阻塞)。需提供三个热键 VK。
pub fn run(
    sender: Sender<HotkeyAction>,
    primary_vk: u16,
    mod_a_vk: u16,
    mod_b_vk: u16,
) -> anyhow::Result<()> {
    let _ = CTX.set(std::sync::Mutex::new(HookCtx {
        state: HotkeyState::default(),
        down_at: None,
        sender,
        primary_vk,
        mod_a_vk,
        mod_b_vk,
    }));
    unsafe {
        let _hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)?;
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    }
    Ok(())
}
```

- [ ] **Step 2: 编译(整体仍需 main 适配,先确认 mod 无自身错误)**
`...; cargo build 2>&1 | Select-String "hotkey|error\["`
Expected: hotkey 模块自身无错误(main.rs 因 run 签名变化报错属正常,Task 7 修)。

- [ ] **Step 3: Commit**
```bash
git add src/hotkey/mod.rs && git commit -m "feat: 钩子配置化 VK + 命令动作 + 通用伪装释放"
```

---

## Task 7: main 串联(配置化热键 / 应用风格 / 命令模式)+ 配置文件

**Files:** Modify `src/main.rs`、`config.example.toml`、`config.toml`

- [ ] **Step 1: 替换 src/main.rs**

```rust
use std::{thread, time::Duration};

use arboard::Clipboard;
use crossbeam_channel::unbounded;
use voice_input::asr::Transcriber;
use voice_input::audio::Recorder;
use voice_input::config::Config;
use voice_input::corrector::Corrector;
use voice_input::foreground::foreground_process_name;
use voice_input::hotkey::{self, HotkeyAction};
use voice_input::inject::inject_text;
use voice_input::keys::vk_from_name;

fn main() -> anyhow::Result<()> {
    let config = Config::load("config.toml")?;
    let primary = vk_from_name(&config.hotkey.primary)?;
    let mod_a = vk_from_name(&config.hotkey.translate_modifier)?;
    let mod_b = vk_from_name(&config.hotkey.command_modifier)?;

    let mut transcriber = Transcriber::load(&config.asr.model_dir, &config.asr.language)?;
    let corrector = Corrector::new(config.llm.clone())?;
    println!(
        "已就绪。{}=识别整理;{}+{}=翻译英文;{}+{}=对选中文字执行语音命令。按住说话,松开输出。",
        config.hotkey.primary,
        config.hotkey.primary,
        config.hotkey.translate_modifier,
        config.hotkey.primary,
        config.hotkey.command_modifier,
    );

    let (tx, rx) = unbounded::<HotkeyAction>();
    thread::spawn(move || {
        if let Err(e) = hotkey::run(tx, primary, mod_a, mod_b) {
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
                recorder = None;
            }
            action @ (HotkeyAction::StopAndTranscribe
            | HotkeyAction::StopAndTranslate
            | HotkeyAction::StopAndCommand) => {
                let Some(r) = recorder.take() else { continue };
                let (samples, rate) = r.stop();
                let raw = match transcriber.transcribe(&samples, rate) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("识别失败: {e}");
                        continue;
                    }
                };
                println!("识别: {raw}");
                let style = foreground_process_name().and_then(|p| config.style_for(&p));
                match action {
                    HotkeyAction::StopAndCommand => {
                        handle_command(&corrector, &raw);
                    }
                    HotkeyAction::StopAndTranslate => {
                        let text = corrector.translate(&raw, style.as_deref());
                        if text != raw {
                            println!("翻译: {text}");
                        }
                        if let Err(e) = inject_text(&text) {
                            eprintln!("注入失败: {e}");
                        }
                    }
                    _ => {
                        let text = corrector.correct(&raw, style.as_deref());
                        if text != raw {
                            println!("修整: {text}");
                        }
                        if let Err(e) = inject_text(&text) {
                            eprintln!("注入失败: {e}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// 命令模式:复制选中文本 → LLM 应用指令 → 替换。无选中则退化为普通插入指令文本。
fn handle_command(corrector: &Corrector, instruction: &str) {
    let selected = match copy_selection() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取选中文本失败: {e}");
            String::new()
        }
    };
    if selected.trim().is_empty() {
        // 没选中:把指令本身当普通输入插入(降级)
        let text = corrector.correct(instruction, None);
        let _ = inject_text(&text);
        return;
    }
    let result = corrector.command(instruction, &selected);
    println!("命令: {instruction}\n结果: {result}");
    if let Err(e) = inject_text(&result) {
        eprintln!("注入失败: {e}");
    }
}

/// 发送 Ctrl+C 复制当前选中,读回剪贴板文本;读完恢复原剪贴板。
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
                    dwExtraInfo: voice_input::INJECTED_TAG,
                },
            },
        }
    }
    let mut clipboard = Clipboard::new()?;
    let previous = clipboard.get_text().ok();
    // 先清空,便于判断是否真的复制到内容
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
```

- [ ] **Step 2: 更新 config.example.toml**

把 `config.example.toml` 顶部的 `hotkey = "LWin"` 整行替换为一个 `[hotkey]` 表,并在 `[llm]` 段补充新字段、文末补 `[[app_style]]` 示例。最终 `config.example.toml` 为:
```toml
# 复制本文件为 config.toml 并填入你的中转站信息。改完保存后重启程序生效。
#
# 热键:按住【主键】说话=识别整理;主键+翻译键=译成英文;主键+命令键=对选中文字执行语音命令。

[hotkey]
primary            = "LWin"   # 可用:LWin RWin LAlt RAlt LCtrl RCtrl LShift RShift
translate_modifier = "LAlt"
command_modifier   = "LCtrl"

[asr]
model_dir = "./models/sensevoice"
language  = "auto"        # auto / zh / en ...

[llm]
enabled  = true           # false 则直接输出原始识别文本,不调用 LLM
base_url = "https://www.packyapi.com/v1"
api_key  = "sk-在这里填你的key"
model    = "deepseek-v4-flash"
mode = "polish"           # clean / polish / summary
temperature = 0.0
timeout_secs = 10
skip_if_shorter_than = 4
vocabulary = []           # 专有名词,如 ["Kubernetes", "OneDrive"];优先按此拼写
# system_prompt = ""       # 非空则覆盖 mode 预设
# translate_prompt = ""    # 翻译模式提示词;留空用内置默认
# command_prompt = ""      # 命令模式提示词;留空用内置默认

[inject]
mode = "paste"            # 目前仅支持 paste

# 随应用风格(可多条);按前台进程名包含匹配,取第一条命中
# [[app_style]]
# match = "outlook"
# style = "用正式、专业的书面语。"
# [[app_style]]
# match = "code"
# style = "保留代码与技术术语原文。"
```

- [ ] **Step 3: 更新 config.toml(真实,gitignore)**

把 `config.toml` 顶部 `hotkey = "LWin"` 整行替换为:
```toml
[hotkey]
primary            = "LWin"
translate_modifier = "LAlt"
command_modifier   = "LCtrl"
```
并在 `[llm]` 段加一行 `vocabulary = []`(其余保持)。

- [ ] **Step 4: 编译 release**
`...; cargo build --release`
Expected: 整体编译成功。

- [ ] **Step 5: Commit(不 add config.toml)**
```bash
git add src/main.rs config.example.toml && git commit -m "feat: 串联可配置热键/应用风格/选中命令模式"
```
然后 bash 确认:`git status --porcelain config.toml` 应无输出。

---

## Task 8: 全量测试 + 端到端

- [ ] **Step 1: 全部单测**
`...; cargo test --lib`
Expected:keys(3)+config(原有+4)+state(9)+corrector(原有+3)+ 其余阶段一/二 = 全绿。

- [ ] **Step 2: 真实 API 验证命令模式(控制器)**
用 example 验证 command():
新增并运行一次性验证(可临时在 correct_text 里加,或控制器另写);确认 `corrector.command("把这段改成正式语气", "嘿哥们这事儿稳了")` 返回正式化结果。

- [ ] **Step 3: 真机端到端(用户)**
控制器后台运行 release;用户验证:
- 主键说话 → 整理输出(回归)
- 主键+Alt → 英文(回归)
- 选中一段文字 → 主键+Ctrl 说"改成正式语气/翻译成英文/改短一点" → 选中被替换为结果
- 词库:把易错专名加入 `vocabulary`,说含该词的话,确认拼写被纠正
- 应用风格:配一条 `[[app_style]]`,在该应用里说话确认风格变化
- 回归:Win+E 正常、松开 Win 不弹菜单、单按 E 出 e

## 完成标准
- [ ] 单测全绿。
- [ ] 四个新功能真机可用;原有两热键行为不回归。
- [ ] config.toml 不入库;config.example.toml 含完整示例。
