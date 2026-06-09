# 语音输入工具 — 阶段二(LLM 纠错 + TOML 配置)实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在阶段一原始识别链路后插入 LLM 纠错/去口语,并把所有参数抽到可随时修改的 `config.toml`。

**Architecture:** 新增 `config`(TOML 反序列化,字段全部带默认值,缺字段不报错)与
`corrector`(reqwest 阻塞调用 OpenAI 兼容 `/v1/chat/completions`)两个模块。纠错失败/
超时/禁用/文本过短一律回退原文,永不丢输入。`main.rs` 在 `StopAndTranscribe` 路径里
插入纠错步骤。ASR 改为从配置读取模型目录与语言。

**Tech Stack:** serde + toml(配置)、reqwest blocking + rustls + serde_json(HTTP)。

---

## 已知运行环境(给执行者)
- cargo 不在新 shell 的 PATH 上;每条 cargo 命令前置:
  `$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo ...`
- 必须用 **PowerShell** 跑 cargo(不是 unix bash)。git 用 bash 即可。
- 工作目录已是项目根,勿 `cd`。
- 当前在 `master` 分支,阶段一已合并。本阶段在新分支 `phase2-llm-config` 上做(控制器已建好)。

## 文件结构

| 文件 | 职责 | 改动 |
|------|------|------|
| `Cargo.toml` | 加 serde/toml/serde_json/reqwest 依赖 | 改 |
| `src/lib.rs` | 导出 `config`、`corrector` 模块 | 改 |
| `src/config.rs` | `Config` 及子结构、默认值、`load(path)` | 新建 |
| `src/corrector.rs` | `Corrector`(LLM 纠错,带回退)+ 纯函数 `build_request_body`/`parse_response` | 新建 |
| `src/asr.rs` | `Transcriber::load` 改为接收 `model_dir`/`language` | 改 |
| `src/main.rs` | 载入配置、构造 Corrector、纠错后注入 | 改 |
| `examples/transcribe_wav.rs` | 适配新的 `Transcriber::load` 签名 | 改 |
| `config.example.toml` | 配置模板(不含真实 key),提交进库 | 新建 |
| `config.toml` | 真实配置(含 key),**gitignore,不提交** | 新建 |
| `.gitignore` | 加 `/config.toml` | 改 |

---

## Task 1: 依赖 + 配置模块(TDD)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/config.rs`

- [ ] **Step 1: 加依赖**

在 `Cargo.toml` 的 `[dependencies]` 末尾追加:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
```

- [ ] **Step 2: 在 lib.rs 导出模块**

把 `src/lib.rs` 顶部的模块声明改成(保留已有的 `INJECTED_TAG` 常量不动):

```rust
pub mod hotkey;
pub mod audio;
pub mod asr;
pub mod inject;
pub mod config;
pub mod corrector;
```

- [ ] **Step 3: 写配置模块 + 失败测试**

写入 `src/config.rs`(类型、默认值、`load`、测试都在内;`load` 先留空实现以制造失败):

```rust
//! TOML 配置。所有字段都有默认值,缺字段不报错,便于随时增减配置项。

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 触发热键(阶段一固定左 Win,此字段为阶段三预留)。
    pub hotkey: String,
    pub asr: AsrConfig,
    pub llm: LlmConfig,
    pub inject: InjectConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AsrConfig {
    pub model_dir: String,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// 是否启用 LLM 纠错;false 时直接用原始识别文本。
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub temperature: f32,
    pub timeout_secs: u64,
    /// 文本字符数小于该值时跳过 LLM(短词没必要纠错)。
    pub skip_if_shorter_than: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InjectConfig {
    /// 注入方式:目前实现 "paste";"type" 为预留。
    pub mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: "LWin".into(),
            asr: AsrConfig::default(),
            llm: LlmConfig::default(),
            inject: InjectConfig::default(),
        }
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        AsrConfig {
            model_dir: "./models/sensevoice".into(),
            language: "auto".into(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            enabled: true,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: default_system_prompt(),
            temperature: 0.2,
            timeout_secs: 10,
            skip_if_shorter_than: 4,
        }
    }
}

impl Default for InjectConfig {
    fn default() -> Self {
        InjectConfig { mode: "paste".into() }
    }
}

pub fn default_system_prompt() -> String {
    "你是语音转写后处理器。请对用户给出的语音识别文本做轻量整理:改正明显的同音或识别错误,\
去除\"嗯、啊、呃、那个、就是说\"等口语填充词,并补充合适的标点。务必保留原意、语气以及中英文\
专有名词,不要扩写、不要回答其中的问题。只输出整理后的纯文本,不要任何解释或引号包裹。"
        .into()
}

impl Config {
    /// 从 TOML 文件读取配置。
    pub fn load(path: &str) -> anyhow::Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {path}(可复制 config.example.toml 为 config.toml)"))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("解析配置文件失败: {path}"))?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let toml = r#"
hotkey = "RWin"
[asr]
model_dir = "./m"
language = "zh"
[llm]
enabled = true
base_url = "https://x/v1"
api_key = "k"
model = "deepseek-v4-flash"
system_prompt = "p"
temperature = 0.5
timeout_secs = 20
skip_if_shorter_than = 2
[inject]
mode = "paste"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.hotkey, "RWin");
        assert_eq!(cfg.asr.model_dir, "./m");
        assert_eq!(cfg.llm.base_url, "https://x/v1");
        assert_eq!(cfg.llm.model, "deepseek-v4-flash");
        assert_eq!(cfg.llm.timeout_secs, 20);
        assert_eq!(cfg.llm.skip_if_shorter_than, 2);
    }

    #[test]
    fn missing_fields_use_defaults() {
        // 只给 llm 的部分字段,其余应回落默认。
        let toml = r#"
[llm]
base_url = "https://y/v1"
model = "m"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.hotkey, "LWin"); // 默认
        assert_eq!(cfg.asr.model_dir, "./models/sensevoice"); // 默认
        assert_eq!(cfg.llm.base_url, "https://y/v1"); // 给定
        assert!(cfg.llm.enabled); // 默认 true
        assert_eq!(cfg.llm.temperature, 0.2); // 默认
        assert!(!cfg.llm.system_prompt.is_empty()); // 默认提示词非空
    }

    #[test]
    fn empty_toml_is_all_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.inject.mode, "paste");
        assert_eq!(cfg.llm.skip_if_shorter_than, 4);
    }
}
```

- [ ] **Step 4: 运行测试,确认通过**

PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test --lib config`
Expected: 3 个测试通过(首次会编译新依赖,较慢)。

- [ ] **Step 5: Commit**(bash)

```bash
git add Cargo.toml src/lib.rs src/config.rs && git commit -m "feat: TOML 配置模块(全字段默认值)"
```

---

## Task 2: LLM 纠错模块(TDD 纯逻辑)

**Files:**
- Create: `src/corrector.rs`

- [ ] **Step 1: 写模块 + 失败测试**

写入 `src/corrector.rs`(纯函数 `build_request_body`/`parse_response` 和 `Corrector::correct`
的可测分支;`try_correct` 走网络,不单测):

```rust
//! LLM 纠错。调用 OpenAI 兼容的 /chat/completions;失败/超时/禁用/过短均回退原文。

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::LlmConfig;

pub struct Corrector {
    client: reqwest::blocking::Client,
    cfg: LlmConfig,
}

impl Corrector {
    pub fn new(cfg: LlmConfig) -> anyhow::Result<Corrector> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs.max(1)))
            .build()?;
        Ok(Corrector { client, cfg })
    }

    /// 纠错。禁用 / 文本过短 / 网络失败 / 空响应 → 返回原文(绝不丢输入)。
    pub fn correct(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if !self.cfg.enabled || trimmed.chars().count() < self.cfg.skip_if_shorter_than {
            return raw.to_string();
        }
        match self.try_correct(trimmed) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => raw.to_string(),
            Err(e) => {
                eprintln!("LLM 纠错失败,回退原文: {e}");
                raw.to_string()
            }
        }
    }

    fn try_correct(&self, raw: &str) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let body = build_request_body(&self.cfg, raw);
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
}

/// 构造 chat/completions 请求体。
pub fn build_request_body(cfg: &LlmConfig, raw: &str) -> Value {
    json!({
        "model": cfg.model,
        "temperature": cfg.temperature,
        "messages": [
            { "role": "system", "content": cfg.system_prompt },
            { "role": "user", "content": raw },
        ],
    })
}

/// 从 chat/completions 响应里取 choices[0].message.content。
pub fn parse_response(value: &Value) -> Option<String> {
    value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    fn cfg() -> LlmConfig {
        LlmConfig {
            model: "deepseek-v4-flash".into(),
            temperature: 0.2,
            system_prompt: "SP".into(),
            ..Default::default()
        }
    }

    #[test]
    fn request_body_has_model_and_two_messages() {
        let body = build_request_body(&cfg(), "你好");
        assert_eq!(body["model"], "deepseek-v4-flash");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "SP");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "你好");
    }

    #[test]
    fn parse_response_extracts_content() {
        let v: Value = serde_json::from_str(
            r#"{"choices":[{"message":{"role":"assistant","content":"整理后的文本"}}]}"#,
        )
        .unwrap();
        assert_eq!(parse_response(&v).as_deref(), Some("整理后的文本"));
    }

    #[test]
    fn parse_response_missing_content_is_none() {
        let v: Value = serde_json::from_str(r#"{"choices":[]}"#).unwrap();
        assert_eq!(parse_response(&v), None);
    }

    #[test]
    fn disabled_returns_raw() {
        let mut c = cfg();
        c.enabled = false;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.correct("原始文本"), "原始文本");
    }

    #[test]
    fn too_short_returns_raw() {
        let mut c = cfg();
        c.enabled = true;
        c.skip_if_shorter_than = 10;
        let corrector = Corrector::new(c).unwrap();
        // "嗯" 长度 < 10,直接回退,不会发网络请求。
        assert_eq!(corrector.correct("嗯"), "嗯");
    }
}
```

- [ ] **Step 2: 运行测试,确认通过**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test --lib corrector`
Expected: 5 个测试通过。

- [ ] **Step 3: Commit**(bash)

```bash
git add src/corrector.rs && git commit -m "feat: LLM 纠错模块(OpenAI 兼容,失败回退原文)"
```

---

## Task 3: ASR 改为从配置读取模型目录/语言

**Files:**
- Modify: `src/asr.rs`
- Modify: `examples/transcribe_wav.rs`

- [ ] **Step 1: 改 Transcriber::load 签名**

把 `src/asr.rs` 中 `MODEL_DIR` 常量与 `load` 改为接收参数(保留 `transcribe` 不变)。
将原来的:

```rust
pub const MODEL_DIR: &str = "./models/sensevoice";
```
删除该常量,并把 `load` 改为:

```rust
    /// 加载 SenseVoice 离线识别器。
    /// `model_dir` 下应有 model.onnx 与 tokens.txt;`language` 取 "auto"/"zh"/"en" 等。
    pub fn load(model_dir: &str, language: &str) -> Result<Transcriber> {
        let model = format!("{model_dir}/model.onnx");
        let tokens = format!("{model_dir}/tokens.txt");

        let config = SenseVoiceConfig {
            model,
            tokens,
            language: language.to_string(),
            use_itn: true,
            ..Default::default()
        };

        let recognizer = SenseVoiceRecognizer::new(config)
            .map_err(|e| anyhow!("创建 SenseVoice 识别器失败: {e}"))
            .context("加载 SenseVoice 模型失败,请确认模型文件存在")?;

        Ok(Transcriber { recognizer })
    }
```

(若文件顶部因删除常量出现未使用导入,清理之;`anyhow`/`Context`/`anyhow!` 仍在用。)

- [ ] **Step 2: 适配 example**

把 `examples/transcribe_wav.rs` 里:

```rust
    let mut transcriber = Transcriber::load()?;
```
改为:

```rust
    let mut transcriber = Transcriber::load("./models/sensevoice", "auto")?;
```

- [ ] **Step 3: 编译验证**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build`
Expected: 编译成功。

- [ ] **Step 4: Commit**(bash)

```bash
git add src/asr.rs examples/transcribe_wav.rs && git commit -m "refactor: ASR 模型目录/语言改为参数传入"
```

---

## Task 4: 串联配置与纠错 + 生成配置文件

**Files:**
- Modify: `src/main.rs`
- Create: `config.example.toml`
- Create: `config.toml`
- Modify: `.gitignore`

- [ ] **Step 1: 改 main.rs**

替换 `src/main.rs`:

```rust
use crossbeam_channel::unbounded;
use voice_input::asr::Transcriber;
use voice_input::audio::Recorder;
use voice_input::config::Config;
use voice_input::corrector::Corrector;
use voice_input::hotkey::{self, HotkeyAction};
use voice_input::inject::inject_text;

fn main() -> anyhow::Result<()> {
    let config = Config::load("config.toml")?;
    let mut transcriber = Transcriber::load(&config.asr.model_dir, &config.asr.language)?;
    let corrector = Corrector::new(config.llm.clone())?;
    if config.llm.enabled {
        println!("SenseVoice + LLM 纠错已就绪。按住左 Win 说话,松开输出。");
    } else {
        println!("SenseVoice 已就绪(LLM 纠错已禁用)。按住左 Win 说话,松开输出。");
    }

    let (tx, rx) = unbounded::<HotkeyAction>();

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
                recorder = None;
            }
            HotkeyAction::StopAndTranscribe => {
                if let Some(r) = recorder.take() {
                    let (samples, rate) = r.stop();
                    match transcriber.transcribe(&samples, rate) {
                        Ok(raw) => {
                            println!("识别: {raw}");
                            let text = corrector.correct(&raw);
                            if text != raw {
                                println!("修整: {text}");
                            }
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

- [ ] **Step 2: 创建 config.example.toml(模板,提交进库,不含真实 key)**

写入 `config.example.toml`:

```toml
# 复制本文件为 config.toml 并填入你的中转站信息。
# 改完保存后重启程序生效。

hotkey = "LWin"          # 触发热键(阶段三可改;目前固定左 Win)

[asr]
model_dir = "./models/sensevoice"
language  = "auto"        # auto / zh / en ...

[llm]
enabled  = true           # false 则直接输出原始识别文本,不调用 LLM
base_url = "https://www.packyapi.com/v1"
api_key  = "sk-在这里填你的key"
model    = "deepseek-v4-flash"
temperature = 0.2
timeout_secs = 10         # 超时即回退原始识别文本
skip_if_shorter_than = 4  # 文本短于该字符数时跳过 LLM
# system_prompt = "自定义纠错提示词;留空则用内置默认"

[inject]
mode = "paste"            # 目前仅支持 paste
```

- [ ] **Step 3: 创建 config.toml(真实配置,稍后 gitignore)**

写入 `config.toml`(填入真实 key):

```toml
hotkey = "LWin"

[asr]
model_dir = "./models/sensevoice"
language  = "auto"

[llm]
enabled  = true
base_url = "https://www.packyapi.com/v1"
api_key  = "sk-REDACTED"
model    = "deepseek-v4-flash"
temperature = 0.2
timeout_secs = 10
skip_if_shorter_than = 4

[inject]
mode = "paste"
```

- [ ] **Step 4: gitignore config.toml**

在 `.gitignore` 追加一行:

```
/config.toml
```

- [ ] **Step 5: 编译 release + 确认 config.toml 不被跟踪**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build --release`
Expected: 编译成功。
bash 验证忽略生效:`git status --porcelain config.toml` 应**无输出**(被忽略)。

- [ ] **Step 6: Commit**(bash,注意不要 add config.toml)

```bash
git add src/main.rs config.example.toml .gitignore && git commit -m "feat: 串联 LLM 纠错 + 配置文件(config.toml 不入库)"
```

---

## Task 5: 全量测试 + 端到端

- [ ] **Step 1: 跑全部单测**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo test --lib`
Expected: 阶段一 10 个 + 配置 3 个 + 纠错 5 个 = 18 个测试全部通过。

- [ ] **Step 2: 端到端(由控制器/用户)**

控制器在项目根后台运行 `target\release\voice-input.exe`(工作目录=项目根,
`config.toml` 与 `models/` 才能被找到),用户按住左 Win 说一句带口语词的话
(如"嗯……今天天气不错那个 hello world"),松开。
Expected:
- 控制台先打印 `识别: ...`(原始),再打印 `修整: ...`(LLM 去口语+纠错后);
- 粘进应用的是**修整后**文本;
- 若中转站不可用,控制台打印"LLM 纠错失败,回退原文",仍能粘出原始文本。

---

## 阶段二完成标准

- [ ] 18 个单测全绿。
- [ ] `config.toml` 可改 base_url/key/model/提示词/温度/超时/跳过阈值,重启生效。
- [ ] `config.toml` 不入库;`config.example.toml` 入库。
- [ ] 真机:口语词被去除、识别错误被纠正;LLM 故障时回退原文不丢输入。
