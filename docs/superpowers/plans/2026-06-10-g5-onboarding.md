# ByType G5 首启向导 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 首次运行时检测缺失(config/模型/依赖),用复用主窗口的分步向导引导用户下载模型、填 LLM、检测环境,完成后当场启动引擎。

**Architecture:** 核心 crate 加 readiness/download 与 config `[model]` 段;src-tauri 新 `wizard.rs`(依赖检测/下载/导入/完成命令)+ `start_engine` 改为可延迟触发的 once 启动;前端 `main.tsx` 按 `wizard_state` 分流渲染向导或 G4 设置,向导 `src-ui/wizard/` 五步。

**Tech Stack:** Rust(reqwest blocking streaming、windows LoadLibraryW、winreg、cpal)、Tauri 2、React 18 + TS + Tailwind。

**规格:** `docs/superpowers/specs/2026-06-10-g5-onboarding-design.md`。

**构建环境(每个 cargo 命令前先跑,PowerShell):**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

前端检查:`npx tsc --noEmit` 与 `npm run build`(仓库根,Node 在 PATH)。**判定 cargo 成功只认 `$LASTEXITCODE`**(PowerShell 会把 cargo stderr 包成 NativeCommandError,别被表象误导)。

---

## 文件结构(全量地图)

```
src/readiness.rs        新:model_present(dir) 纯逻辑(TDD)
src/download.rs         新:download_file(url,dest,on_progress,should_cancel) blocking streaming
src/audio.rs            改:+ has_input_device()
src/config.rs           改:+ ModelConfig + Config.model 字段;config.example.toml + [model]
src/lib.rs              改:+ pub mod readiness; pub mod download;
src-tauri/src/wizard.rs 新:WizardState/DepCheck + 命令 wizard_state/check_dependencies/
                        download_model/cancel_download/import_model/finish_wizard/open_external
src-tauri/src/lib.rs    改:start_engine(once 守卫)+ setup 就绪分流 + 挂 wizard 命令 + manage 状态
src-tauri/Cargo.toml    改:+ winreg;windows features + Win32_System_LibraryLoader
src-ui/main.tsx         改:invoke wizard_state 分流 <App>/<Wizard>
src-ui/wizard/types.ts  新:WizardState/DepCheck/DlProgress TS 接口
src-ui/wizard/api.ts    新:向导命令 invoke 封装 + bt:dl-progress 监听
src-ui/wizard/Wizard.tsx          新:5 步壳
src-ui/wizard/WelcomeStep.tsx     新
src-ui/wizard/DepsStep.tsx        新
src-ui/wizard/LlmStep.tsx         新(复用 settings 的 testLlm + 小件)
src-ui/wizard/DownloadStep.tsx    新
src-ui/wizard/DoneStep.tsx        新
```

不动:engine.rs、hotkey/*、overlay.tsx、G4 settings/* 与 App.tsx(仅被 main.tsx 条件渲染)。

---

### Task 1: 核心 readiness.rs —— model_present(TDD)

**Files:**
- Create: `src/readiness.rs`
- Modify: `src/lib.rs`(加 `pub mod readiness;`)

- [ ] **Step 1: 写失败测试**(新建 `src/readiness.rs`,先只放测试 + 空函数签名会编译失败 → 用完整文件一步到位更顺;此处按 TDD 先写测试)

先创建 `src/readiness.rs` 内容如下(函数 + 测试一起):

```rust
//! 就绪检测:语音识别模型文件是否齐全。

use std::path::Path;

/// 模型是否齐全:`model_dir` 下 `model.onnx` 与 `tokens.txt` 都存在且非空。
pub fn model_present(model_dir: &Path) -> bool {
    let ok = |name: &str| {
        std::fs::metadata(model_dir.join(name))
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
    };
    ok("model.onnx") && ok("tokens.txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_only_when_both_files_nonempty() {
        let dir = std::env::temp_dir().join(format!("bytype-g5-readiness-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 都没有 → false
        assert!(!model_present(&dir));
        // 只有 model.onnx → false
        std::fs::write(dir.join("model.onnx"), b"x").unwrap();
        assert!(!model_present(&dir));
        // 两个都在且非空 → true
        std::fs::write(dir.join("tokens.txt"), b"y").unwrap();
        assert!(model_present(&dir));
        // tokens.txt 为空 → false
        std::fs::write(dir.join("tokens.txt"), b"").unwrap();
        assert!(!model_present(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: 在 `src/lib.rs` 注册模块**

在 `src/lib.rs` 现有 `pub mod sound;` 一行附近加:

```rust
pub mod download;
pub mod readiness;
```

(`download` 模块 Task 3 才创建;若本任务先加会编译失败,故本步**只加 `pub mod readiness;`**,`pub mod download;` 留到 Task 3 加。)

实际本步只加:

```rust
pub mod readiness;
```

- [ ] **Step 3: 跑测试**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib readiness:: ; "EXIT=$LASTEXITCODE"
```

预期:`1 passed`,EXIT=0。

- [ ] **Step 4: 全量无回归**

```powershell
cargo test -p voice-input --lib ; "EXIT=$LASTEXITCODE"
```

预期:`51 passed`(原 50 + 新 1),EXIT=0。

- [ ] **Step 5: Commit**

```powershell
git add src/readiness.rs src/lib.rs
git commit -m "feat(g5): 核心 readiness::model_present(模型齐全判定,TDD)"
```

---

### Task 2: 核心 config `[model]` 段(TDD)

**Files:**
- Modify: `src/config.rs`、`config.example.toml`
- Test: `src/config.rs`

- [ ] **Step 1: 写失败测试**(加到 `src/config.rs` 的 `mod tests` 末尾)

```rust
    #[test]
    fn model_section_defaults_and_override() {
        // 默认:URL 指向 hf-mirror
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.model.model_url.contains("hf-mirror.com"));
        assert!(cfg.model.tokens_url.contains("tokens.txt"));
        // 覆盖
        let cfg: Config =
            toml::from_str("[model]\nmodel_url = \"https://x/m.onnx\"\n").unwrap();
        assert_eq!(cfg.model.model_url, "https://x/m.onnx");
        // 往返
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, cfg);
    }
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib config::model_section ; "EXIT=$LASTEXITCODE"
```

预期:编译错误(`Config` 无 `model` 字段)。

- [ ] **Step 3: 实现**

3a. `src/config.rs` 的 `Config` 结构体加字段(在 `pub sound: SoundConfig,` 之后):

```rust
    pub model: ModelConfig,
```

3b. `Config` 的 `Default` impl 里加(在 `sound: SoundConfig::default(),` 之后):

```rust
            model: ModelConfig::default(),
```

3c. 在 `SoundConfig` 的 Default impl 之后,新增结构体与默认值:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ModelConfig {
    /// 语音识别模型(int8 onnx)下载源;下载后存为 model.onnx。
    pub model_url: String,
    /// tokens.txt 下载源。
    pub tokens_url: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            model_url: "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx".into(),
            tokens_url: "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt".into(),
        }
    }
}
```

- [ ] **Step 4: `config.example.toml` 追加**(文件末尾)

```toml

[model]
# 语音识别模型下载源(首启向导用;留空走内置默认 hf-mirror 镜像)。下载的 int8 模型存为 model.onnx。
model_url  = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx"
tokens_url = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt"
```

- [ ] **Step 5: 跑测试 + 全量**

```powershell
cargo test -p voice-input --lib ; "EXIT=$LASTEXITCODE"
```

预期:`52 passed`,EXIT=0。

- [ ] **Step 6: Commit**

```powershell
git add src/config.rs config.example.toml
git commit -m "feat(g5): config 加 [model] 段(下载源 URL,默认 hf-mirror)"
```

---

### Task 3: 核心 download.rs —— 带进度回调的流式下载

**Files:**
- Create: `src/download.rs`
- Modify: `src/lib.rs`(加 `pub mod download;`)

不写单测(真实网络,真机验证);验证 = 编译 + 全量测试无回归。

- [ ] **Step 1: 新建 `src/download.rs`**(完整文件)

```rust
//! 带进度回调的文件下载(reqwest blocking streaming)。
//! 供首启向导下载模型用;下载逻辑放核心、进度与取消由调用方注入(与 GUI 解耦)。

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result};

/// 下载 `url` 到 `dest`,边下边回调 `on_progress(received, total)`(total=0 表示未知)。
/// 每当 `should_cancel()` 返回 true 立即中止并返回错误(调用方负责删残文件)。
pub fn download_file(
    url: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, u64),
    should_cancel: impl Fn() -> bool,
) -> Result<()> {
    // 大文件:不设总超时(只在建连阶段用默认行为)。
    let client = reqwest::blocking::Client::builder().build()?;
    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("请求失败: {url}"))?
        .error_for_status()
        .with_context(|| format!("下载响应错误: {url}"))?;
    let total = resp.content_length().unwrap_or(0);

    let mut file =
        std::fs::File::create(dest).with_context(|| format!("创建文件失败: {}", dest.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    loop {
        if should_cancel() {
            anyhow::bail!("已取消");
        }
        let n = resp.read(&mut buf).context("读取下载流失败")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("写入文件失败")?;
        received += n as u64;
        on_progress(received, total);
    }
    file.flush().ok();
    Ok(())
}
```

- [ ] **Step 2: 注册模块** —— `src/lib.rs` 加(Task 1 已加 `pub mod readiness;`,现在补):

```rust
pub mod download;
```

- [ ] **Step 3: 编译 + 全量测试无回归**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib ; "EXIT=$LASTEXITCODE"
```

预期:`52 passed`,EXIT=0。

- [ ] **Step 4: Commit**

```powershell
git add src/download.rs src/lib.rs
git commit -m "feat(g5): 核心 download::download_file(流式下载+进度回调+取消)"
```

---

### Task 4: 核心 audio::has_input_device

**Files:**
- Modify: `src/audio.rs`

不写单测(依赖真实音频设备);验证 = 编译。

- [ ] **Step 1: 在 `src/audio.rs` 末尾(`push_mono` 之后)加函数**

```rust
/// 是否存在可用的麦克风输入设备(供首启依赖检测用)。
pub fn has_input_device() -> bool {
    cpal::default_host().default_input_device().is_some()
}
```

(`cpal::traits::HostTrait` 已在文件顶部 `use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};` 引入,无需新增。)

- [ ] **Step 2: 编译**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build -p voice-input ; "EXIT=$LASTEXITCODE"
```

预期:EXIT=0。

- [ ] **Step 3: Commit**

```powershell
git add src/audio.rs
git commit -m "feat(g5): 核心 audio::has_input_device(麦克风设备检测)"
```

---

### Task 5: src-tauri wizard.rs —— 就绪状态 + 依赖检测 + open_external

**Files:**
- Create: `src-tauri/src/wizard.rs`
- Modify: `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`

- [ ] **Step 1: `src-tauri/Cargo.toml` 加依赖**

`[dependencies]` 里 `serde = { version = "1", features = ["derive"] }` 下一行加:

```toml
winreg = "0.10"
```

`[target.'cfg(windows)'.dependencies.windows]` 的 features 数组改为:

```toml
features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_System_LibraryLoader"]
```

- [ ] **Step 2: 新建 `src-tauri/src/wizard.rs`**(完整文件)

```rust
//! 首启向导后端:就绪状态、依赖检测、模型下载/导入。

use std::path::PathBuf;

use serde::Serialize;
use voice_input::config::Config;

#[derive(Serialize)]
pub struct WizardState {
    pub ready: bool,
    pub config_exists: bool,
    pub model_present: bool,
    pub model_dir: String,
}

#[derive(Serialize)]
pub struct DepCheck {
    pub key: String,
    pub label: String,
    pub status: String, // "ok" | "bad" | "warn"
    pub detail: String,
    pub fix_url: Option<String>,
}

/// exe 所在目录。
pub(crate) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// 向导阶段模型目录:有 config 用其解析后的 asr.model_dir,否则 exe 目录旁 models/sensevoice。
pub(crate) fn wizard_model_dir() -> PathBuf {
    if let Ok(cfg) = Config::load_resolved() {
        return PathBuf::from(cfg.asr.model_dir);
    }
    exe_dir()
        .map(|d| d.join("models").join("sensevoice"))
        .unwrap_or_else(|| PathBuf::from("models/sensevoice"))
}

/// 就绪状态:config 存在 + 模型齐全。供前端分流。
#[tauri::command]
pub fn wizard_state() -> WizardState {
    let config_exists = voice_input::config::find_config_file().is_ok();
    let dir = wizard_model_dir();
    let model_present = voice_input::readiness::model_present(&dir);
    WizardState {
        ready: config_exists && model_present,
        config_exists,
        model_present,
        model_dir: dir.display().to_string(),
    }
}

/// 4 项依赖检测。
#[tauri::command]
pub fn check_dependencies() -> Vec<DepCheck> {
    vec![vcredist_check(), core_dll_check(), mic_device_check(), mic_privacy_check()]
}

/// 用资源管理器打开 URL(支持 http(s) 与 ms-settings: 协议)。
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- 依赖检测实现 ----

fn vcredist_check() -> DepCheck {
    let ok = vcredist_installed();
    DepCheck {
        key: "vcredist".into(),
        label: "VC++ 运行时".into(),
        status: if ok { "ok" } else { "bad" }.into(),
        detail: if ok {
            "Microsoft Visual C++ Redistributable 已安装".into()
        } else {
            "未检测到,onnxruntime 将无法加载".into()
        },
        fix_url: if ok { None } else { Some("https://aka.ms/vs/17/release/vc_redist.x64.exe".into()) },
    }
}

#[cfg(windows)]
fn vcredist_installed() -> bool {
    use windows::core::w;
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    unsafe { LoadLibraryW(w!("vcruntime140.dll")).is_ok() || LoadLibraryW(w!("vcruntime140_1.dll")).is_ok() }
}
#[cfg(not(windows))]
fn vcredist_installed() -> bool {
    true
}

fn core_dll_check() -> DepCheck {
    let ok = match exe_dir() {
        Some(d) => ["onnxruntime.dll", "sherpa-onnx-c-api.dll"].iter().all(|n| d.join(n).is_file()),
        None => false,
    };
    DepCheck {
        key: "core_dll".into(),
        label: "核心组件".into(),
        status: if ok { "ok" } else { "bad" }.into(),
        detail: if ok {
            "onnxruntime.dll / sherpa-onnx-c-api.dll 完整".into()
        } else {
            "缺少 onnxruntime.dll 或 sherpa-onnx-c-api.dll".into()
        },
        fix_url: None,
    }
}

fn mic_device_check() -> DepCheck {
    let ok = voice_input::audio::has_input_device();
    DepCheck {
        key: "mic_device".into(),
        label: "麦克风设备".into(),
        status: if ok { "ok" } else { "warn" }.into(),
        detail: if ok { "检测到可用录音设备".into() } else { "未检测到录音设备".into() },
        fix_url: None,
    }
}

fn mic_privacy_check() -> DepCheck {
    match mic_privacy_allowed() {
        Some(false) => DepCheck {
            key: "mic_privacy".into(),
            label: "麦克风权限".into(),
            status: "warn".into(),
            detail: "Windows 隐私设置可能禁止桌面应用访问麦克风".into(),
            fix_url: Some("ms-settings:privacy-microphone".into()),
        },
        // true 或 未知(None)都不报警
        _ => DepCheck {
            key: "mic_privacy".into(),
            label: "麦克风权限".into(),
            status: "ok".into(),
            detail: "已允许桌面应用访问麦克风".into(),
            fix_url: None,
        },
    }
}

/// 读注册表判断麦克风隐私权限;读不到返回 None(未知,不报警)。
#[cfg(windows)]
fn mic_privacy_allowed() -> Option<bool> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone")
        .ok()?;
    let val: String = key.get_value("Value").ok()?;
    Some(val == "Allow")
}
#[cfg(not(windows))]
fn mic_privacy_allowed() -> Option<bool> {
    None
}
```

- [ ] **Step 3: `src-tauri/src/lib.rs` 注册**

文件顶部 `mod settings;` 下一行加:

```rust
mod wizard;
```

`invoke_handler` 数组在 `settings::open_config_dir` 之后加(注意前一行补逗号):

```rust
            settings::open_config_dir,
            wizard::wizard_state,
            wizard::check_dependencies,
            wizard::open_external
```

- [ ] **Step 4: 编译**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build -p bytype ; "EXIT=$LASTEXITCODE"
```

预期:EXIT=0(winreg 已由 autostart 传递引入,显式声明同版本不冲突)。

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/wizard.rs src-tauri/src/lib.rs Cargo.lock
git commit -m "feat(g5): wizard 后端——就绪状态 + 4 项依赖检测 + open_external"
```

---

### Task 6: src-tauri wizard.rs —— 模型下载 / 取消 / 导入

**Files:**
- Modify: `src-tauri/src/wizard.rs`、`src-tauri/src/lib.rs`

- [ ] **Step 1: `wizard.rs` 顶部 use 补充**

把文件顶部的 use 区改为(新增 atomic 与 Arc):

```rust
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use voice_input::config::Config;
```

- [ ] **Step 2: `wizard.rs` 末尾追加下载相关**

```rust
/// 下载取消标志(由 lib.rs manage)。
#[derive(Default)]
pub struct DownloadCancel(pub Arc<AtomicBool>);

#[derive(Clone, Serialize)]
struct DlProgress {
    file: String,
    received: u64,
    total: u64,
}

/// 下载模型(tokens + model)到向导模型目录,emit `bt:dl-progress`。
#[tauri::command]
pub async fn download_model(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, DownloadCancel>,
) -> Result<(), String> {
    let flag = cancel.0.clone();
    flag.store(false, Ordering::SeqCst);
    let dir = wizard_model_dir();
    // 无 config 时用默认配置的 URL(仍是 hf-mirror 默认)。
    let cfg = Config::load_resolved().unwrap_or_default();
    let (model_url, tokens_url) = (cfg.model.model_url.clone(), cfg.model.tokens_url.clone());

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // 先 tokens(小、快,能早暴露 URL 错误),再 model(大),存为 model.onnx
        dl_one(&app, &flag, &tokens_url, &dir.join("tokens.txt"), "tokens")?;
        dl_one(&app, &flag, &model_url, &dir.join("model.onnx"), "model")?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn dl_one(
    app: &tauri::AppHandle,
    flag: &Arc<AtomicBool>,
    url: &str,
    dest: &Path,
    file_tag: &str,
) -> Result<(), String> {
    let part = dest.with_extension("part");
    let app2 = app.clone();
    let tag = file_tag.to_string();
    let res = voice_input::download::download_file(
        url,
        &part,
        |received, total| {
            let _ = app2.emit("bt:dl-progress", DlProgress { file: tag.clone(), received, total });
        },
        || flag.load(Ordering::SeqCst),
    );
    match res {
        Ok(()) => {
            // 校验大小:model ≥ 100MB,tokens ≥ 1KB
            let min: u64 = if file_tag == "model" { 100 * 1024 * 1024 } else { 1024 };
            let size = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
            if size < min {
                std::fs::remove_file(&part).ok();
                return Err(format!("{file_tag} 文件过小({size} 字节),可能下载不完整"));
            }
            std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            std::fs::remove_file(&part).ok();
            Err(e.to_string())
        }
    }
}

/// 取消进行中的下载。
#[tauri::command]
pub fn cancel_download(cancel: tauri::State<DownloadCancel>) {
    cancel.0.store(true, Ordering::SeqCst);
}

/// 导入用户本地已下好的模型文件(校验后复制到模型目录)。
#[tauri::command]
pub fn import_model(model_path: String, tokens_path: String) -> Result<(), String> {
    let dir = wizard_model_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let m = Path::new(&model_path);
    let t = Path::new(&tokens_path);
    let msize = std::fs::metadata(m).map(|x| x.len()).unwrap_or(0);
    if msize < 100 * 1024 * 1024 {
        return Err("所选模型文件过小,不像有效的 model.onnx".into());
    }
    if std::fs::metadata(t).map(|x| x.len()).unwrap_or(0) == 0 {
        return Err("所选 tokens 文件为空或不存在".into());
    }
    std::fs::copy(m, dir.join("model.onnx")).map_err(|e| e.to_string())?;
    std::fs::copy(t, dir.join("tokens.txt")).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: `lib.rs` manage + 挂命令**

`.manage(ControlSlot::default())` 下一行加:

```rust
        .manage(wizard::DownloadCancel::default())
```

`invoke_handler` 数组 `wizard::open_external` 之后加(前一行补逗号):

```rust
            wizard::open_external,
            wizard::download_model,
            wizard::cancel_download,
            wizard::import_model
```

- [ ] **Step 4: 编译**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build -p bytype ; "EXIT=$LASTEXITCODE"
```

预期:EXIT=0。

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/wizard.rs src-tauri/src/lib.rs
git commit -m "feat(g5): wizard 后端——模型流式下载(进度/取消)+ 本地导入"
```

---

### Task 7: src-tauri lib.rs —— start_engine(once)+ 就绪分流 + finish_wizard

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 顶部 use 与状态**

文件顶部 use 区把 `use std::sync::{Arc, Mutex};` 改为:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
```

在 `ControlSlot` 结构体定义之后加:

```rust
/// 引擎是否已启动(防止 setup 与 finish_wizard 重复启动钩子/录音器)。
#[derive(Default)]
struct EngineStarted(AtomicBool);
```

- [ ] **Step 2: 新增 `start_engine` 与 `finish_wizard`**(加在 `apply_no_activate` 的 `#[cfg(not(windows))]` 版本之后、`pub fn run()` 之前)

```rust
/// 启动听写引擎(只启动一次)。就绪 setup 与向导完成都经此入口。
fn start_engine(app: &tauri::AppHandle) {
    let started = app.state::<EngineStarted>();
    if started.0.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return; // 已启动过,忽略
    }
    let app_handle = app.clone();
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
}

/// 向导「完成」:用向导填的 LLM 值更新(或创建)config.toml,然后当场启动引擎。
#[tauri::command]
fn finish_wizard(
    app: tauri::AppHandle,
    llm: voice_input::config::LlmConfig,
) -> Result<(), String> {
    // 读现有(无则默认),只覆盖 [llm],保留其它字段。
    let (mut cfg, path) = match Config::load_raw() {
        Ok((c, p)) => (c, p),
        Err(_) => {
            let dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .ok_or("无法确定程序目录")?;
            (Config::default(), dir.join("config.toml"))
        }
    };
    let mut llm = llm;
    if llm.api_key.trim().is_empty() {
        llm.enabled = false; // 没填 key 就不开 LLM,避免每次失败请求
    }
    cfg.llm = llm;
    cfg.save_to(&path).map_err(|e| format!("{e:#}"))?;
    start_engine(&app);
    // 向导完成 → 隐藏主窗口,转入托盘后台运行。
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    Ok(())
}
```

(`get_webview_window` 来自已 `use` 的 `tauri::Manager`。)

- [ ] **Step 3: `run()` 里 manage + 挂 finish_wizard**

`.manage(wizard::DownloadCancel::default())` 下一行加:

```rust
        .manage(EngineStarted::default())
```

`invoke_handler` 数组 `wizard::import_model` 之后加(前一行补逗号):

```rust
            wizard::import_model,
            finish_wizard
```

- [ ] **Step 4: setup 就绪分流**

把 setup 里现有的「读配置并在后台线程跑引擎」整段(从 `// 读配置并在后台线程跑引擎` 注释到其 `match Config::load_resolved() { ... }` 结束)替换为:

```rust
            // 就绪分流:就绪→启动引擎(主窗口保持隐藏到托盘);未就绪→显示主窗口跑首启向导。
            let handle = app.handle().clone();
            if wizard::wizard_state().ready {
                start_engine(&handle);
            } else if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
```

- [ ] **Step 5: 编译**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build -p bytype ; "EXIT=$LASTEXITCODE"
```

预期:EXIT=0。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat(g5): start_engine once 守卫 + setup 就绪分流 + finish_wizard 命令"
```

---

### Task 8: 前端基座 —— dialog 插件 + types/api + main.tsx 分流

**Files:**
- Modify: `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/main.json`、`package.json`(npm)、`src-ui/main.tsx`
- Create: `src-ui/wizard/types.ts`、`src-ui/wizard/api.ts`

- [ ] **Step 1: 接 dialog 插件(本地文件导入要用文件对话框拿真实路径)**

`src-tauri/Cargo.toml` 的 `[dependencies]`,`winreg = "0.10"` 下一行加:

```toml
tauri-plugin-dialog = "2"
```

`src-tauri/src/lib.rs` builder 链,在 autostart 的 `.plugin(...)` 之后加:

```rust
        .plugin(tauri_plugin_dialog::init())
```

`src-tauri/capabilities/main.json` 的 permissions 改为:

```json
  "permissions": [
    "autostart:default",
    "dialog:default"
  ]
```

仓库根:

```powershell
npm install @tauri-apps/plugin-dialog
```

- [ ] **Step 2: `src-ui/wizard/types.ts`**(完整文件)

```ts
export interface WizardState {
  ready: boolean;
  config_exists: boolean;
  model_present: boolean;
  model_dir: string;
}

export interface DepCheck {
  key: string;
  label: string;
  status: "ok" | "bad" | "warn";
  detail: string;
  fix_url: string | null;
}

export interface DlProgress {
  file: string; // "tokens" | "model"
  received: number;
  total: number;
}
```

- [ ] **Step 3: `src-ui/wizard/api.ts`**(完整文件)

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DepCheck, DlProgress, WizardState } from "./types";
import type { LlmConfig } from "../settings/types";

export const wizardState = () => invoke<WizardState>("wizard_state");
export const checkDependencies = () => invoke<DepCheck[]>("check_dependencies");
export const downloadModel = () => invoke<void>("download_model");
export const cancelDownload = () => invoke<void>("cancel_download");
// Tauri 2 命令参数:JS 端用 camelCase,自动映射到 Rust 的 snake_case(model_path/tokens_path)。
export const importModel = (modelPath: string, tokensPath: string) =>
  invoke<void>("import_model", { modelPath, tokensPath });
export const finishWizard = (llm: LlmConfig) => invoke<void>("finish_wizard", { llm });
export const openExternal = (url: string) => invoke<void>("open_external", { url });

export const onDlProgress = (cb: (p: DlProgress) => void): Promise<UnlistenFn> =>
  listen<DlProgress>("bt:dl-progress", (e) => cb(e.payload));
```

- [ ] **Step 4: 重写 `src-ui/main.tsx`**(完整文件)

```tsx
import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Wizard from "./wizard/Wizard";
import { wizardState } from "./wizard/api";
import type { WizardState } from "./wizard/types";
import "./index.css";
import { initTheme } from "./settings/theme";

initTheme();

function Root() {
  const [st, setSt] = useState<WizardState | null>(null);
  useEffect(() => {
    // 出错也不卡死:当作已就绪,进设置界面(用户至少能看/改配置)。
    wizardState()
      .then(setSt)
      .catch(() => setSt({ ready: true, config_exists: true, model_present: true, model_dir: "" }));
  }, []);

  if (st === null) {
    return (
      <div className="h-screen flex items-center justify-center text-neutral-400 text-sm dark:bg-neutral-900">
        启动中…
      </div>
    );
  }
  return st.ready ? <App /> : <Wizard initial={st} />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
```

- [ ] **Step 5: 类型检查 + 构建**

`Wizard` 组件 Task 9 才建,本步 `tsc` 会因找不到 `./wizard/Wizard` 报错。**因此 Step 5 推迟到 Task 9 一起验证**;本任务只做 Step 1–4 的文件落地与 npm 安装,不单独跑 tsc。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/capabilities/main.json package.json package-lock.json Cargo.lock src-ui/wizard/types.ts src-ui/wizard/api.ts src-ui/main.tsx
git commit -m "feat(g5): 前端基座(dialog 插件 + wizard types/api + main.tsx 就绪分流)"
```

---

### Task 9: 前端向导壳 + 五步组件

**Files:**
- Create: `src-ui/wizard/Wizard.tsx`、`WelcomeStep.tsx`、`DepsStep.tsx`、`LlmStep.tsx`、`DownloadStep.tsx`、`DoneStep.tsx`

- [ ] **Step 1: `src-ui/wizard/WelcomeStep.tsx`**

```tsx
export default function WelcomeStep() {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-3 h-full">
      <div className="w-14 h-14 rounded-2xl bg-blue-500" />
      <h2 className="text-lg font-semibold">欢迎使用 ByType</h2>
      <p className="max-w-md text-sm text-neutral-500 dark:text-neutral-400">
        按住热键说话,松手即把文字输入到任何应用。首次使用需几步准备:检测运行环境、填写 LLM
        中转站(可选)、下载语音识别模型(约 228MB)。
      </p>
    </div>
  );
}
```

- [ ] **Step 2: `src-ui/wizard/DepsStep.tsx`**

```tsx
import { useEffect, useState } from "react";
import { checkDependencies, openExternal } from "./api";
import type { DepCheck } from "./types";

export default function DepsStep({ onStatus }: { onStatus: (ok: boolean) => void }) {
  const [deps, setDeps] = useState<DepCheck[] | null>(null);

  const run = () => {
    setDeps(null);
    checkDependencies()
      .then((d) => {
        setDeps(d);
        onStatus(d.every((x) => x.status !== "bad")); // 无致命项即可继续
      })
      .catch(() => onStatus(false));
  };
  useEffect(run, []);

  const icon = (s: string) => (s === "ok" ? "✓" : s === "bad" ? "✕" : "!");
  const color = (s: string) =>
    s === "ok" ? "text-emerald-600" : s === "bad" ? "text-red-600" : "text-amber-600";

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">运行环境检测</h2>
      {deps === null && <p className="text-sm text-neutral-400">检测中…</p>}
      {deps?.map((d) => (
        <div
          key={d.key}
          className="flex items-start gap-3 border border-neutral-200 dark:border-neutral-700 rounded-lg px-3 py-2"
        >
          <span className={`${color(d.status)} font-bold`}>{icon(d.status)}</span>
          <div className="flex-1 min-w-0">
            <div className="text-sm">{d.label}</div>
            <div className="text-xs text-neutral-400">{d.detail}</div>
            {d.fix_url && (
              <button
                onClick={() => openExternal(d.fix_url!)}
                className="text-xs text-blue-600 hover:underline mt-0.5"
              >
                {d.fix_url.startsWith("ms-settings:") ? "打开 Windows 设置" : "下载安装"}
              </button>
            )}
          </div>
        </div>
      ))}
      {deps && (
        <button onClick={run} className="self-start text-xs text-blue-600 hover:underline">
          重新检测
        </button>
      )}
      {deps?.some((d) => d.status === "bad") && (
        <p className="text-xs text-red-600">存在致命缺失,修复后点「重新检测」才能继续。</p>
      )}
    </div>
  );
}
```

- [ ] **Step 3: `src-ui/wizard/LlmStep.tsx`**

```tsx
import { useState } from "react";
import { testLlm } from "../settings/api";
import type { LlmConfig } from "../settings/types";

export default function LlmStep({
  llm,
  setLlm,
}: {
  llm: LlmConfig;
  setLlm: (l: LlmConfig) => void;
}) {
  const [show, setShow] = useState(false);
  const [test, setTest] = useState<string | null>(null);
  const set = (p: Partial<LlmConfig>) => setLlm({ ...llm, ...p });
  const cls =
    "border border-neutral-300 dark:border-neutral-700 dark:bg-neutral-800 rounded-md px-2.5 py-1.5 text-sm";

  const runTest = async () => {
    setTest("测试中…");
    try {
      const r = await testLlm({ ...llm, enabled: true });
      setTest(`✓ ${r.latency_ms}ms · ${r.reply.slice(0, 40)}`);
    } catch (e) {
      setTest(`✗ ${String(e).slice(0, 80)}`);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">LLM 中转站(用于整理/翻译,可跳过)</h2>
      <label className="text-sm flex flex-col gap-1">
        接口地址
        <input
          className={cls}
          value={llm.base_url}
          placeholder="https://example.com/v1"
          onChange={(e) => set({ base_url: e.target.value })}
        />
      </label>
      <label className="text-sm flex flex-col gap-1">
        API Key
        <span className="flex items-center gap-1.5">
          <input
            type={show ? "text" : "password"}
            className={`flex-1 ${cls}`}
            value={llm.api_key}
            onChange={(e) => set({ api_key: e.target.value })}
          />
          <button type="button" onClick={() => setShow(!show)} className="text-neutral-400">
            👁
          </button>
        </span>
      </label>
      <label className="text-sm flex flex-col gap-1">
        模型
        <input
          className={cls}
          value={llm.model}
          placeholder="deepseek-v4-flash"
          onChange={(e) => set({ model: e.target.value })}
        />
      </label>
      <div className="flex items-center gap-3">
        <button
          onClick={runTest}
          className="px-3 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm"
        >
          ⚡ 测试连接
        </button>
        {test && <span className="text-xs text-neutral-500">{test}</span>}
      </div>
      <p className="text-xs text-neutral-400">不填也能用:听写直接输出原始识别文本,日后可在设置里补。</p>
    </div>
  );
}
```

- [ ] **Step 4: `src-ui/wizard/DownloadStep.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cancelDownload, downloadModel, importModel, onDlProgress } from "./api";
import type { DlProgress } from "./types";

export default function DownloadStep({
  modelReady,
  onReady,
}: {
  modelReady: boolean;
  onReady: () => void;
}) {
  const [phase, setPhase] = useState<"idle" | "downloading">("idle");
  const [prog, setProg] = useState<DlProgress | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const unlisten = useRef<(() => void) | null>(null);

  useEffect(() => {
    onDlProgress(setProg).then((u) => (unlisten.current = u));
    return () => unlisten.current?.();
  }, []);

  const start = async () => {
    setErr(null);
    setPhase("downloading");
    setProg(null);
    try {
      await downloadModel();
      onReady();
    } catch (e) {
      setErr(String(e));
    } finally {
      setPhase("idle");
    }
  };

  const doImport = async () => {
    setErr(null);
    const m = await open({ title: "选择 model.onnx(int8)", filters: [{ name: "ONNX", extensions: ["onnx"] }] });
    if (typeof m !== "string") return;
    const t = await open({ title: "选择 tokens.txt", filters: [{ name: "Text", extensions: ["txt"] }] });
    if (typeof t !== "string") return;
    try {
      await importModel(m, t);
      onReady();
    } catch (e) {
      setErr(String(e));
    }
  };

  const pct = prog && prog.total > 0 ? Math.round((prog.received / prog.total) * 100) : 0;
  const mb = (n: number) => (n / 1024 / 1024).toFixed(1);

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold">下载语音识别模型</h2>
      {modelReady ? (
        <p className="text-sm text-emerald-600">✓ 模型已就绪,点「下一步」继续。</p>
      ) : (
        <>
          <p className="text-xs text-neutral-400">SenseVoice int8 · 约 228MB · 源:hf-mirror.com(可在设置改)</p>
          {phase === "downloading" && (
            <>
              <div className="h-2 bg-neutral-200 dark:bg-neutral-700 rounded-full overflow-hidden">
                <div className="h-full bg-blue-500" style={{ width: `${pct}%` }} />
              </div>
              <div className="flex justify-between text-xs text-neutral-500">
                <span>
                  {prog?.file === "model" ? "模型" : "词表"} · {prog ? mb(prog.received) : "0"} /{" "}
                  {prog && prog.total ? mb(prog.total) : "?"} MB（{pct}%）
                </span>
                <button onClick={() => cancelDownload()} className="text-blue-600">
                  取消
                </button>
              </div>
            </>
          )}
          {phase === "idle" && (
            <button
              onClick={start}
              className="self-start px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600"
            >
              开始下载
            </button>
          )}
          {err && <p className="text-xs text-red-600">下载失败:{err}</p>}
          <div className="border-t border-dashed border-neutral-200 dark:border-neutral-700 pt-3 text-xs text-neutral-400">
            下载不动?
            <button onClick={doImport} className="text-blue-600 hover:underline">
              改用本地文件导入
            </button>
            (选已下好的 model.onnx + tokens.txt,校验后放入)。
          </div>
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 5: `src-ui/wizard/DoneStep.tsx`**

```tsx
export default function DoneStep() {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-3 h-full">
      <div className="text-4xl text-emerald-600">✓</div>
      <h2 className="text-lg font-semibold">一切就绪</h2>
      <p className="max-w-md text-sm text-neutral-500 dark:text-neutral-400">
        按住 <b>左 Win</b> 说话即可输入。ByType 将常驻托盘后台运行,可从托盘打开设置。点「完成」开始。
      </p>
    </div>
  );
}
```

- [ ] **Step 6: `src-ui/wizard/Wizard.tsx`**(壳)

```tsx
import { useEffect, useState } from "react";
import { finishWizard } from "./api";
import { getConfig } from "../settings/api";
import type { LlmConfig } from "../settings/types";
import type { WizardState } from "./types";
import WelcomeStep from "./WelcomeStep";
import DepsStep from "./DepsStep";
import LlmStep from "./LlmStep";
import DownloadStep from "./DownloadStep";
import DoneStep from "./DoneStep";

const STEPS = ["欢迎", "依赖检测", "LLM 配置", "下载模型", "完成"];

export default function Wizard({ initial }: { initial: WizardState }) {
  const [step, setStep] = useState(0);
  const [llm, setLlm] = useState<LlmConfig | null>(null);
  const [depsOk, setDepsOk] = useState(false);
  const [modelReady, setModelReady] = useState(initial.model_present);
  const [finishing, setFinishing] = useState(false);
  const [finishErr, setFinishErr] = useState<string | null>(null);

  useEffect(() => {
    // 预填现有 LLM(config 不存在时后端返回默认),避免覆盖已有配置。
    getConfig()
      .then((r) => setLlm(r.config.llm))
      .catch(() => {});
  }, []);

  const canNext = step === 1 ? depsOk : step === 3 ? modelReady : true;
  const next = () => setStep((s) => Math.min(STEPS.length - 1, s + 1));
  const prev = () => setStep((s) => Math.max(0, s - 1));

  const onFinish = async () => {
    if (!llm) return;
    setFinishing(true);
    setFinishErr(null);
    try {
      await finishWizard(llm); // 成功后后端隐藏主窗口
    } catch (e) {
      setFinishErr(String(e));
      setFinishing(false);
    }
  };

  return (
    <div className="h-screen flex flex-col bg-white text-neutral-800 dark:bg-neutral-900 dark:text-neutral-200">
      <div className="flex gap-1 px-4 py-3 text-xs border-b border-neutral-200 dark:border-neutral-700 bg-neutral-50 dark:bg-neutral-950">
        {STEPS.map((s, i) => (
          <span
            key={s}
            className={`px-2 ${
              i === step
                ? "text-blue-500 font-semibold"
                : i < step
                ? "text-emerald-600"
                : "text-neutral-400"
            }`}
          >
            {i < step ? "✓ " : `${i + 1} `}
            {s}
          </span>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {step === 0 && <WelcomeStep />}
        {step === 1 && <DepsStep onStatus={setDepsOk} />}
        {step === 2 && llm && <LlmStep llm={llm} setLlm={setLlm} />}
        {step === 3 && <DownloadStep modelReady={modelReady} onReady={() => setModelReady(true)} />}
        {step === 4 && <DoneStep />}
      </div>
      <div className="flex-none border-t border-neutral-200 dark:border-neutral-700 px-4 py-2.5 flex items-center gap-3">
        {step > 0 && step < 4 && (
          <button
            onClick={prev}
            className="px-3.5 py-1.5 rounded-md border border-neutral-300 dark:border-neutral-700 text-sm"
          >
            ← 上一步
          </button>
        )}
        {finishErr && <span className="text-xs text-red-600">{finishErr}</span>}
        <span className="flex-1" />
        {step < 4 && (
          <button
            onClick={next}
            disabled={!canNext}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40"
          >
            下一步 →
          </button>
        )}
        {step === 4 && (
          <button
            onClick={onFinish}
            disabled={finishing || !llm}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40"
          >
            {finishing ? "启动中…" : "完成,开始使用"}
          </button>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 7: 类型检查 + 构建**(含 Task 8 的 main.tsx)

```powershell
npx tsc --noEmit ; "TSC=$LASTEXITCODE"
npm run build ; "BUILD=$LASTEXITCODE"
```

预期:TSC=0、BUILD=0。

- [ ] **Step 8: Commit**

```powershell
git add src-ui/wizard
git commit -m "feat(g5): 首启向导前端(壳 + 欢迎/依赖/LLM/下载/完成 五步)"
```

---

### Task 10: 真机端到端验证(人工,GUI dev)

**Files:** 无代码改动;发现问题修复并单独提交(`fix(g5): ...`)。

- [ ] **Step 1: 备份并制造「未就绪」**(PowerShell;先把现有 config/模型挪走)

```powershell
Rename-Item config.toml config.toml.bak -ErrorAction SilentlyContinue
Rename-Item models\sensevoice models\sensevoice.bak -ErrorAction SilentlyContinue
```

- [ ] **Step 2: 启动 GUI dev**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
npm run tauri dev
```

- [ ] **Step 3: 按清单验证**

1. 未就绪→主窗口自动弹**向导**(不是设置界面),引擎未启动(按住热键无反应)。
2. 欢迎页→依赖检测:VC++/核心 dll 应为 ✓(dev 下 target\debug 旁有 dll);麦克风设备/权限按真机显示;致命项缺失会挡「下一步」并能点修复链接。
3. LLM 步:填中转站→「测试连接」真实成功;或直接「下一步」跳过。
4. 下载步:「开始下载」→ 进度条/速度跳动→ 可「取消」(取消后 .part 应被删,重试可继续);完整下载后显示就绪。
   或「改用本地文件导入」→ 选 model.onnx.bak 目录里的两个文件 → 校验通过、就绪。
5. 完成步「完成,开始使用」→ 主窗口隐藏到托盘;**不重启进程**;按住左 Win 说话→出字(引擎当场启动成功)。
6. 检查 config.toml 已在程序目录/仓库根生成;LLM 跳过时 `llm.enabled=false`。
7. **二次启动**:再 `npm run tauri dev` → 直接就绪、不弹向导、托盘后台、热键可用。
8. 设置界面回归:托盘→设置正常打开(G4 七页无碍)。

- [ ] **Step 4: 还原现场**(验证完毕)

```powershell
# 若想保留向导下载的模型则跳过删除;否则还原备份:
Remove-Item models\sensevoice -Recurse -Force -ErrorAction SilentlyContinue
Rename-Item models\sensevoice.bak models\sensevoice -ErrorAction SilentlyContinue
Rename-Item config.toml.bak config.toml -ErrorAction SilentlyContinue
```

- [ ] **Step 5: 全量回归**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib ; "EXIT=$LASTEXITCODE"
```

预期:`52 passed`,EXIT=0。

---

## Plan 自检备注(写计划时已核对)

- **Spec 覆盖:** 就绪分流(T7)、5 步向导(T9)、依赖检测 4 项(T5)、模型下载+进度+取消+导入(T3/T6/T9)、
  hf-mirror 默认 URL 可配(T2)、复用主窗口分流(T8 main.tsx)、当场启动不重启(T7 start_engine once)、
  LLM 可跳过+空 key 关 LLM(T7 finish_wizard)、config 合并不覆盖其它字段(T7)、完成隐藏窗口(T7)。
- **类型一致:** Rust `WizardState{ready,config_exists,model_present,model_dir}` / `DepCheck{key,label,status,detail,fix_url}`
  / `DlProgress{file,received,total}`(T5/T6)与 TS(T8 types.ts)字段逐一对应;`finish_wizard(llm: LlmConfig)`
  接 G4 已有的 `LlmConfig`;`import_model(model_path,tokens_path)` ↔ JS `{modelPath,tokensPath}`(Tauri camelCase 映射)。
- **依赖装配:** winreg(T5)、windows `Win32_System_LibraryLoader`(T5)、tauri-plugin-dialog + @tauri-apps/plugin-dialog(T8)、
  capabilities/main.json 加 dialog:default(T8)。
- **无占位:** 每个代码步骤为完整文件或精确 diff。
- **已知风险(spec 标注):** 默认下载 URL 的文件名/路径以官方仓库为准,真机首次下载验证;不对则改 `[model]` 默认值。
- **判定 cargo 成功只认 `$LASTEXITCODE`**(G4 教训:PowerShell 把 cargo stderr 包成 NativeCommandError 会误导)。每个 src-tauri 任务都 `cargo build -p bytype` 实测。
