# ByType G6 — 可视化安装包 + 第三方开源致谢 设计文档

日期:2026-06-10
状态:已批准设计,待写实现计划

## 背景与目标

ByType 前五个 GUI 阶段(G1 Tauri 壳 / G2 浮窗 / G3 提示音 / G4 设置 / G5 首启向导)已完成并合并。
G6 是最后一个阶段,让它**可分发**:

1. `tauri build` 产出**可视化 NSIS 安装程序**(.exe),当前用户安装、免管理员,装完即可运行 G5 向导。
2. 关于页把占位文案换成真·**第三方开源致谢清单**。
3. 顺带把版本号升到 **1.0.0**(首个可分发版本)。

语音模型(`model.onnx` 228MB + `tokens.txt`)**不进安装包**——首次运行由 G5 向导在线下载或本地导入(已实现)。

## 已定决策(经 brainstorming 确认)

| 决策 | 选择 |
|------|------|
| 安装方式 | **当前用户 + 仅 NSIS**:装到 `%LOCALAPPDATA%\ByType`,免管理员;exe 目录可写,G5 写配置/下模型/自启全部正常 |
| VC++ 运行时 | **exe 静态链接 CRT(crt-static)+ 沿用 G5 向导检测**:bytype.exe 不再依赖 vcruntime140.dll,任何机器都能启动并弹向导;向导检测到 onnxruntime 所需 VC++ 缺失时给官方链接引导 |
| ByType 授权 | **专有 / 保留所有权利**:关于页写 © Yong Zhang;NSIS 不显许可协议页 |
| 第三方致谢 | **手工整理核心清单**:主要组件名 + 协议 + 链接 |
| 引擎 DLL 随包机制 | **路线 B**:发布脚本先 `cargo build --release` 产 DLL → 拷进 `src-tauri/runtime/` → `tauri build` 引用 `runtime/*.dll` |

## 架构

### 引擎 DLL 随包(路线 B)

引擎硬依赖以下 DLL,由 sherpa-rs 构建脚本产到 workspace 根 `target/release/`,**必须与 bytype.exe 同目录**
(Windows 加载器从 exe 所在目录查找导入的 DLL),否则装完引擎起不来——G5 的「核心 dll」就绪检测正好会拦下:

- `onnxruntime.dll`
- `onnxruntime_providers_shared.dll`
- `sherpa-onnx-c-api.dll`
- `sherpa-onnx-cxx-api.dll`
- `cargs.dll`

为求完整,按通配 `onnxruntime*.dll`、`sherpa-onnx*.dll`、`cargs.dll` 全拷(多带无害)。

**机制**:在 `src-tauri/runtime/`(新建,gitignore)放这些 DLL 的稳定副本,`tauri.conf.json` 的
`bundle.resources` 把 `runtime/*.dll` 映射到安装根目录(`"./"`),NSIS 打包时随包放到 exe 同目录。
DLL 副本由发布脚本在 `tauri build` 前从 `target/release/` 拷入。

> 真机验证项:打包后检查这些 DLL 是否确实落在安装目录里 bytype.exe 同级,且装完能识别。若 `runtime/*.dll → "./"`
> 通配+映射未把文件放到安装根目录(Tauri 行为差异),回退为在 `bundle.resources` 中逐个 DLL 显式映射。

### crt-static(仅发布构建)

为让 bytype.exe 在缺 VC++ 运行时的机器上也能启动(从而弹出向导、由 G5 依赖检测引导装 VC++),
发布构建用 `-C target-feature=+crt-static` 静态链接 CRT。

**只在发布脚本里通过环境变量 `RUSTFLAGS` 启用,不写入 `.cargo/config.toml`**——避免污染日常 `cargo test` /
`tauri dev`(它们继续动态链接、构建更快)。注意:改 `RUSTFLAGS` 会使 cargo 指纹变化,触发一次 release 全量重编。

crt-static 只影响 bytype.exe 自身;onnxruntime.dll 等是预编译的动态 CRT C++ 模块,跨 C ABI 调用不受影响,
但它们运行时**仍需** VC++ 运行时 → 由 G5 向导检测 + 链接引导用户安装(本就是选定方案)。

### 发布脚本 `scripts/build-installer.ps1`(新)

一条命令产出安装包:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:RUSTFLAGS = "-C target-feature=+crt-static"   # 仅本次发布构建静态链接 CRT

# 步骤 1:release 构建,sherpa-rs 产引擎 DLL 到 target/release/
cargo build --release

# 步骤 2:拷引擎 DLL 到 src-tauri/runtime/(bundle.resources 的稳定来源)
#   New-Item -ItemType Directory -Force src-tauri/runtime
#   Copy-Item target/release/onnxruntime*.dll, target/release/sherpa-onnx*.dll, target/release/cargs.dll -> src-tauri/runtime/

# 步骤 3:打包(cargo 增量空转,打包器取 runtime/*.dll)
npm run tauri build

# 产物:target/release/bundle/nsis/ByType_1.0.0_x64-setup.exe
```

### 打包配置(`src-tauri/tauri.conf.json`)

```jsonc
"version": "1.0.0",
"bundle": {
  "active": true,
  "targets": ["nsis"],                       // 从 "all" 收窄,只出 NSIS
  "icon": ["icons/icon.png", "icons/icon.ico"],
  "publisher": "Yong Zhang",
  "copyright": "© 2026 Yong Zhang. 保留所有权利。",
  "shortDescription": "按住说话的语音输入工具",
  "longDescription": "ByType——按住热键说话,本地 SenseVoice 识别 + LLM 整理,松手即把文字注入当前应用。",
  "resources": { "runtime/*.dll": "./" },
  "windows": {
    "nsis": {
      "installMode": "currentUser",          // 装到 %LOCALAPPDATA%\ByType,免管理员、exe 目录可写
      "languages": ["SimpChinese", "English"],
      "displayLanguageSelector": false
    }
  }
}
```

- 不设 `nsis.license` → 不显许可协议页(选定:专有 / 不显)。
- WebView2 沿用 Tauri NSIS 默认 `downloadBootstrapper`(缺 WebView2 时安装期联网拉取;Win10/11 一般自带)。

### 版本号

三处版本号统一为 **1.0.0**:`src-tauri/tauri.conf.json` 的 `version`、根 `Cargo.toml`、`src-tauri/Cargo.toml`。
关于页版本号经 `getVersion()`(读 tauri.conf.json)自动跟随,无需改前端。

### 变更日志(`src-ui/settings/changelog.ts`)

`CHANGELOG` 数组顶部加 1.0.0 条目(日期 2026-06-10),涵盖 G5/G6 新增:

- 首次运行向导:依赖检测 + 模型在线下载 / 本地导入
- 可视化安装程序(正式发布)
- 关于页第三方开源致谢

保留既有 0.1.0 条目在其下。

### 关于页第三方致谢(`src-ui/settings/AboutPage.tsx`)

把现有占位行(`第三方开源组件致谢将在正式安装版中提供。`)替换为 `<Section title="第三方开源致谢">`,
内含手工清单,每条一行:`名称 — 协议 — 打开`(「打开」按钮调用 `openExternal(url)`,https 已在
`open_external` 命令白名单内放行)。清单(协议字串实现时逐一核对各上游 LICENSE):

| 组件 | 协议 | 链接 |
|------|------|------|
| ONNX Runtime | MIT | https://github.com/microsoft/onnxruntime |
| sherpa-onnx | Apache-2.0 | https://github.com/k2-fsa/sherpa-onnx |
| SenseVoice 语音识别模型 | 见上游 | https://github.com/FunAudioLLM/SenseVoice |
| Tauri | Apache-2.0 / MIT | https://github.com/tauri-apps/tauri |
| React | MIT | https://react.dev |
| Tailwind CSS | MIT | https://tailwindcss.com |
| Vite | MIT | https://vitejs.dev |

末尾一行说明:"以及 cpal、arboard、reqwest、serde 等众多 Rust / Node 开源库(MIT / Apache-2.0)。"

`src-ui/settings/api.ts` 加 `openExternal(url: string)` 包装(`invoke("open_external", { url })`)。
`open_external` 命令在 G5 已注册且为应用级命令,主窗口可直接调用(向导本就跑在主窗口、已在用它),
**无需改后端、无需改 capability**。

## 数据流(打包 → 首启)

```
开发者:scripts/build-installer.ps1
  → cargo build --release(产引擎 DLL + crt-static bytype.exe)
  → 拷 DLL 到 src-tauri/runtime/
  → npm run tauri build → NSIS setup.exe
用户:运行 setup.exe(当前用户,免管理员)→ 装到 %LOCALAPPDATA%\ByType(exe + 引擎 DLL)
  → 启动 bytype.exe(crt-static,缺 VC++ 也能起)
  → G5 readiness 判定未就绪 → 弹首启向导
  → 依赖检测(VC++ 缺则✕+aka.ms 链接;装好复检)→ LLM(可跳过)→ 下载/导入模型 → 完成、引擎当场启动
  → 二次启动:就绪 → 后台托盘,不再弹向导
```

## 错误处理 / 已知限制(本期写明,不在本期解决)

| 情况 | 说明 |
|------|------|
| 无代码签名 | 首次运行 SmartScreen「未知发布者」警告(更多信息 → 仍要运行)。无证书,G6 不做签名。 |
| WebView2 缺失 | NSIS 默认 downloadBootstrapper 安装期联网拉取;Win10/11 一般自带。 |
| onnxruntime 需 VC++ | crt-static 让 exe 能启动,但 ASR 仍需 VC++ → 向导检测 + 链接引导(选定方案)。 |
| 卸载残留 autostart | 卸载不清理 autostart 的 HKCU Run 键(指向已删 exe,Windows 静默忽略)。可选加 NSIS 卸载钩子清理,优先级低,本期 YAGNI 不做。 |
| `runtime/*.dll → "./"` 映射行为差异 | 真机验证 DLL 落点;不达预期则在 resources 逐个 DLL 显式映射(见上)。 |

## 测试策略

- **无新增纯逻辑** → 不加单元测试(G6 全是打包配置 + 发布脚本 + 静态 UI 文案 + 版本号)。
- 现有核心测试须仍全绿(回归):`cargo test -p voice-input --lib`(预期 52 passed)。
- **真机端到端验收清单**:
  1. 跑 `scripts/build-installer.ps1`,确认 `target/release/bundle/nsis/ByType_1.0.0_x64-setup.exe` 产出。
  2. 运行 setup.exe → 当前用户安装、无管理员提示 → 装到 `%LOCALAPPDATA%\ByType`。
  3. 确认安装目录里 bytype.exe 同级有全部引擎 DLL。
  4. 启动(理想在缺 VC++ 的环境)→ bytype.exe 能起 → 弹首启向导。
  5. 依赖检测:VC++ 缺失显✕ + 链接;装好后「重新检测」转✓。
  6. 从 hf-mirror 下载模型(或本地导入)→ 完成 → 按住热键出字。
  7. 托盘菜单 / 录音浮窗 / 开机自启从安装位置均正常。
  8. 关于页显示 v1.0.0 + 第三方致谢清单;点任一致谢链接打开系统浏览器。
  9. 卸载:从「应用和功能」卸载,程序移除(autostart 残留键为已知限制,可忽略)。

## 非目标(后续或不做)

- 代码签名 / 证书(无证书,SmartScreen 警告为已知限制)。
- 自动更新(updater)。
- MSI 安装包(只出 NSIS)。
- 内置 vc_redist 自动安装(选定 crt-static + 向导引导路线)。
- 迁移 config.toml / 模型存储位置(当前用户安装已让 exe 目录可写,无需迁)。
- 卸载时清理 autostart 残留键(可选,本期不做)。
- 设置界面「模型管理」页(G5 非目标,延续)。
