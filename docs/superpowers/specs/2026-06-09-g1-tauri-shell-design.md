# ByType G1 — Tauri 外壳基座 设计文档

日期:2026-06-09
状态:已批准设计,待写实现计划

## 背景与目标

把现有"无头命令行听写工具"升级为 **Tauri 2 桌面应用 ByType** 的**地基**:托盘常驻、
后端听写引擎在 Tauri 内运行,功能与现命令行版等价(三热键听写)。**本子项目只做地基**,
不含浮窗/提示音/设置界面/首启向导/安装包(各自后续子项目)。

这是 GUI 阶段(G1..G6)的第一步,后续子项目都建立在此结构上。

## 已定决策(本 GUI 阶段全局)

| 决策 | 选择 |
|------|------|
| 框架 | Tauri 2(Rust 后端 + WebView 前端) |
| 前端栈 | React + TypeScript + Tailwind + Vite |
| 交互模型 | 按住说话(保留),浮窗鼠标可取消(G2 实装) |
| 模型下载 | 首启向导后台下载(G5) |
| 产品名 | ByType |
| 工具链 | Node v24 / npm 11 已装;Tauri CLI 作为 npm dev 依赖 |

## 架构

仓库改为 **Cargo workspace**:

```
voice-input/                      (仓库根)
├── Cargo.toml                    # [workspace] + 根 package: voice_input(lib,核心)
├── src/                          # 现有核心:config/asr/corrector/hotkey/audio/inject/keys/foreground
│   └── engine.rs                 # 新增:听写引擎主循环(从旧 main.rs 提炼,供复用)
├── src/main.rs                   # 旧命令行 bin,改为调用 voice_input::engine::run(保留作回归)
├── examples/                     # 保留
├── src-tauri/                    # 新增:ByType Tauri 应用
│   ├── Cargo.toml                # 依赖 voice_input { path = ".." } + tauri 2
│   ├── tauri.conf.json           # productName=ByType, identifier, 窗口配置
│   ├── build.rs
│   ├── icons/                    # 占位图标集
│   └── src/main.rs               # Tauri 入口:托盘 + 单实例 + setup 启动引擎线程
├── index.html                    # 前端入口
├── package.json                  # React/Vite/Tailwind/@tauri-apps/cli
├── vite.config.ts
├── tailwind.config.js
└── src-ui/                       # React 前端源码(占位首页)
```

> 前端源码放 `src-ui/`(避免与 Rust 的 `src/` 冲突);Vite 输出到 `dist/`,
> `tauri.conf.json` 的 `frontendDist` 指向 `../dist`。

### 模块职责

- **`voice_input::engine`(新增,lib)**:`pub fn run(config: Config) -> anyhow::Result<()>`,
  内含现 `main.rs` 的整条循环(起热键钩子线程 + 接收 HotkeyAction + 录音/ASR/纠错/翻译/命令/
  注入 + 前台风格 + copy_selection/handle_command)。**纯逻辑迁移,不改行为**。
  - 旧 `src/main.rs` 改为:`Config::load("config.toml")` → `voice_input::engine::run(config)`。
  - 命令模式的 `copy_selection`/`handle_command` 一并迁入 engine(它们依赖 windows/arboard,已在 lib 依赖内)。
- **`src-tauri/src/main.rs`(新增,Tauri 应用)**:
  - 初始化 Tauri 2,注册 `tauri-plugin-single-instance`(防多开)。
  - 托盘图标 + 菜单:**设置**(G1 先弹出/聚焦主窗口占位)、**退出**(真正退出)。
  - `setup`:读 `config.toml`,在后台线程 `std::thread::spawn` 跑 `voice_input::engine::run(config)`;
    线程 panic/退出仅记录,不拖垮 UI。
  - 主窗口:加载 React 前端(占位首页,显示"ByType 已运行,可用热键听写");
    关闭按钮 → 隐藏到托盘(`WindowEvent::CloseRequested` 阻止默认、hide)。启动时可隐藏。
- **前端(`src-ui/`)**:React + Tailwind 占位首页一张,显示运行状态文字。真正界面在 G2/G4/G5。

## 数据流(G1)

```
ByType 启动 → Tauri setup → 读 config.toml → 线程跑 engine::run
   engine 内:安装键盘钩子 → 监听三热键 → 录音→ASR→(LLM)→注入(与现命令行版一致)
托盘菜单 退出 → 结束进程;关闭窗口 → 隐藏到托盘
```

## 命名

- `tauri.conf.json`:`productName = "ByType"`,`identifier = "com.bytype.app"`,主窗口 `title = "ByType"`。
- 托盘 tooltip:ByType。

## 错误处理

| 情况 | 处理 |
|------|------|
| config.toml 缺失/解析失败 | 托盘通知 + 主窗口提示;引擎不启动(G5 首启向导会改善) |
| 引擎线程出错退出 | 记录日志(eprintln 暂存),不崩 UI |
| WebView2 运行时缺失 | G6 安装器处理;G1 假定开发机已有 |

## 测试策略

- 现有 lib 单测(34)全绿;`engine` 迁移后行为不变(靠现有单测 + 手动端到端守护)。
- `cargo build`(workspace)通过;`npm install` 成功。
- `npm run tauri dev` 能起托盘 + 后台听写可用(手动):按 Win 说话能粘字、托盘退出有效。
- 不为 Tauri/UI 写自动化测试(G1 是地基,手动验证为主)。

## 非目标(后续子项目)

- 录音浮窗 + 波形 + ✕/✓ 取消(G2)
- 提示音(G3)
- 设置 / LLM 配置 + 测试按钮界面(G4)
- 首启向导 + 模型下载管理 + 依赖检测(G5)
- 可视化安装包 + 关于/版权页(G6)
