# ByType G1 — Tauri 外壳基座 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把仓库变成 Cargo workspace,提炼听写引擎为 `voice_input::engine::run`,新增 Tauri 2 应用 ByType(托盘常驻 + 后端引擎线程 + React/Tailwind 占位前端),功能与现命令行版等价。

**Architecture:** 根 crate 仍是 `voice_input` 核心库(+旧 bin/examples);新增 `src-tauri` Tauri 2 应用依赖它;前端 React+TS+Tailwind+Vite 放 `src-ui/`,Vite 输出 `dist/`。

**Tech Stack:** Tauri 2、React + TypeScript + Tailwind + Vite、Node v24/npm 11(已装)、Rust(已装)。

---

## 运行环境(执行者必读)
- cargo 经 **PowerShell**,每条前置:`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; `
- node/npm 在系统 PATH(`C:\Program Files\nodejs`),PowerShell 直接可用。
- 不要 `cd`;工作目录已是项目根。git 用 bash。
- 不要运行听写二进制(全局键盘钩子)。Tauri dev 由用户/控制器手动验证。
- 分支 `g1-tauri-shell`。

---

## Task 1: 提炼听写引擎到 lib(可回归)

把现 `src/main.rs` 的整条循环搬到 `src/engine.rs`,旧 bin 改为薄壳调用。行为不变,靠现有 34 单测 + 手动守护。

**Files:** Create `src/engine.rs`;Modify `src/lib.rs`、`src/main.rs`

- [ ] **Step 1: 新增 src/engine.rs**

把当前 `src/main.rs` 里 `main()` 内从 `let primary = ...` 到循环结束的逻辑、以及 `handle_command`/`copy_selection` 两个函数,迁移为 `engine.rs` 的公开函数 `run(config: Config)`。完整内容:

```rust
//! 听写引擎主循环:热键 → 录音 → ASR →(LLM 整理/翻译/命令)→ 注入。
//! 命令行 bin 与 Tauri 应用共用此入口。

use std::{thread, time::Duration};

use arboard::Clipboard;
use crossbeam_channel::unbounded;

use crate::asr::Transcriber;
use crate::audio::Recorder;
use crate::config::Config;
use crate::corrector::Corrector;
use crate::foreground::foreground_process_name;
use crate::hotkey::{self, HotkeyAction};
use crate::inject::inject_text;
use crate::keys::vk_from_name;

/// 阻塞运行听写引擎(安装键盘钩子并进入消息/事件循环)。
pub fn run(config: Config) -> anyhow::Result<()> {
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
                    HotkeyAction::StopAndCommand => handle_command(&corrector, &raw),
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

fn handle_command(corrector: &Corrector, instruction: &str) {
    let selected = match copy_selection() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取选中文本失败: {e}");
            String::new()
        }
    };
    if selected.trim().is_empty() {
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
```

- [ ] **Step 2: lib.rs 导出 engine**

在 `src/lib.rs` 模块声明区加 `pub mod engine;`(与其他并列)。

- [ ] **Step 3: 旧 main.rs 改薄壳**

把 `src/main.rs` 全文替换为:
```rust
use voice_input::config::Config;

fn main() -> anyhow::Result<()> {
    let config = Config::load("config.toml")?;
    voice_input::engine::run(config)
}
```

- [ ] **Step 4: 构建 + 测试**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build; cargo test --lib`
Expected: 编译通过;34 lib 测试全绿(engine 是逻辑迁移,无新测试)。

- [ ] **Step 5: Commit**
```bash
git add src/engine.rs src/lib.rs src/main.rs && git commit -m "refactor: 听写循环提炼为 voice_input::engine::run"
```

---

## Task 2: 转为 Cargo workspace

**Files:** Modify `Cargo.toml`(根)

- [ ] **Step 1: 根 Cargo.toml 加 workspace 表**

在 `Cargo.toml` 顶部(`[package]` 之前)加入:
```toml
[workspace]
members = ["src-tauri"]
```
(根 package 自身仍是 workspace 成员;`src-tauri` 在 Task 3 创建。注意:先创建 `src-tauri` 后再 build,否则 workspace 缺成员会报错——本任务只改 Cargo.toml,Step 2 不 build,直接进 Task 3。)

- [ ] **Step 2: Commit**
```bash
git add Cargo.toml && git commit -m "chore: 仓库转为 Cargo workspace"
```

---

## Task 3: 脚手架 Tauri 2 应用 + React/Tailwind 前端

本任务以"生成 + 适配"为主。Tauri 2 的确切 crate/插件版本与 `tauri.conf.json` schema 以当前 crates.io / 官方模板为准;下方为目标结构,实施时按真实版本校正(用 `cargo add tauri@2` / npm 安装最新 2.x,遇 schema 不符按官方文档调整,**保持产品名 ByType 与目录结构**)。

**Files:** Create `package.json`、`vite.config.ts`、`tailwind.config.js`、`postcss.config.js`、`index.html`、`src-ui/main.tsx`、`src-ui/App.tsx`、`src-ui/index.css`、`tsconfig.json`、`src-tauri/Cargo.toml`、`src-tauri/build.rs`、`src-tauri/tauri.conf.json`、`src-tauri/src/main.rs`、`src-tauri/icons/*`、`.gitignore`(追加 node_modules/dist)

- [ ] **Step 1: 前端脚手架(根目录)**

创建 `package.json`:
```json
{
  "name": "bytype-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@vitejs/plugin-react": "^4.3.1",
    "autoprefixer": "^10.4.19",
    "postcss": "^8.4.38",
    "tailwindcss": "^3.4.10",
    "typescript": "^5.5.4",
    "vite": "^5.4.0"
  }
}
```

创建 `vite.config.ts`:
```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  root: ".",
  build: { outDir: "dist" },
  server: { port: 1420, strictPort: true },
  clearScreen: false,
});
```

创建 `tailwind.config.js`:
```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src-ui/**/*.{ts,tsx}"],
  theme: { extend: {} },
  plugins: [],
};
```

创建 `postcss.config.js`:
```js
export default { plugins: { tailwindcss: {}, autoprefixer: {} } };
```

创建 `tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true
  },
  "include": ["src-ui"]
}
```

创建 `index.html`:
```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ByType</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src-ui/main.tsx"></script>
  </body>
</html>
```

创建 `src-ui/index.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

创建 `src-ui/main.tsx`:
```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

创建 `src-ui/App.tsx`(占位首页):
```tsx
export default function App() {
  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-neutral-50 text-neutral-800 gap-3">
      <h1 className="text-2xl font-semibold">ByType</h1>
      <p className="text-sm text-neutral-500">
        正在后台运行。按住左 Win 说话即可输入。
      </p>
    </div>
  );
}
```

`.gitignore` 追加:
```
/node_modules
/dist
```

- [ ] **Step 2: 安装前端依赖并构建**

PowerShell:`npm install`;然后 `npm run build`
Expected:`dist/` 生成。若依赖版本冲突,放宽/更新到兼容的最新版后重试。

- [ ] **Step 3: src-tauri Rust 应用**

创建 `src-tauri/Cargo.toml`:
```toml
[package]
name = "bytype"
version = "0.1.0"
edition = "2021"

[lib]
name = "bytype_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-single-instance = "2"
voice_input = { path = ".." }
anyhow = "1"

[[bin]]
name = "bytype"
path = "src/main.rs"
```

创建 `src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

创建 `src-tauri/tauri.conf.json`(以 Tauri 2 schema 为准;字段按真实 CLI 校正):
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ByType",
  "version": "0.1.0",
  "identifier": "com.bytype.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "ByType",
        "width": 900,
        "height": 640,
        "visible": false
      }
    ],
    "trayIcon": {
      "id": "main",
      "iconPath": "icons/icon.png",
      "tooltip": "ByType"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png", "icons/icon.ico"]
  }
}
```

- [ ] **Step 4: 图标**

Tauri 需图标集。用 CLI 从一张 PNG 生成:准备一张 512x512 `app-icon.png`(可用纯色占位,放仓库根),运行
`npx tauri icon app-icon.png`(它会在 `src-tauri/icons/` 生成 icon.png/icon.ico 等)。
若没有现成 PNG,先用任意工具生成一张纯色 512x512 PNG。

- [ ] **Step 5: 提交脚手架(暂不含 main.rs 逻辑,Task 4 加)**

先放一个最小可编译 `src-tauri/src/main.rs`:
```rust
fn main() {
    bytype_lib::run();
}
```
并创建 `src-tauri/src/lib.rs`:
```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running ByType");
}
```

构建:`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build -p bytype`
Expected:Tauri 应用编译成功(首次拉 tauri 依赖较慢)。若 `tauri.conf.json` schema 报错,据 CLI 提示修正字段名后重试。

- [ ] **Step 6: Commit**
```bash
git add -A && git commit -m "feat: 脚手架 Tauri 2 + React/Tailwind 前端(ByType)"
```

---

## Task 4: 托盘 + 单实例 + 隐藏到托盘 + 启动引擎线程

**Files:** Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 实现 lib.rs**

把 `src-tauri/src/lib.rs` 替换为(Tauri 2 托盘/菜单 API 以当前版本为准,按编译报错适配同名项):
```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 第二次启动:聚焦主窗口
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            // 托盘菜单
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

            // 后台启动听写引擎
            std::thread::spawn(|| match voice_input::config::Config::load("config.toml") {
                Ok(cfg) => {
                    if let Err(e) = voice_input::engine::run(cfg) {
                        eprintln!("引擎退出: {e}");
                    }
                }
                Err(e) => eprintln!("读取 config.toml 失败: {e}"),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭按钮 → 隐藏到托盘,而非退出
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ByType");
}
```

- [ ] **Step 2: 构建**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build -p bytype`
Expected:编译通过。若 Tauri 2 的 `Menu`/`TrayIconBuilder`/`get_webview_window` 等 API 名称/签名在当前版本不同,据编译错误与 docs.rs/tauri 2 适配(保持:托盘有"设置/退出"、关闭隐藏到托盘、setup 起引擎线程)。

- [ ] **Step 3: Commit**
```bash
git add src-tauri/src/lib.rs && git commit -m "feat: 托盘菜单 + 单实例 + 关闭隐藏 + 启动引擎线程"
```

---

## Task 5: 全量验证 + 端到端(控制器/用户)

- [ ] **Step 1: workspace 构建 + lib 测试**

`$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"; cargo build; cargo test --lib`
Expected:整个 workspace 编译;34 lib 测试全绿。

- [ ] **Step 2: Tauri dev 手动验证(用户)**

控制器后台运行 `npm run tauri dev`(工作目录=项目根,需 config.toml 与 models/ 就位)。
用户验证:
- 托盘出现 ByType 图标,右键有"设置/退出"
- 按住左 Win 说话 → 文字粘进当前应用(引擎在 Tauri 内工作)
- Win+Alt 翻译、Win+Ctrl 选中命令仍可用
- 点托盘"退出"→ 进程结束
- (窗口默认隐藏;点"设置"弹出占位首页,关闭后回到托盘)

## 完成标准
- [ ] workspace 编译、34 lib 测试全绿。
- [ ] ByType 从托盘常驻,三热键听写在 Tauri 应用内可用。
- [ ] 关闭窗口隐藏到托盘;托盘退出有效;单实例防多开。
- [ ] 旧 `voice-input.exe` 仍可用(engine 复用未回归)。

## 备注(后续子项目)
G2 浮窗 / G3 提示音 / G4 设置+LLM测试 / G5 首启+模型管理 / G6 安装包+关于页。
