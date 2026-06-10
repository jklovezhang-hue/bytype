# ByType G6 — 可视化安装包 + 第三方致谢 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 ByType 可分发——`tauri build` 产出当前用户 NSIS 安装程序(随包带引擎 DLL、crt-static 保证 exe 能启动),并在关于页补上第三方开源致谢,版本升至 1.0.0。

**Architecture:** 纯打包/发布层改动 + 静态 UI 文案,无新增运行时逻辑。引擎 DLL 走「路线 B」:发布脚本先 `cargo build --release` 产 DLL → 拷进 `src-tauri/runtime/` → `tauri build` 经 `bundle.resources` 把 `runtime/*.dll` 放到安装根目录(exe 同级)。crt-static 仅在发布脚本里经 `RUSTFLAGS` 启用,不污染日常 dev/test。

**Tech Stack:** Tauri 2 bundler(NSIS)、PowerShell 构建脚本、React/TS(关于页)、Cargo/JSON 配置。

**依据 spec:** `docs/superpowers/specs/2026-06-10-g6-installer-design.md`

**构建环境(每条 cargo / tauri 命令前先设;PowerShell):**
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

**关键纪律:** `cargo` / `npm run tauri build` 的成功与否**只看 `$LASTEXITCODE`(== 0 才算成功)**,绝不靠 stderr 有无输出判断——PowerShell 会把 cargo 的 stderr 包装成 NativeCommandError,看起来像失败其实没失败。

---

## 文件结构(改动总览)

| 文件 | 职责 | 动作 |
|------|------|------|
| `src-tauri/tauri.conf.json` | 版本号 + NSIS 打包配置(targets/publisher/copyright/描述/resources/nsis) | 改 |
| `Cargo.toml`(根) | workspace 根包版本号 | 改 |
| `src-tauri/Cargo.toml` | bytype 包版本号 | 改 |
| `src-ui/settings/changelog.ts` | 加 1.0.0 变更条目 | 改 |
| `.gitignore` | 忽略 `/src-tauri/runtime`(DLL 构建副本) | 改 |
| `scripts/build-installer.ps1` | 一条命令发布构建(env + crt-static + cargo build + 拷 DLL + tauri build) | 新建 |
| `src-ui/settings/api.ts` | 加 `openExternal()` 命令包装 | 改 |
| `src-ui/settings/AboutPage.tsx` | 占位文案 → 第三方开源致谢清单 | 改 |

无新增纯逻辑 → 无新增单元测试;现有核心测试须仍全绿。

---

## Task 1: 版本号升 1.0.0 + changelog 条目

**Files:**
- Modify: `src-tauri/tauri.conf.json:4`
- Modify: `Cargo.toml:6`
- Modify: `src-tauri/Cargo.toml:3`
- Modify: `src-ui/settings/changelog.ts`

- [ ] **Step 1: 改 tauri.conf.json 版本号**

把第 4 行 `"version": "0.1.0",` 改为:
```json
  "version": "1.0.0",
```

- [ ] **Step 2: 改根 Cargo.toml 版本号**

把 `Cargo.toml` 第 6 行 `version = "0.1.0"` 改为:
```toml
version = "1.0.0"
```

- [ ] **Step 3: 改 src-tauri/Cargo.toml 版本号**

把 `src-tauri/Cargo.toml` 第 3 行 `version = "0.1.0"` 改为:
```toml
version = "1.0.0"
```

- [ ] **Step 4: changelog.ts 顶部加 1.0.0 条目**

`src-ui/settings/changelog.ts` 的 `CHANGELOG` 数组里,在现有 `0.1.0` 对象**之前**插入新对象(新版本在最前):
```ts
  {
    version: "1.0.0",
    date: "2026-06-10",
    items: [
      "首次运行向导:依赖检测 + 模型在线下载 / 本地导入",
      "可视化安装程序(正式发布)",
      "关于页第三方开源致谢",
    ],
  },
```
保留其后的 `0.1.0` 对象不动。

- [ ] **Step 5: 校验 Cargo 清单可解析 + 前端可构建**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo metadata --no-deps --format-version 1 > $null; "META_EXIT=$LASTEXITCODE"
npm run build; "BUILD_EXIT=$LASTEXITCODE"
```
预期:`META_EXIT=0`(两个 Cargo.toml 版本号合法)且 `BUILD_EXIT=0`(changelog.ts 语法正确,vite 构建通过)。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/tauri.conf.json Cargo.toml src-tauri/Cargo.toml src-ui/settings/changelog.ts
git commit -m "chore(g6): 版本升至 1.0.0 + changelog 条目"
```

---

## Task 2: NSIS 打包配置 + gitignore

**Files:**
- Modify: `src-tauri/tauri.conf.json`(`bundle` 段,当前第 37–41 行)
- Modify: `.gitignore`

- [ ] **Step 1: 替换 bundle 段**

把 `src-tauri/tauri.conf.json` 现有 `bundle` 对象整体替换为(注意:**纯 JSON,不能有注释**):
```json
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/icon.png", "icons/icon.ico"],
    "publisher": "Yong Zhang",
    "copyright": "© 2026 Yong Zhang. 保留所有权利。",
    "shortDescription": "按住说话的语音输入工具",
    "longDescription": "ByType——按住热键说话,本地 SenseVoice 识别 + LLM 整理,松手即把文字注入当前应用。",
    "resources": { "runtime/*.dll": "./" },
    "windows": {
      "nsis": {
        "installMode": "currentUser",
        "languages": ["SimpChinese", "English"],
        "displayLanguageSelector": false
      }
    }
  }
```

- [ ] **Step 2: .gitignore 加 runtime/**

在 `.gitignore` 末尾追加一行:
```
/src-tauri/runtime
```

- [ ] **Step 3: 校验 JSON 合法**

```powershell
Get-Content src-tauri/tauri.conf.json -Raw -Encoding UTF8 | ConvertFrom-Json > $null; "JSON_EXIT=$LASTEXITCODE"
```
预期:`JSON_EXIT=0`(无解析错误)。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/tauri.conf.json .gitignore
git commit -m "feat(g6): NSIS 当前用户打包配置 + 引擎 DLL resources + gitignore runtime/"
```

---

## Task 3: 发布构建脚本

**Files:**
- Create: `scripts/build-installer.ps1`

- [ ] **Step 1: 写脚本**

新建 `scripts/build-installer.ps1`,内容如下(整段)。脚本三步:release 构建产 DLL → 拷 DLL 到 `src-tauri/runtime/` → `tauri build` 打包;每步用 `$LASTEXITCODE` 判定成功。

```powershell
#requires -version 5
# ByType 发布构建:产出当前用户 NSIS 安装包(target/release/bundle/nsis/*-setup.exe)。
# 用法(任意目录均可):  powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
$ErrorActionPreference = "Stop"

# 切到仓库根(本脚本在 scripts/ 下,根 = 上一级)
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

# 构建环境:cargo / libclang 不在默认 PATH;crt-static 仅本次发布生效(不写 .cargo/config,避免污染 dev/test)
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:RUSTFLAGS = "-C target-feature=+crt-static"

# 步骤 1:release 构建。sherpa-rs 把引擎 DLL 产到 target/release/;bytype.exe 静态链接 CRT(缺 VC++ 也能启动)
Write-Host "[1/3] cargo build --release ..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build 失败 (exit=$LASTEXITCODE)" }

# 步骤 2:拷引擎 DLL 到 src-tauri/runtime/(bundle.resources 的稳定来源;多带无害)
Write-Host "[2/3] copy engine DLLs -> src-tauri/runtime/ ..." -ForegroundColor Cyan
$runtime = Join-Path $repo "src-tauri\runtime"
New-Item -ItemType Directory -Force -Path $runtime | Out-Null
$src = Join-Path $repo "target\release"
$patterns = @("onnxruntime*.dll", "sherpa-onnx*.dll", "cargs.dll")
$copied = 0
foreach ($p in $patterns) {
    foreach ($f in (Get-ChildItem -Path $src -Filter $p -ErrorAction SilentlyContinue)) {
        Copy-Item $f.FullName -Destination $runtime -Force
        $copied++
    }
}
if ($copied -eq 0) { throw "未在 target/release 找到任何引擎 DLL,无法打包" }
Write-Host "    copied $copied DLL(s)" -ForegroundColor Green

# 步骤 3:打包。RUSTFLAGS 不变 → 此处 cargo 构建命中步骤 1 缓存(增量空转),打包器收 runtime/*.dll
Write-Host "[3/3] npm run tauri build ..." -ForegroundColor Cyan
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build 失败 (exit=$LASTEXITCODE)" }

$setup = Get-ChildItem -Path (Join-Path $repo "target\release\bundle\nsis") -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
         Select-Object -First 1
if (-not $setup) { throw "未找到 NSIS 安装包产物" }
Write-Host "DONE ✓ installer: $($setup.FullName)" -ForegroundColor Green
```

- [ ] **Step 2: 语法校验(不执行重构建)**

仅解析脚本语法,确认无语法错(不跑完整构建,那放到 Task 5):
```powershell
[void][System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path scripts/build-installer.ps1), [ref]$null, [ref]$null); "PARSE_EXIT=$LASTEXITCODE"
```
预期:`PARSE_EXIT=0`,无异常抛出。

- [ ] **Step 3: 提交**

```powershell
git add scripts/build-installer.ps1
git commit -m "feat(g6): 发布构建脚本(crt-static + 拷 DLL + tauri build)"
```

---

## Task 4: 关于页第三方致谢 + openExternal 包装

**Files:**
- Modify: `src-ui/settings/api.ts`
- Modify: `src-ui/settings/AboutPage.tsx`

- [ ] **Step 1: api.ts 加 openExternal**

在 `src-ui/settings/api.ts` 末尾(`openConfigDir` 那行之后)追加一行,沿用文件现有的 `const + invoke<...>` 风格:
```ts
export const openExternal = (url: string) => invoke<void>("open_external", { url });
```
(`open_external` 命令在 G5 已注册且为应用级命令,主窗口可直接调用,无需改后端 / capability。)

- [ ] **Step 2: AboutPage.tsx 引入 openExternal + 致谢常量**

把 `src-ui/settings/AboutPage.tsx` 第 3 行:
```ts
import { openConfigDir } from "./api";
```
改为:
```ts
import { openConfigDir, openExternal } from "./api";
```

并在第 7 行 `const EMAIL = "jklover2025@outlook.com";` 之后新增致谢常量:
```ts
const CREDITS: { name: string; license: string; url: string }[] = [
  { name: "ONNX Runtime", license: "MIT", url: "https://github.com/microsoft/onnxruntime" },
  { name: "sherpa-onnx", license: "Apache-2.0", url: "https://github.com/k2-fsa/sherpa-onnx" },
  { name: "SenseVoice 语音识别模型", license: "见上游许可", url: "https://github.com/FunAudioLLM/SenseVoice" },
  { name: "Tauri", license: "Apache-2.0 / MIT", url: "https://github.com/tauri-apps/tauri" },
  { name: "React", license: "MIT", url: "https://react.dev" },
  { name: "Tailwind CSS", license: "MIT", url: "https://tailwindcss.com" },
  { name: "Vite", license: "MIT", url: "https://vitejs.dev" },
];
```

- [ ] **Step 3: 用致谢 Section 替换占位行**

把 `src-ui/settings/AboutPage.tsx` 的占位行(原第 73 行):
```tsx
      <p className="text-xs text-neutral-400">第三方开源组件致谢将在正式安装版中提供。</p>
```
替换为:
```tsx
      <Section title="第三方开源致谢">
        <ul className="flex flex-col gap-1.5">
          {CREDITS.map((c) => (
            <li
              key={c.name}
              className="text-sm text-neutral-700 dark:text-neutral-300 flex items-center gap-2 flex-wrap"
            >
              <span>{c.name}</span>
              <span className="text-xs text-neutral-400">{c.license}</span>
              <button
                type="button"
                onClick={() => openExternal(c.url)}
                className="text-xs text-blue-600 dark:text-blue-400 hover:underline"
              >
                打开
              </button>
            </li>
          ))}
        </ul>
        <p className="text-xs text-neutral-400 mt-2">
          以及 cpal、arboard、reqwest、serde 等众多 Rust / Node 开源库(MIT / Apache-2.0)。
        </p>
      </Section>
```
(`Section` 组件已在文件第 3 块 import 自 `./widgets`,无需新增 import。)

- [ ] **Step 4: 前端构建校验**

```powershell
npm run build; "BUILD_EXIT=$LASTEXITCODE"
```
预期:`BUILD_EXIT=0`(TS/JSX 无错,两个 HTML 入口都打包成功)。

- [ ] **Step 5: 提交**

```powershell
git add src-ui/settings/api.ts src-ui/settings/AboutPage.tsx
git commit -m "feat(g6): 关于页第三方开源致谢 + openExternal 包装"
```

---

## Task 5: 构建安装包 + 真机端到端验证

> 本任务前半段(产出安装包)由执行方运行;后半段(安装、启动、向导、关于页)为真机验收清单,由用户在本机完成。打包/OS 集成不写自动化测试,沿用项目惯例。

**Files:** 无(仅运行与验证)

- [ ] **Step 1: 现有核心测试回归**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo test -p voice-input --lib; "TEST_EXIT=$LASTEXITCODE"
```
预期:`TEST_EXIT=0`,52 passed(G6 未动核心逻辑)。

- [ ] **Step 2: 跑发布脚本产出安装包**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-installer.ps1
```
判定:**只看脚本是否以 `DONE ✓ installer: ...` 收尾且未抛错**(脚本内部已用 `$LASTEXITCODE` 把关 cargo / tauri build)。
- 若 `cargo build --release` 因 crt-static 报错(如 proc-macro 相关链接错误):记录确切错误并上报——回退方案是不在发布脚本设 `crt-static`、改为在安装包内置 vc_redist(属设计层调整,需回到 spec 决策),不要擅自换方案。
- 若报 `runtime/*.dll → "./"` 资源未匹配 / 未落到安装根:回退为在 `tauri.conf.json` 的 `resources` 里逐个 DLL 显式映射(键为 `runtime/onnxruntime.dll` 等,值为 `"./"`),重跑脚本。

- [ ] **Step 3: 确认产物**

```powershell
Get-ChildItem target/release/bundle/nsis/*-setup.exe | Select-Object Name, Length
```
预期:存在 `ByType_1.0.0_x64-setup.exe`(体积约几 MB——不含模型)。

- [ ] **Step 4: 真机安装与运行(用户)**

清单(逐条确认):
1. 双击 `ByType_1.0.0_x64-setup.exe` → 安装界面为中文、**无管理员提权弹窗** → 装到 `%LOCALAPPDATA%\ByType`。
2. 打开安装目录,确认 `bytype.exe` 同级有引擎 DLL(`onnxruntime.dll`、`sherpa-onnx-c-api.dll` 等)。
3. 启动 ByType(理想在未装 VC++ 的环境)→ `bytype.exe` 能启动 → 弹出首启向导。
4. 向导依赖检测:缺 VC++ 时该项显✕ + 链接;点链接装好后「重新检测」转✓。
5. 从 hf-mirror 下载模型(或本地导入)→ 完成 → 按住热键能出字。
6. 托盘菜单(设置/退出)、录音浮窗、开机自启 从安装位置均正常。
7. 打开设置 → 关于页:版本显示 **v1.0.0**;「第三方开源致谢」清单可见;点任一「打开」按钮弹出系统浏览器到对应项目。
8. 从「应用和功能」卸载 → 程序被移除(autostart 残留 HKCU Run 键为已知限制,可忽略)。

- [ ] **Step 5: 标记 G6 完成**

真机清单全部通过后,本阶段实现完成,转入 finishing-a-development-branch 合并 `g6-installer`。

---

## 自查(写完计划回看 spec)

- **spec 覆盖**:NSIS 当前用户打包(T2)、crt-static + 发布脚本(T3)、引擎 DLL 路线 B(T2 resources + T3 拷贝)、版本 1.0.0(T1)、changelog(T1)、关于页致谢 + openExternal(T4)、真机验收清单含已知限制(T5)——逐条有归属,无遗漏。
- **占位扫描**:无 TBD/TODO;致谢协议为确定值,SenseVoice 显示「见上游许可」为实际展示文案而非占位。
- **类型一致**:`openExternal(url: string)`(api.ts 定义)↔ AboutPage 调用 `openExternal(c.url)` 一致;`CREDITS` 字段 `{name,license,url}` 与渲染中 `c.name/c.license/c.url` 一致;命令名 `open_external` 与后端 G5 注册一致。
