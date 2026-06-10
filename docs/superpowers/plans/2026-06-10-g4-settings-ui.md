# ByType G4 设置界面 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把主窗口做成完整设置界面:读写 config.toml(保存并重启生效)、LLM 一键测试、热键/词库/应用风格/各开关、开机自启、帮助与关于页。

**Architecture:** 核心 crate 给配置加 `Serialize` 并新增 `load_raw()`/`save_to()`(原始路径不解析,整文件写回);src-tauri 新模块 `settings.rs` 提供 `get_config / save_config / test_llm / restart_app / open_config_dir` 五个命令 + autostart 插件;前端 `src-ui/settings/` 七个页面组件 + 共享小件,App.tsx 重写为「侧边导航 + 脏检查 + 底部保存栏」的设置壳。

**Tech Stack:** Rust(serde/toml/reqwest blocking)、Tauri 2(tauri-plugin-autostart)、React 18 + TypeScript + Tailwind(无新 UI 库、无路由库)。

**规格:** `docs/superpowers/specs/2026-06-10-g4-settings-ui-design.md`(以下简称"spec")。

**构建环境(每个 cargo 命令前必须先跑,PowerShell):**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

前端检查用两条命令:`npx tsc --noEmit`(类型检查;`npm run build` 只走 vite/esbuild 不查类型)和 `npm run build`(产物构建)。都在仓库根目录跑,Node 已在 PATH。

---

## 文件结构(全量地图)

```
src/config.rs                 改:全部配置结构体 +Serialize +PartialEq;find_config_file 转 pub;
                              新增 Config::load_raw() / Config::save_to();load_resolved 改走 load_raw
src/corrector.rs              改:新增 pub fn test_connection(&LlmConfig) -> Result<(u64, String)>
src-tauri/src/settings.rs     新:5 个 tauri 命令 + GetConfigResp/TestOk 结构
src-tauri/src/lib.rs          改:mod settings;invoke_handler 挂 5 命令;注册 autostart 插件
src-tauri/Cargo.toml          改:+ tauri-plugin-autostart = "2"
src-tauri/capabilities/default.json  改:permissions + "autostart:default"
package.json                  改:+ @tauri-apps/plugin-autostart(npm install)
src-ui/vite-env.d.ts          新:/// <reference types="vite/client" />(让 .png 导入有类型)
src-ui/App.tsx                重写:设置壳(导航/加载/脏检查/保存栏/横幅)
src-ui/settings/types.ts      新:Config 等 TS 接口(snake_case 与 serde JSON 一致)+ PageProps
src-ui/settings/api.ts        新:invoke 封装
src-ui/settings/consts.ts     新:KEY_OPTIONS/LANG_OPTIONS/MODE_OPTIONS/keyLabel
src-ui/settings/validate.ts   新:hotkeyConflict
src-ui/settings/widgets.tsx   新:Section/Row/Toggle/TextInput/NumberInput/SelectBox/Collapsible
src-ui/settings/changelog.ts  新:版本说明数据
src-ui/settings/GeneralPage.tsx HotkeyPage.tsx LlmPage.tsx VocabPage.tsx
src-ui/settings/AppStylePage.tsx HelpPage.tsx AboutPage.tsx   新:七个页面
```

不动的文件:`src/engine.rs`、`src/hotkey/*`、`src-ui/overlay.tsx`、`overlay.html`、`vite.config.ts`(settings/ 在 src-ui 下,tailwind content 已覆盖)、`tauri.conf.json`、`config.example.toml`。

---

### Task 1: config.rs —— Serialize + load_raw/save_to(TDD)

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs`(同文件 `#[cfg(test)]`,项目惯例)

- [ ] **Step 1: 写失败测试**(加到 `src/config.rs` 的 `mod tests` 末尾)

```rust
    #[test]
    fn serialize_roundtrip_preserves_values() {
        let mut cfg = Config::default();
        cfg.hotkey.primary = "RWin".into();
        cfg.asr.model_dir = "./models/sensevoice".into();
        cfg.llm.api_key = "sk-test".into();
        cfg.llm.vocabulary = vec!["Kubernetes".into(), "ByType".into()];
        cfg.app_style = vec![AppStyle { match_: "outlook".into(), style: "正式".into() }];
        cfg.sound.enabled = false;
        let text = toml::to_string_pretty(&cfg).unwrap();
        // rename 生效:写出的是 match 而不是 match_
        assert!(text.contains("match = \"outlook\""), "got: {text}");
        // 相对路径原样保留
        assert!(text.contains("./models/sensevoice"));
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn save_to_then_load_roundtrips() {
        let mut cfg = Config::default();
        cfg.llm.model = "deepseek-v4-flash".into();
        cfg.llm.vocabulary = vec!["OneDrive".into()];
        let dir = std::env::temp_dir().join("bytype-g4-save-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        cfg.save_to(&path).unwrap();
        let back = Config::load(&path.to_string_lossy()).unwrap();
        assert_eq!(back, cfg);
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 2: 跑测试确认失败**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib config::
```

预期:编译错误 —— `Config` 未实现 `Serialize`/`PartialEq`、无 `save_to` 方法。

- [ ] **Step 3: 最小实现**

3a. `src/config.rs` 顶部 `use serde::Deserialize;` 改为:

```rust
use serde::{Deserialize, Serialize};
```

3b. 八个结构体的 derive 全部加 `Serialize, PartialEq`(`Config` `AsrConfig` `LlmConfig` `InjectConfig` `OverlayConfig` `SoundConfig` `HotkeyConfig` 现为 `#[derive(Debug, Clone, Deserialize)]`,`AppStyle` 现为 `#[derive(Debug, Clone, Deserialize, Default)]`)。改成例如:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config { /* 字段不动 */ }
```

`AppStyle` 改成 `#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]`(其 `#[serde(rename = "match")]` 对序列化同样生效,无需改动)。

3c. `find_config_file` 由私有改公开(签名与实现不动,加文档):

```rust
/// 查找 config.toml,返回 (文件路径, 所在目录)。
/// 顺序:当前工作目录 → 可执行文件目录 → 其各级父目录。
pub fn find_config_file() -> anyhow::Result<(PathBuf, PathBuf)> {
```

3d. `impl Config` 里、`load_resolved` 旁新增两个方法,并把 `load_resolved` 改为复用 `load_raw`:

```rust
    /// 原样加载(路径字段不做相对→绝对解析),返回配置与 config.toml 路径。
    /// 设置界面用它,保证 "./models/sensevoice" 这类相对路径原样写回。
    pub fn load_raw() -> anyhow::Result<(Config, PathBuf)> {
        let (path, _base) = find_config_file()?;
        let cfg = Config::load(&path.to_string_lossy())?;
        Ok((cfg, path))
    }

    /// 序列化为 TOML 并整文件写回(手写注释会丢失,字段值全部保留)。
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let text = toml::to_string_pretty(self).context("序列化配置失败")?;
        std::fs::write(path, text)
            .with_context(|| format!("写入配置文件失败: {}", path.display()))?;
        Ok(())
    }

    /// 不依赖工作目录地加载:查找 config.toml,并把相对的 `asr.model_dir`
    /// 与提示音路径解析到 config 所在目录,得到绝对路径。
    pub fn load_resolved() -> anyhow::Result<Config> {
        let (mut cfg, path) = Config::load_raw()?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        cfg.asr.model_dir = resolve_model_dir(&base, &cfg.asr.model_dir);
        cfg.sound.start_sound = resolve_sound_path(&base, &cfg.sound.start_sound);
        cfg.sound.end_sound = resolve_sound_path(&base, &cfg.sound.end_sound);
        Ok(cfg)
    }
```

(原 `load_resolved` 函数体删除替换;`find_config_file` 返回的 dir 即文件父目录,语义不变。)

- [ ] **Step 4: 跑测试确认通过(全量,确认无回归)**

```powershell
cargo test -p voice-input --lib
```

预期:`50 passed`(原 48 + 新 2)。

- [ ] **Step 5: Commit**

```powershell
git add src/config.rs
git commit -m "feat(g4): 配置可序列化 + load_raw/save_to(原始路径往返写回)"
```

---

### Task 2: corrector.rs —— test_connection 连通性测试

**Files:**
- Modify: `src/corrector.rs`

不写单测(真实网络调用,项目惯例真机验证);本任务验证 = 编译 + 既有测试不回归。

- [ ] **Step 1: 实现**(加在 `compose_system_prompt` 函数之前,`impl Corrector` 块之外的自由函数)

```rust
/// 连通性测试:用给定 [llm] 配置发一条固定请求,返回(耗时 ms, 回复文本)。
/// 供设置界面"测试连接"按钮用:**不受** `enabled` 与 `skip_if_shorter_than` 影响,
/// temperature 固定 0;失败时原样返回错误(由调用方展示)。
pub fn test_connection(cfg: &LlmConfig) -> anyhow::Result<(u64, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_secs.max(1)))
        .build()?;
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "temperature": 0.0,
        "messages": [
            { "role": "system", "content": "你是连接测试助手,请只回复:你好,ByType!" },
            { "role": "user", "content": "ping" },
        ],
    });
    let start = std::time::Instant::now();
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()?
        .error_for_status()?;
    let value: Value = resp.json()?;
    let reply = parse_response(&value)
        .ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))?;
    Ok((start.elapsed().as_millis() as u64, reply))
}
```

(`Duration`/`json!`/`Value`/`parse_response` 该文件已有引入,无需新增 `use`。不复用 `build_request_body` 是因为它会带上表单的 temperature,而测试要求固定 0。)

- [ ] **Step 2: 编译 + 全量测试无回归**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib
```

预期:`50 passed`(与 Task 1 后相同)。

- [ ] **Step 3: Commit**

```powershell
git add src/corrector.rs
git commit -m "feat(g4): corrector::test_connection 连通性测试(固定提示词,temperature 0)"
```

---

### Task 3: src-tauri settings.rs —— 五个设置命令

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`(两处:`mod settings;` + invoke_handler)

- [ ] **Step 1: 新建 `src-tauri/src/settings.rs`**(完整文件)

```rust
//! 设置界面后端:读写 config.toml、LLM 连通测试、重启应用、打开配置目录。

use std::path::PathBuf;

use serde::Serialize;
use voice_input::config::{find_config_file, Config, LlmConfig};

#[derive(Serialize)]
pub struct GetConfigResp {
    pub config: Config,
    /// config.toml 的实际路径;找不到文件时为 None。
    pub path: Option<String>,
    /// 文件存在但解析失败时的错误信息(此时 config 为默认值)。
    pub error: Option<String>,
}

/// 读取**原始**配置(路径字段不解析,保证相对路径原样往返)。
#[tauri::command]
pub fn get_config() -> GetConfigResp {
    match find_config_file() {
        Ok((file, _dir)) => {
            let path = Some(file.display().to_string());
            match Config::load(&file.to_string_lossy()) {
                Ok(config) => GetConfigResp { config, path, error: None },
                Err(e) => GetConfigResp {
                    config: Config::default(),
                    path,
                    error: Some(format!("{e:#}")),
                },
            }
        }
        Err(_) => GetConfigResp { config: Config::default(), path: None, error: None },
    }
}

/// 整文件写回 config.toml;找不到原文件时写到程序目录。
#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    let path = match find_config_file() {
        Ok((file, _)) => file,
        Err(_) => exe_dir().ok_or("无法确定程序目录")?.join("config.toml"),
    };
    config.save_to(&path).map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
pub struct TestOk {
    pub latency_ms: u64,
    pub reply: String,
}

/// 用表单当前的 [llm] 值测试连通(阻塞 HTTP 放 spawn_blocking,不卡 UI 线程)。
#[tauri::command]
pub async fn test_llm(llm: LlmConfig) -> Result<TestOk, String> {
    tauri::async_runtime::spawn_blocking(move || {
        voice_input::corrector::test_connection(&llm)
            .map(|(latency_ms, reply)| TestOk { latency_ms, reply })
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 保存成功后由前端调用:重启应用,使新配置生效。
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

/// 用资源管理器打开 config.toml 所在目录(找不到则打开程序目录)。
#[tauri::command]
pub fn open_config_dir() -> Result<(), String> {
    let dir = match find_config_file() {
        Ok((_, dir)) => dir,
        Err(_) => exe_dir().ok_or("无法确定程序目录")?,
    };
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}
```

- [ ] **Step 2: 接进 `src-tauri/src/lib.rs`**

文件顶部(`use` 块之前)加:

```rust
mod settings;
```

invoke_handler 一行由:

```rust
        .invoke_handler(tauri::generate_handler![cancel_recording])
```

改为:

```rust
        .invoke_handler(tauri::generate_handler![
            cancel_recording,
            settings::get_config,
            settings::save_config,
            settings::test_llm,
            settings::restart_app,
            settings::open_config_dir
        ])
```

- [ ] **Step 3: 编译整个 workspace**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build
```

预期:编译通过(若 `app.restart()` 的返回类型 `!` 触发 unreachable 警告,属正常)。自定义命令无需 capability(项目已验证的 Tauri 2 行为)。

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/settings.rs src-tauri/src/lib.rs
git commit -m "feat(g4): settings 命令模块(get/save/test_llm/restart/open_dir)"
```

---

### Task 4: 开机自启 —— autostart 插件接入

**Files:**
- Modify: `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json`、`package.json`(经 npm install)

- [ ] **Step 1: Rust 依赖** —— `src-tauri/Cargo.toml` 的 `[dependencies]` 里 `tauri-plugin-single-instance = "2"` 下一行加:

```toml
tauri-plugin-autostart = "2"
```

- [ ] **Step 2: 注册插件** —— `src-tauri/src/lib.rs` 的 builder 链,在 `.plugin(tauri_plugin_single_instance::init(...))` 之后加:

```rust
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
```

(Windows 下走注册表 Run 键;MacosLauncher 参数仅 mac 生效,照官方签名传即可。)

- [ ] **Step 3: 能力授权** —— `src-tauri/capabilities/default.json` 的 permissions 改为:

```json
  "permissions": [
    "core:default",
    "autostart:default"
  ]
```

(`autostart:default` 含 allow-enable/allow-disable/allow-is-enabled。)

- [ ] **Step 4: npm 包**(仓库根目录)

```powershell
npm install @tauri-apps/plugin-autostart
```

预期:package.json dependencies 出现 `"@tauri-apps/plugin-autostart": "^2..."`。

- [ ] **Step 5: 编译验证**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo build
npm run build
```

预期:双双通过(前端尚未使用该包,本步只确认依赖装配无误)。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/capabilities/default.json package.json package-lock.json Cargo.lock
git commit -m "feat(g4): 接入 tauri-plugin-autostart(Rust 插件 + npm 包 + 能力)"
```

---

### Task 5: 前端基座 —— types/api/consts/validate/widgets

**Files:**
- Create: `src-ui/settings/types.ts`、`src-ui/settings/api.ts`、`src-ui/settings/consts.ts`、`src-ui/settings/validate.ts`、`src-ui/settings/widgets.tsx`、`src-ui/vite-env.d.ts`

前端无单测惯例;本任务验证 = `npx tsc --noEmit` + `npm run build`。

- [ ] **Step 1: `src-ui/vite-env.d.ts`**(完整文件)

```ts
/// <reference types="vite/client" />
```

- [ ] **Step 2: `src-ui/settings/types.ts`**(完整文件)

```ts
// 与 src/config.rs 的 serde JSON 字段一一对应(snake_case,零映射)。
// 注意 AppStyle 的 Rust 字段 match_ 经 #[serde(rename = "match")] 在 JSON 里是 match。

export interface HotkeyConfig {
  primary: string;
  translate_modifier: string;
  command_modifier: string;
}

export interface AsrConfig {
  model_dir: string;
  language: string;
}

export interface LlmConfig {
  enabled: boolean;
  base_url: string;
  api_key: string;
  model: string;
  mode: string;
  system_prompt: string;
  translate_prompt: string;
  command_prompt: string;
  temperature: number;
  timeout_secs: number;
  skip_if_shorter_than: number;
  vocabulary: string[];
}

export interface InjectConfig {
  mode: string;
}

export interface AppStyle {
  match: string;
  style: string;
}

export interface OverlayConfig {
  enabled: boolean;
}

export interface SoundConfig {
  enabled: boolean;
  start_sound: string;
  end_sound: string;
}

export interface Config {
  hotkey: HotkeyConfig;
  asr: AsrConfig;
  llm: LlmConfig;
  inject: InjectConfig;
  app_style: AppStyle[];
  overlay: OverlayConfig;
  sound: SoundConfig;
}

export interface GetConfigResp {
  config: Config;
  path: string | null;
  error: string | null;
}

export interface TestOk {
  latency_ms: number;
  reply: string;
}

/** 各设置页的统一 props:配置 + 不可变更新器。 */
export interface PageProps {
  cfg: Config;
  set: (updater: (c: Config) => Config) => void;
}
```

- [ ] **Step 3: `src-ui/settings/api.ts`**(完整文件)

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Config, GetConfigResp, LlmConfig, TestOk } from "./types";

export const getConfig = () => invoke<GetConfigResp>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const testLlm = (llm: LlmConfig) => invoke<TestOk>("test_llm", { llm });
export const restartApp = () => invoke<void>("restart_app");
export const openConfigDir = () => invoke<void>("open_config_dir");
```

- [ ] **Step 4: `src-ui/settings/consts.ts`**(完整文件)

```ts
// 与 src/keys.rs 支持的 8 个键名保持一致。
export const KEY_OPTIONS = [
  { value: "LWin", label: "左 Win" },
  { value: "RWin", label: "右 Win" },
  { value: "LAlt", label: "左 Alt" },
  { value: "RAlt", label: "右 Alt" },
  { value: "LCtrl", label: "左 Ctrl" },
  { value: "RCtrl", label: "右 Ctrl" },
  { value: "LShift", label: "左 Shift" },
  { value: "RShift", label: "右 Shift" },
];

/** 键名 → 中文标签;未知键名原样返回(配置可能手写了别的值)。 */
export const keyLabel = (v: string) =>
  KEY_OPTIONS.find((k) => k.value.toLowerCase() === v.trim().toLowerCase())?.label ?? v;

export const LANG_OPTIONS = [
  { value: "auto", label: "自动" },
  { value: "zh", label: "中文" },
  { value: "en", label: "英文" },
  { value: "yue", label: "粤语" },
  { value: "ja", label: "日语" },
  { value: "ko", label: "韩语" },
];

export const MODE_OPTIONS = [
  { value: "clean", label: "忠实清理" },
  { value: "polish", label: "智能整理" },
  { value: "summary", label: "要点提炼" },
];
```

- [ ] **Step 5: `src-ui/settings/validate.ts`**(完整文件)

```ts
import type { HotkeyConfig } from "./types";

/** 三个热键任意两个相同(大小写不敏感)即冲突。 */
export function hotkeyConflict(h: HotkeyConfig): boolean {
  const keys = [h.primary, h.translate_modifier, h.command_modifier].map((s) =>
    s.trim().toLowerCase()
  );
  return new Set(keys).size < 3;
}
```

- [ ] **Step 6: `src-ui/settings/widgets.tsx`**(完整文件)

```tsx
import React, { useState } from "react";

/** 页面区块:标题 + 纵向内容。 */
export function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-base font-semibold text-neutral-900">{title}</h2>
      {children}
    </div>
  );
}

/** 设置行:左标签(+小字说明),右控件。 */
export function Row({ label, sub, children }: { label: string; sub?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-1">
      <div className="min-w-0">
        <div className="text-sm text-neutral-800">{label}</div>
        {sub && <div className="text-xs text-neutral-400 mt-0.5">{sub}</div>}
      </div>
      <div className="flex-none flex items-center gap-2">{children}</div>
    </div>
  );
}

export function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className={`w-10 h-[22px] rounded-full relative transition-colors ${checked ? "bg-blue-500" : "bg-neutral-300"}`}
    >
      <span
        className={`absolute top-[2px] w-[18px] h-[18px] rounded-full bg-white transition-all ${checked ? "right-[2px]" : "left-[2px]"}`}
      />
    </button>
  );
}

/** 文本输入:透传原生属性(value/onChange/type/placeholder/onKeyDown…)。 */
export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={`border border-neutral-300 rounded-md px-2.5 py-1.5 text-sm w-full focus:outline-none focus:border-blue-500 ${props.className ?? ""}`}
    />
  );
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <input
      type="number"
      value={value}
      min={min}
      max={max}
      step={step}
      onChange={(e) => {
        const n = Number(e.target.value);
        if (!Number.isNaN(n)) onChange(n);
      }}
      className="border border-neutral-300 rounded-md px-2.5 py-1.5 text-sm w-24 focus:outline-none focus:border-blue-500"
    />
  );
}

/** 下拉框;当前值不在选项里时(配置手写了未知值)原样保留为首项,避免静默改值。 */
export function SelectBox({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  const matched = options.find((o) => o.value.toLowerCase() === value.trim().toLowerCase());
  return (
    <select
      value={matched ? matched.value : value}
      onChange={(e) => onChange(e.target.value)}
      className="border border-neutral-300 rounded-md px-2 py-1.5 text-sm bg-white focus:outline-none focus:border-blue-500"
    >
      {!matched && <option value={value}>{value}</option>}
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Collapsible({ title, children }: { title: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="border-t border-dashed border-neutral-200 pt-3">
      <button type="button" onClick={() => setOpen(!open)} className="text-sm text-neutral-500 hover:text-neutral-700">
        {open ? "▾" : "▸"} {title}
      </button>
      {open && <div className="mt-3 flex flex-col gap-3">{children}</div>}
    </div>
  );
}
```

- [ ] **Step 7: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:双双通过(新文件暂未被引用,仅验证自身类型)。

- [ ] **Step 8: Commit**

```powershell
git add src-ui/vite-env.d.ts src-ui/settings
git commit -m "feat(g4): 设置前端基座(类型/API封装/常量/校验/共享小件)"
```

> **任务顺序说明:** Task 6–9 先做七个页面组件(各自独立、可单独类型检查,互不依赖),
> Task 10 最后重写 App.tsx 把它们串起来 —— 避免中途出现引用不存在文件的编译断档。

---

### Task 6: 通用页 + 热键页

**Files:**
- Create: `src-ui/settings/GeneralPage.tsx`、`src-ui/settings/HotkeyPage.tsx`

- [ ] **Step 1: `src-ui/settings/GeneralPage.tsx`**(完整文件)

```tsx
import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { LANG_OPTIONS } from "./consts";
import type { PageProps } from "./types";
import { Row, Section, SelectBox, Toggle } from "./widgets";

export default function GeneralPage({ cfg, set }: PageProps) {
  // 开机自启走 autostart 插件(注册表),立即生效,不进 config.toml、不参与脏检查。
  const [autoStart, setAutoStart] = useState(false);
  const [autoErr, setAutoErr] = useState<string | null>(null);

  useEffect(() => {
    isEnabled().then(setAutoStart).catch(() => {});
  }, []);

  const toggleAutostart = async (v: boolean) => {
    setAutoErr(null);
    setAutoStart(v); // 乐观切换
    try {
      if (v) await enable();
      else await disable();
    } catch (e) {
      setAutoStart(!v); // 失败回弹
      setAutoErr(String(e));
    }
  };

  return (
    <Section title="通用">
      <Row label="录音浮窗" sub="录音时屏幕底部显示计时药丸">
        <Toggle
          checked={cfg.overlay.enabled}
          onChange={(v) => set((c) => ({ ...c, overlay: { ...c.overlay, enabled: v } }))}
        />
      </Row>
      <Row label="提示音" sub="录音开始/结束播放提示音">
        <Toggle
          checked={cfg.sound.enabled}
          onChange={(v) => set((c) => ({ ...c, sound: { ...c.sound, enabled: v } }))}
        />
      </Row>
      <Row label="开机自启" sub="登录 Windows 后自动在后台运行(立即生效,无需保存)">
        {autoErr && <span className="text-xs text-red-600">{autoErr}</span>}
        <Toggle checked={autoStart} onChange={toggleAutostart} />
      </Row>
      <Row label="识别语言" sub="SenseVoice 识别语种">
        <SelectBox
          value={cfg.asr.language}
          onChange={(v) => set((c) => ({ ...c, asr: { ...c.asr, language: v } }))}
          options={LANG_OPTIONS}
        />
      </Row>
    </Section>
  );
}
```

- [ ] **Step 2: `src-ui/settings/HotkeyPage.tsx`**(完整文件)

```tsx
import { KEY_OPTIONS } from "./consts";
import type { PageProps } from "./types";
import { hotkeyConflict } from "./validate";
import { Row, Section, SelectBox } from "./widgets";

const ITEMS = [
  { key: "primary", label: "主键", sub: "按住说话,松手出字" },
  { key: "translate_modifier", label: "翻译键", sub: "主键+它:译成英文" },
  { key: "command_modifier", label: "命令键", sub: "主键+它:对选中文字执行语音命令" },
] as const;

export default function HotkeyPage({ cfg, set }: PageProps) {
  const conflict = hotkeyConflict(cfg.hotkey);
  return (
    <Section title="热键">
      {ITEMS.map((it) => (
        <Row key={it.key} label={it.label} sub={it.sub}>
          <div className={conflict ? "rounded-md ring-2 ring-red-400" : ""}>
            <SelectBox
              value={cfg.hotkey[it.key]}
              onChange={(v) => set((c) => ({ ...c, hotkey: { ...c.hotkey, [it.key]: v } }))}
              options={KEY_OPTIONS}
            />
          </div>
        </Row>
      ))}
      {conflict && <p className="text-xs text-red-600">三个热键必须互不相同。</p>}
      <p className="text-xs text-neutral-400">
        修饰键要在主键按住期间一起按下;录音中按 Esc 或点浮窗药丸可取消。
      </p>
    </Section>
  );
}
```

- [ ] **Step 3: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过(组件尚未挂进 App,tsc 全量检查仍覆盖)。

- [ ] **Step 4: Commit**

```powershell
git add src-ui/settings/GeneralPage.tsx src-ui/settings/HotkeyPage.tsx
git commit -m "feat(g4): 通用页(开关/自启/语言)+ 热键页(下拉+冲突标红)"
```

---

### Task 7: LLM 整理页(测试按钮 + 高级折叠区)

**Files:**
- Create: `src-ui/settings/LlmPage.tsx`

- [ ] **Step 1: `src-ui/settings/LlmPage.tsx`**(完整文件)

```tsx
import { useState } from "react";
import { testLlm } from "./api";
import { MODE_OPTIONS } from "./consts";
import type { LlmConfig, PageProps } from "./types";
import { Collapsible, NumberInput, Row, Section, TextInput, Toggle } from "./widgets";

type TestState =
  | { st: "idle" }
  | { st: "testing" }
  | { st: "ok"; ms: number; reply: string }
  | { st: "err"; msg: string };

const PROMPTS = [
  { key: "system_prompt", label: "整理提示词" },
  { key: "translate_prompt", label: "翻译提示词" },
  { key: "command_prompt", label: "命令提示词" },
] as const;

export default function LlmPage({ cfg, set }: PageProps) {
  const [showKey, setShowKey] = useState(false);
  const [test, setTest] = useState<TestState>({ st: "idle" });
  const llm = cfg.llm;
  const setLlm = (patch: Partial<LlmConfig>) =>
    set((c) => ({ ...c, llm: { ...c.llm, ...patch } }));

  const runTest = async () => {
    setTest({ st: "testing" });
    try {
      const r = await testLlm(llm); // 用表单当前值,无需先保存
      setTest({ st: "ok", ms: r.latency_ms, reply: r.reply.slice(0, 50) });
    } catch (e) {
      setTest({ st: "err", msg: String(e).slice(0, 120) });
    }
  };

  return (
    <Section title="LLM 整理">
      <Row label="启用" sub="关闭则直接输出识别原文,不调用 LLM">
        <Toggle checked={llm.enabled} onChange={(v) => setLlm({ enabled: v })} />
      </Row>
      <Row label="接口地址" sub="OpenAI 兼容,通常以 /v1 结尾">
        <div className="w-72">
          <TextInput
            value={llm.base_url}
            placeholder="https://example.com/v1"
            onChange={(e) => setLlm({ base_url: e.target.value })}
          />
        </div>
      </Row>
      <Row label="API Key">
        <div className="w-72 flex items-center gap-1.5">
          <TextInput
            type={showKey ? "text" : "password"}
            value={llm.api_key}
            onChange={(e) => setLlm({ api_key: e.target.value })}
          />
          <button
            type="button"
            className="text-neutral-400 hover:text-neutral-600"
            title={showKey ? "隐藏" : "显示"}
            onClick={() => setShowKey(!showKey)}
          >
            👁
          </button>
        </div>
      </Row>
      <Row label="模型">
        <div className="w-72">
          <TextInput
            value={llm.model}
            placeholder="deepseek-v4-flash"
            onChange={(e) => setLlm({ model: e.target.value })}
          />
        </div>
      </Row>
      <Row label="整理力度" sub="忠实清理=只去口语词;智能整理=理顺+取自我更正;要点提炼=压缩成要点">
        <div className="flex border border-neutral-300 rounded-md overflow-hidden">
          {MODE_OPTIONS.map((m) => (
            <button
              key={m.value}
              type="button"
              onClick={() => setLlm({ mode: m.value })}
              className={`px-3 py-1.5 text-sm ${
                llm.mode === m.value
                  ? "bg-blue-500 text-white"
                  : "bg-white text-neutral-600 hover:bg-neutral-50"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
      </Row>

      <div className="flex items-center gap-3 flex-wrap">
        <button
          type="button"
          onClick={runTest}
          disabled={test.st === "testing"}
          className="px-3.5 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-700 bg-white hover:bg-neutral-50 disabled:opacity-50"
        >
          ⚡ {test.st === "testing" ? "测试中…" : "测试连接"}
        </button>
        {test.st === "ok" && (
          <span className="text-xs text-emerald-600">
            ✓ 连接正常 · {test.ms}ms · 回复:「{test.reply}」
          </span>
        )}
        {test.st === "err" && <span className="text-xs text-red-600">✗ {test.msg}</span>}
        <span className="text-xs text-neutral-400">用当前表单值测试,无需先保存</span>
      </div>

      <Collapsible title="高级">
        <Row label="temperature" sub="0–2,越低输出越稳定">
          <NumberInput
            value={llm.temperature}
            onChange={(v) => setLlm({ temperature: Math.min(2, Math.max(0, v)) })}
            min={0}
            max={2}
            step={0.1}
          />
        </Row>
        <Row label="超时秒数">
          <NumberInput
            value={llm.timeout_secs}
            onChange={(v) => setLlm({ timeout_secs: Math.max(1, Math.round(v)) })}
            min={1}
          />
        </Row>
        <Row label="短文本跳过阈值" sub="识别文本字符数小于该值时不调用 LLM">
          <NumberInput
            value={llm.skip_if_shorter_than}
            onChange={(v) => setLlm({ skip_if_shorter_than: Math.max(0, Math.round(v)) })}
            min={0}
          />
        </Row>
        {PROMPTS.map((p) => (
          <div key={p.key} className="flex flex-col gap-1">
            <div className="text-sm text-neutral-800">{p.label}</div>
            <textarea
              value={llm[p.key]}
              rows={3}
              placeholder="留空使用内置预设"
              onChange={(e) => setLlm({ [p.key]: e.target.value } as Partial<LlmConfig>)}
              className="border border-neutral-300 rounded-md px-2.5 py-1.5 text-sm focus:outline-none focus:border-blue-500"
            />
          </div>
        ))}
      </Collapsible>
    </Section>
  );
}
```

- [ ] **Step 2: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过。

- [ ] **Step 3: Commit**

```powershell
git add src-ui/settings/LlmPage.tsx
git commit -m "feat(g4): LLM 整理页(配置/测试连接/高级折叠区)"
```

---

### Task 8: 词库页 + 应用风格页

**Files:**
- Create: `src-ui/settings/VocabPage.tsx`、`src-ui/settings/AppStylePage.tsx`

- [ ] **Step 1: `src-ui/settings/VocabPage.tsx`**(完整文件)

```tsx
import { useState } from "react";
import type { PageProps } from "./types";
import { Section, TextInput } from "./widgets";

export default function VocabPage({ cfg, set }: PageProps) {
  const [draft, setDraft] = useState("");
  const words = cfg.llm.vocabulary;

  const add = () => {
    const w = draft.trim();
    if (!w) return;
    if (!words.includes(w)) {
      set((c) => ({ ...c, llm: { ...c.llm, vocabulary: [...c.llm.vocabulary, w] } }));
    }
    setDraft(""); // 重复词也清空输入,视觉上"已存在"
  };

  const remove = (i: number) =>
    set((c) => ({ ...c, llm: { ...c.llm, vocabulary: c.llm.vocabulary.filter((_, j) => j !== i) } }));

  return (
    <Section title="词库">
      <p className="text-xs text-neutral-400">
        专有名词优先按以下拼写输出(如 Kubernetes、OneDrive)。输入后按回车添加,点 × 删除。
      </p>
      <div className="flex flex-wrap items-center gap-2">
        {words.map((w, i) => (
          <span
            key={`${w}-${i}`}
            className="inline-flex items-center gap-1.5 rounded-full border border-blue-200 bg-blue-50 text-blue-700 text-sm px-3 py-1"
          >
            {w}
            <button type="button" className="text-blue-400 hover:text-blue-700" onClick={() => remove(i)}>
              ×
            </button>
          </span>
        ))}
        <div className="w-44">
          <TextInput
            value={draft}
            placeholder="输入后按回车…"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
          />
        </div>
      </div>
    </Section>
  );
}
```

- [ ] **Step 2: `src-ui/settings/AppStylePage.tsx`**(完整文件)

```tsx
import type { AppStyle, PageProps } from "./types";
import { Section, TextInput } from "./widgets";

export default function AppStylePage({ cfg, set }: PageProps) {
  const rows = cfg.app_style;

  const update = (i: number, patch: Partial<AppStyle>) =>
    set((c) => ({
      ...c,
      app_style: c.app_style.map((r, j) => (j === i ? { ...r, ...patch } : r)),
    }));

  const remove = (i: number) =>
    set((c) => ({ ...c, app_style: c.app_style.filter((_, j) => j !== i) }));

  const addRow = () =>
    set((c) => ({ ...c, app_style: [...c.app_style, { match: "", style: "" }] }));

  return (
    <Section title="应用风格">
      <p className="text-xs text-neutral-400">
        前台进程名包含「匹配串」即生效(不区分大小写),取第一条命中;匹配串为空的行保存时自动忽略。
      </p>
      {rows.map((r, i) => (
        <div key={i} className="flex items-center gap-2">
          <div className="w-36 flex-none">
            <TextInput
              value={r.match}
              placeholder="如 outlook"
              onChange={(e) => update(i, { match: e.target.value })}
            />
          </div>
          <TextInput
            value={r.style}
            placeholder="如 用正式、专业的书面语。"
            onChange={(e) => update(i, { style: e.target.value })}
          />
          <button
            type="button"
            className="flex-none text-red-400 hover:text-red-600"
            title="删除"
            onClick={() => remove(i)}
          >
            🗑
          </button>
        </div>
      ))}
      <div>
        <button
          type="button"
          onClick={addRow}
          className="px-3 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-600 bg-white hover:bg-neutral-50"
        >
          + 添加规则
        </button>
      </div>
    </Section>
  );
}
```

- [ ] **Step 3: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过。

- [ ] **Step 4: Commit**

```powershell
git add src-ui/settings/VocabPage.tsx src-ui/settings/AppStylePage.tsx
git commit -m "feat(g4): 词库页(标签输入)+ 应用风格页(规则表)"
```

---

### Task 9: 帮助页 + 关于页 + 版本说明数据

**Files:**
- Create: `src-ui/settings/changelog.ts`、`src-ui/settings/HelpPage.tsx`、`src-ui/settings/AboutPage.tsx`

- [ ] **Step 1: `src-ui/settings/changelog.ts`**(完整文件)

```ts
export interface Release {
  version: string;
  date: string;
  items: string[];
}

/** 版本说明(新版本加在数组最前)。 */
export const CHANGELOG: Release[] = [
  {
    version: "0.1.0",
    date: "2026-06-10",
    items: [
      "按住热键说话即输入:本地 SenseVoice 识别 + LLM 整理",
      "翻译热键:说中文,输出英文",
      "命令热键:对选中文字执行语音指令",
      "录音浮窗:计时药丸,点它或按 Esc 取消",
      "录音开始/结束提示音",
      "设置界面(本页所在的这一套)",
    ],
  },
];
```

- [ ] **Step 2: `src-ui/settings/HelpPage.tsx`**(完整文件;只读 cfg,键名跟随当前表单值)

```tsx
import { CHANGELOG } from "./changelog";
import { keyLabel } from "./consts";
import type { Config } from "./types";
import { Section } from "./widgets";

const FAQS: [string, string][] = [
  [
    "按了热键没出字?",
    "确认目标输入框拥有焦点;以管理员权限运行的程序需要 ByType 也以管理员身份运行;说话需按住至少 0.3 秒,过短视为误触丢弃。",
  ],
  [
    "测试连接失败?",
    "检查接口地址(通常以 /v1 结尾)、API Key 与网络;在「LLM 整理」页点「测试连接」可看到具体原因。",
  ],
  [
    "找不到 config.toml?",
    "在「关于」页可查看配置文件路径;若文件不存在,在设置里点「保存并重启」会自动创建。",
  ],
  [
    "提示音没声音?",
    "确认「通用」页提示音开关已开,且 Windows 音量混合器中 ByType 未被静音。",
  ],
];

export default function HelpPage({ cfg }: { cfg: Config }) {
  const p = keyLabel(cfg.hotkey.primary);
  const t = keyLabel(cfg.hotkey.translate_modifier);
  const m = keyLabel(cfg.hotkey.command_modifier);

  return (
    <div className="flex flex-col gap-6">
      <Section title="使用说明">
        <ul className="text-sm text-neutral-700 flex flex-col gap-1.5 list-disc pl-5">
          <li>
            按住 <b>{p}</b> 说话,松手自动识别、整理并输入到当前光标处。
          </li>
          <li>
            按住 <b>{p} + {t}</b> 说话:整理后翻译成英文输出。
          </li>
          <li>
            先选中一段文字,按住 <b>{p} + {m}</b> 说出修改指令(如"改得正式一点"):用结果替换选中文字。
          </li>
          <li>
            录音中按 <b>Esc</b> 或点击底部药丸:取消本次录音,不出字。
          </li>
          <li>按住不足 0.3 秒视为误触,自动丢弃。</li>
        </ul>
      </Section>
      <Section title="常见问题">
        {FAQS.map(([q, a]) => (
          <details key={q} className="text-sm">
            <summary className="cursor-pointer text-neutral-800">{q}</summary>
            <p className="mt-1.5 text-neutral-500 pl-4">{a}</p>
          </details>
        ))}
      </Section>
      <Section title="版本说明">
        {CHANGELOG.map((r) => (
          <div key={r.version} className="text-sm">
            <div className="font-medium text-neutral-800">
              v{r.version}
              <span className="text-xs text-neutral-400 ml-2">{r.date}</span>
            </div>
            <ul className="mt-1 list-disc pl-5 text-neutral-600">
              {r.items.map((it) => (
                <li key={it}>{it}</li>
              ))}
            </ul>
          </div>
        ))}
      </Section>
    </div>
  );
}
```

- [ ] **Step 3: `src-ui/settings/AboutPage.tsx`**(完整文件)

```tsx
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openConfigDir } from "./api";
import { Section } from "./widgets";
import iconUrl from "../../src-tauri/icons/icon.png";

const EMAIL = "jklover2025@outlook.com";

export default function AboutPage({ cfgPath }: { cfgPath: string | null }) {
  const [version, setVersion] = useState("…");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion("未知"));
  }, []);

  const copyEmail = async () => {
    try {
      await navigator.clipboard.writeText(EMAIL);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // 剪贴板不可用时静默;用户可手动选中复制
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center gap-4">
        <img src={iconUrl} alt="ByType" className="w-14 h-14 rounded-xl" />
        <div>
          <div className="text-lg font-semibold text-neutral-900">
            ByType <span className="text-sm font-normal text-neutral-400 ml-1">v{version}</span>
          </div>
          <div className="text-sm text-neutral-500">按住热键说话,松手即出字 —— 本地识别 + LLM 整理。</div>
        </div>
      </div>
      <Section title="作者与联系">
        <p className="text-sm text-neutral-700">© 2026 Yong Zhang</p>
        <p className="text-sm text-neutral-700 flex items-center gap-2">
          <a className="text-blue-600 hover:underline" href={`mailto:${EMAIL}`}>
            {EMAIL}
          </a>
          <button
            type="button"
            onClick={copyEmail}
            className="text-xs px-2 py-0.5 rounded border border-neutral-300 text-neutral-500 hover:bg-neutral-50"
          >
            {copied ? "已复制 ✓" : "复制"}
          </button>
        </p>
      </Section>
      <Section title="配置文件">
        <p className="text-xs text-neutral-500 break-all">
          {cfgPath ?? "未找到,保存后将创建于程序目录"}
        </p>
        <div>
          <button
            type="button"
            onClick={() => openConfigDir()}
            className="px-3 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-600 bg-white hover:bg-neutral-50"
          >
            打开所在文件夹
          </button>
        </div>
      </Section>
      <p className="text-xs text-neutral-400">第三方开源组件致谢将在正式安装版中提供。</p>
    </div>
  );
}
```

(图标直接 import 仓库内 `src-tauri/icons/icon.png`,Vite 会打包该资源;`vite-env.d.ts` 已在 Task 5 提供 `*.png` 模块类型。)

- [ ] **Step 4: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过。

- [ ] **Step 5: Commit**

```powershell
git add src-ui/settings/changelog.ts src-ui/settings/HelpPage.tsx src-ui/settings/AboutPage.tsx
git commit -m "feat(g4): 帮助页(动态热键说明/FAQ/版本说明)+ 关于页(版本/署名/配置路径)"
```

---

### Task 10: App.tsx 设置壳 —— 串联七页 + 脏检查 + 保存栏

**Files:**
- Modify: `src-ui/App.tsx`(整文件重写,原 G1 占位页删除)

- [ ] **Step 1: 重写 `src-ui/App.tsx`**(完整文件)

```tsx
import { useEffect, useMemo, useState } from "react";
import { getConfig, restartApp, saveConfig } from "./settings/api";
import type { Config } from "./settings/types";
import { hotkeyConflict } from "./settings/validate";
import AboutPage from "./settings/AboutPage";
import AppStylePage from "./settings/AppStylePage";
import GeneralPage from "./settings/GeneralPage";
import HelpPage from "./settings/HelpPage";
import HotkeyPage from "./settings/HotkeyPage";
import LlmPage from "./settings/LlmPage";
import VocabPage from "./settings/VocabPage";

const PAGES = [
  { id: "general", icon: "⚙", label: "通用" },
  { id: "hotkey", icon: "⌨", label: "热键" },
  { id: "llm", icon: "✨", label: "LLM 整理" },
  { id: "vocab", icon: "📖", label: "词库" },
  { id: "style", icon: "🎯", label: "应用风格" },
  { id: "help", icon: "❓", label: "帮助" },
  { id: "about", icon: "ℹ️", label: "关于" },
] as const;
type PageId = (typeof PAGES)[number]["id"];

export default function App() {
  const [page, setPage] = useState<PageId>("general");
  const [cfg, setCfg] = useState<Config | null>(null);
  const [snapshot, setSnapshot] = useState(""); // 加载时的 JSON 快照,用于脏检查与"放弃更改"
  const [cfgPath, setCfgPath] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getConfig().then((r) => {
      setCfg(r.config);
      setSnapshot(JSON.stringify(r.config));
      setCfgPath(r.path);
      setLoadError(r.error);
    });
  }, []);

  const dirty = useMemo(
    () => cfg !== null && JSON.stringify(cfg) !== snapshot,
    [cfg, snapshot]
  );
  const conflict = cfg !== null && hotkeyConflict(cfg.hotkey);

  if (cfg === null) {
    return (
      <div className="h-screen flex items-center justify-center text-neutral-400 text-sm">
        加载配置…
      </div>
    );
  }

  const set = (updater: (c: Config) => Config) => {
    setSaveError(null);
    setCfg((c) => (c ? updater(c) : c));
  };

  const onDiscard = () => {
    setCfg(JSON.parse(snapshot) as Config);
    setSaveError(null);
  };

  const onSave = async () => {
    setSaving(true);
    setSaveError(null);
    // 匹配串为空的应用风格行保存时忽略(spec)
    const toSave: Config = {
      ...cfg,
      app_style: cfg.app_style.filter((r) => r.match.trim() !== ""),
    };
    try {
      await saveConfig(toSave);
      await restartApp(); // 应用即将重启,这个 Promise 不会正常返回
    } catch (e) {
      setSaveError(String(e));
      setSaving(false);
    }
  };

  return (
    <div className="h-screen flex flex-col bg-white text-neutral-800">
      <div className="flex-1 flex min-h-0">
        <nav className="w-44 flex-none bg-neutral-50 border-r border-neutral-200 p-2.5 flex flex-col gap-1">
          {PAGES.map((p) => (
            <button
              key={p.id}
              onClick={() => setPage(p.id)}
              className={`text-left px-3 py-2 rounded-md text-sm ${
                page === p.id ? "bg-blue-500 text-white" : "text-neutral-600 hover:bg-neutral-100"
              }`}
            >
              {p.icon} {p.label}
            </button>
          ))}
        </nav>
        <main className="flex-1 min-w-0 overflow-y-auto px-6 py-5">
          {loadError && (
            <div className="mb-4 text-xs rounded-md border border-amber-300 bg-amber-50 text-amber-800 px-3 py-2">
              config.toml 解析失败:{loadError} —— 以下显示默认值,保存将整文件覆盖。
            </div>
          )}
          {!loadError && cfgPath === null && (
            <div className="mb-4 text-xs rounded-md border border-amber-300 bg-amber-50 text-amber-800 px-3 py-2">
              未找到 config.toml,保存时将在程序目录创建。
            </div>
          )}
          {page === "general" && <GeneralPage cfg={cfg} set={set} />}
          {page === "hotkey" && <HotkeyPage cfg={cfg} set={set} />}
          {page === "llm" && <LlmPage cfg={cfg} set={set} />}
          {page === "vocab" && <VocabPage cfg={cfg} set={set} />}
          {page === "style" && <AppStylePage cfg={cfg} set={set} />}
          {page === "help" && <HelpPage cfg={cfg} />}
          {page === "about" && <AboutPage cfgPath={cfgPath} />}
        </main>
      </div>
      {dirty && (
        <div className="flex-none border-t border-neutral-200 bg-amber-50 px-4 py-2.5 flex items-center gap-3">
          <span className="text-sm text-amber-700">● 有未保存的更改</span>
          {conflict && <span className="text-xs text-red-600">热键互相冲突,无法保存</span>}
          {saveError && (
            <span className="text-xs text-red-600 truncate">保存失败:{saveError}</span>
          )}
          <span className="flex-1" />
          <span className="text-xs text-neutral-400">保存后 ByType 将自动重启</span>
          <button
            onClick={onDiscard}
            disabled={saving}
            className="px-3.5 py-1.5 rounded-md border border-neutral-300 text-sm text-neutral-600 bg-white hover:bg-neutral-50"
          >
            放弃更改
          </button>
          <button
            onClick={onSave}
            disabled={conflict || saving}
            className="px-3.5 py-1.5 rounded-md text-sm text-white bg-blue-500 hover:bg-blue-600 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {saving ? "保存中…" : "保存并重启"}
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过;dist/ 正常产出两个入口。

- [ ] **Step 3: Commit**

```powershell
git add src-ui/App.tsx
git commit -m "feat(g4): App 设置壳(侧边导航/脏检查/保存并重启/横幅)"
```

---

### Task 11: 暗黑模式(外观三选,立即生效)

**Files:**
- Create: `src-ui/settings/theme.ts`
- Modify: `tailwind.config.js`(加 darkMode)、`src-ui/main.tsx`(initTheme)、
  `src-ui/settings/GeneralPage.tsx`(外观行)、`src-ui/settings/widgets.tsx`、`src-ui/App.tsx`
  及七个页面(按映射表补 `dark:` 变体)

- [ ] **Step 1: `src-ui/settings/theme.ts`**(完整文件)

```ts
// 外观主题:跟随系统/浅色/深色。立即生效,localStorage 持久化,
// 不进 config.toml、不参与脏检查/保存栏(与开机自启同类)。
export type Theme = "system" | "light" | "dark";

export const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
];

const KEY = "bt-theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: Theme = read();

function read(): Theme {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : "system";
}

function apply(t: Theme) {
  const dark = t === "dark" || (t === "system" && media.matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function getTheme(): Theme {
  return current;
}

export function setTheme(t: Theme) {
  current = t;
  localStorage.setItem(KEY, t);
  apply(t);
}

/** 应用启动时调用一次:应用当前主题,并在「跟随系统」时联动系统切换。 */
export function initTheme() {
  apply(current);
  media.addEventListener("change", () => apply(current));
}
```

- [ ] **Step 2: `tailwind.config.js`** —— 在导出对象顶层(`content` 之前)加一行:

```js
  darkMode: "class",
```

- [ ] **Step 3: `src-ui/main.tsx`** —— 在 `import "./index.css";` 之后、`ReactDOM.createRoot` 之前加:

```tsx
import { initTheme } from "./settings/theme";

initTheme();
```

(overlay.tsx 不动 —— 浮窗药丸不随主题。)

- [ ] **Step 4: `GeneralPage.tsx` 加「外观」行**

imports 区加:

```tsx
import { getTheme, setTheme, THEME_OPTIONS, type Theme } from "./theme";
```

组件内 `autoErr` state 之后加:

```tsx
  const [theme, setThemeState] = useState<Theme>(getTheme());
```

「识别语言」Row 之前加(立即生效,不触发脏检查):

```tsx
      <Row label="外观" sub="界面配色,立即生效,无需保存">
        <SelectBox
          value={theme}
          onChange={(v) => {
            setTheme(v as Theme);
            setThemeState(v as Theme);
          }}
          options={THEME_OPTIONS}
        />
      </Row>
```

- [ ] **Step 5: 全部设置组件按映射表补 `dark:` 变体**

对 `widgets.tsx`、`App.tsx`、`GeneralPage/HotkeyPage/LlmPage/VocabPage/AppStylePage/HelpPage/AboutPage.tsx`
逐文件处理:凡出现左列类名,在同一 className 里追加右列(蓝色主按钮、选中态 `bg-blue-500 text-white`、
红/琥珀强调按下表所列,其余彩色保持不变):

| 现有类 | 追加 dark: 变体 |
|---|---|
| `bg-white`(窗口/输入框/按钮底) | `dark:bg-neutral-900`(窗口)/ `dark:bg-neutral-800`(输入框、select、ghost 按钮) |
| `bg-neutral-50`(侧栏/hover) | `dark:bg-neutral-950`(侧栏)/ `dark:hover:bg-neutral-800`(hover 类) |
| `hover:bg-neutral-100` / `hover:bg-neutral-50` | `dark:hover:bg-neutral-800` |
| `text-neutral-900` | `dark:text-neutral-100` |
| `text-neutral-800` | `dark:text-neutral-200` |
| `text-neutral-700` | `dark:text-neutral-300` |
| `text-neutral-600` | `dark:text-neutral-300` |
| `text-neutral-500` | `dark:text-neutral-400` |
| `text-neutral-400`(辅助字,够淡) | 不变 |
| `border-neutral-200` / `border-neutral-300` | `dark:border-neutral-700` |
| `bg-neutral-300`(Toggle 关) | `dark:bg-neutral-600` |
| `bg-amber-50 border-amber-300 text-amber-800`(黄条) | `dark:bg-amber-900/20 dark:border-amber-700 dark:text-amber-300` |
| `bg-amber-50`(保存栏)/ `text-amber-700` | `dark:bg-neutral-800` / `dark:text-amber-400` |
| `bg-blue-50 border-blue-200 text-blue-700`(词库 chips) | `dark:bg-blue-900/30 dark:border-blue-800 dark:text-blue-300` |
| `text-emerald-600` | `dark:text-emerald-400` |
| `text-red-600` | `dark:text-red-400` |
| `text-blue-600`(邮箱链接) | `dark:text-blue-400` |

- [ ] **Step 6: 类型检查 + 构建**

```powershell
npx tsc --noEmit
npm run build
```

预期:通过。

- [ ] **Step 7: Commit**

```powershell
git add tailwind.config.js src-ui
git commit -m "feat(g4): 暗黑模式(跟随系统/浅色/深色,立即生效,localStorage)"
```

---

### Task 12: 应用图标 —— 药丸波形(gen_icon + tauri icon)

**Files:**
- Create: `examples/gen_icon.rs`
- Modify: `Cargo.toml`(dev-dependencies 加 image)
- Regenerate: `src-tauri/icons/*`(经 `tauri icon`)

- [ ] **Step 1: `Cargo.toml` dev-dependencies** —— `hound = "3"` 下一行加:

```toml
image = "0.25"
```

- [ ] **Step 2: `examples/gen_icon.rs`**(完整文件)

```rust
//! 生成 ByType 应用图标:蓝色圆角药丸 + 白色波形五柱(与录音浮窗视觉一致)。
//! 运行:cargo run --example gen_icon
//! 输出:src-tauri/icons/app-icon-source.png(1024×1024,透明背景)
//! 然后:npm run tauri -- icon src-tauri/icons/app-icon-source.png  重新生成全套图标。

use image::{imageops, ImageBuffer, Rgba};

const SS: u32 = 4; // 超采样倍数(先大画再缩小 = 抗锯齿)
const OUT: u32 = 1024;

fn main() {
    let size = OUT * SS;
    let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(size, size, Rgba([0, 0, 0, 0]));

    let s = size as f32;
    let (cx, cy) = (0.5 * s, 0.5 * s);
    let blue = Rgba([59, 130, 246, 255]); // tailwind blue-500,与界面主色一致

    // 横向药丸:直段半长 0.24,端帽半径 0.24(总宽 0.96、高 0.48,居中)
    draw_capsule_h(&mut img, cx, cy, 0.24 * s, 0.24 * s, blue);

    // 白色波形五柱(竖直小胶囊),中间最高、两侧对称
    let white = Rgba([255, 255, 255, 255]);
    let bar_r = 0.032 * s;
    let gap = 0.10 * s;
    let halves = [0.08_f32, 0.13, 0.17, 0.13, 0.08];
    for (i, h) in halves.iter().enumerate() {
        let x = cx + (i as f32 - 2.0) * gap;
        draw_capsule_v(&mut img, x, cy, h * s, bar_r, white);
    }

    let small = imageops::resize(&img, OUT, OUT, imageops::FilterType::Lanczos3);
    let out = "src-tauri/icons/app-icon-source.png";
    small.save(out).expect("写出 PNG 失败");
    println!("已生成 {out}({OUT}x{OUT})");
}

/// 横向胶囊:水平线段 (cx±half, cy) 膨胀半径 r 的点集。
fn draw_capsule_h(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: f32,
    cy: f32,
    half: f32,
    r: f32,
    c: Rgba<u8>,
) {
    paint(img, c, |x, y| {
        let dx = ((x - cx).abs() - half).max(0.0);
        let dy = y - cy;
        dx * dx + dy * dy <= r * r
    });
}

/// 竖直胶囊:竖直线段 (cx, cy±half) 膨胀半径 r 的点集。
fn draw_capsule_v(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: f32,
    cy: f32,
    half: f32,
    r: f32,
    c: Rgba<u8>,
) {
    paint(img, c, |x, y| {
        let dy = ((y - cy).abs() - half).max(0.0);
        let dx = x - cx;
        dx * dx + dy * dy <= r * r
    });
}

fn paint(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, c: Rgba<u8>, hit: impl Fn(f32, f32) -> bool) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            if hit(x as f32 + 0.5, y as f32 + 0.5) {
                img.put_pixel(x, y, c);
            }
        }
    }
}
```

- [ ] **Step 3: 生成源图**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo run --example gen_icon
```

预期:打印 `已生成 src-tauri/icons/app-icon-source.png(1024x1024)`。

- [ ] **Step 4: 重新生成全套图标**(仓库根)

```powershell
npm run tauri -- icon src-tauri/icons/app-icon-source.png
```

预期:`src-tauri/icons/` 下 icon.ico / icon.png / 各尺寸 PNG 被覆盖更新(tauri.conf.json 的
bundle.icon 路径不变,无需改配置;托盘走 `default_window_icon`、关于页 import 同一 icon.png,
自动同步新图)。

- [ ] **Step 5: 编译确认 + 目检**

```powershell
cargo build
```

打开 `src-tauri/icons/app-icon-source.png` 与 `icon.png` 目检:蓝色药丸 + 白色五柱波形,边缘平滑。

- [ ] **Step 6: Commit**

```powershell
git add Cargo.toml Cargo.lock examples/gen_icon.rs src-tauri/icons
git commit -m "feat(g4): 药丸波形应用图标(gen_icon 程序化生成 + tauri icon 全套)"
```

---

### Task 13: 真机端到端验证(人工,GUI dev)

**Files:** 无代码改动;发现问题则修复并单独提交。

- [ ] **Step 1: 启动 GUI dev**(仓库根;cargo 环境前缀同上)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
npm run tauri dev
```

- [ ] **Step 2: 按 spec 真机清单逐项验证**(托盘菜单「设置」打开主窗口)

1. 七页都能打开,加载值与 config.toml 一致(LLM key 掩码显示)。
2. 改「整理力度」→ 保存栏出现 → 放弃更改 → 回滚且保存栏消失。
3. 热键页把翻译键改成与主键相同 → 红框 + 提示 + 保存按钮禁用;改回 → 恢复。
4. LLM 页「测试连接」→ 真实 ✓ 延迟 + 回复;把 key 改错一位再测 → ✗ 原因;改回。
5. 词库添加/删除标签;应用风格添加一行(留空匹配串)+ 一行有效规则。
6. 改 mode 为 clean + 关提示音 → 「保存并重启」→ 应用重启(托盘重现)→ config.toml 检查:
   值已更新、空匹配串行被过滤、`model_dir` 仍是相对路径原样、API key 原值保留。
7. 重启后听写一次:提示音已静音、LLM 走 clean(输出更"忠实");再开回 polish + 提示音,保存重启。
8. 通用页开机自启开 → 任务管理器「启动应用」出现 ByType;关 → 消失。
9. 关于页:版本 0.1.0、署名/邮箱正确、「复制」按钮工作、「打开所在文件夹」弹出资源管理器且定位正确。
10. 帮助页:把主键改成 RWin(不保存),帮助页说明跟着显示「右 Win」;放弃更改。
11. 无 config.toml 场景(把 config.toml 临时改名):重开设置 → 黄条提示;改一项保存 → 程序目录生成新
    config.toml;验证后删除生成文件、恢复原 config.toml。
12. Esc/点药丸取消、三热键听写 —— 确认 G1–G3 行为无回归。
13. 外观:三选切换立即生效(不出保存栏);选「深色」后关开设置窗口仍是深色;选「跟随系统」后切换
    Windows 颜色模式(设置→个性化→颜色),界面跟随;浮窗药丸样式不受影响。
14. 新图标:任务栏运行图标与托盘图标均为蓝色药丸波形;关于页应用图标同步;
    (exe/安装包图标在 `npm run tauri build` 产物上抽查,可留到 G6 一并验)。

- [ ] **Step 3: 修复发现的问题**(每个修复单独 commit,信息格式 `fix(g4): <问题>`)

- [ ] **Step 4: 全量回归**

```powershell
cargo test -p voice-input --lib
```

预期:`50 passed`。

---

## Plan 自检备注(写计划时已核对)

- **Spec 覆盖:** 决策表逐条→ 保存并重启(T3/T10)、七页布局(T6–T10)、自启+高级提示词(T4/T6/T7)、
  下拉热键+冲突(T6/T5 validate)、chips 词库(T8)、风格行表+空行过滤(T8/T10)、测试按钮表单值(T2/T3/T7)、
  TOML 整写+注释丢失(T1)、署名/邮箱(T9)、动态帮助键名(T9)、changelog(T9)、黄条两种(T10)、
  关于页路径 None 文案(T9)、open_config_dir(T3/T9)。
- **类型一致:** `PageProps{cfg,set}`(T5)被 T6–T8 页面使用;`AppStyle.match`(TS)对应 Rust `match_` rename;
  `GetConfigResp/TestOk` 字段 T3(Rust)与 T5(TS)一致(latency_ms/reply/path/error);
  `find_config_file` T1 转 pub、T3 引用;`test_connection` T2 定义、T3 spawn_blocking 调用。
- **无占位:** 每个代码步骤均为完整文件或精确 diff;无 TBD/"类似 Task N"。
- **增补(用户追加需求):** 暗黑模式(T11:theme.ts 全代码 + tailwind darkMode + GeneralPage 外观行 +
  dark: 变体映射表)、药丸波形应用图标(T12:gen_icon.rs 全代码 + tauri icon 流程);
  真机清单相应加第 13/14 项。T11 的 dark: 映射表是精确的类名→类名规则(机械可执行),非占位。
