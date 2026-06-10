# ByType G5 — 首启向导 + 模型下载 + 依赖检测 设计文档

日期:2026-06-10
状态:已批准设计,待写实现计划

## 背景与目标

打包后的 ByType 首次在用户机器上运行时,通常**缺三样**:① `config.toml`(含 LLM 中转站信息);
② 语音识别模型(`model.onnx` 228MB + `tokens.txt`,太大塞不进安装包);③ 运行环境依赖
(onnxruntime 需要的 VC++ 运行时等)。G5 提供一个**首启向导**:检测缺什么、引导填配置、下载模型、
检测依赖,完成后让 ByType 正常后台运行。这是用户多次强调的重点。

## 已定决策(经 brainstorming 确认)

| 决策 | 选择 |
|------|------|
| 形态 | **独立首启向导**:未就绪时弹向导,分步引导;就绪后直接后台运行、不再弹 |
| 窗口归属 | **复用主窗口**:主窗口加载时按 `wizard_state` 分流渲染「向导」或「设置」;不新增窗口/vite 入口 |
| 完成后启动 | **当场启动引擎,不重启进程**:`finish_wizard` 写 config + 触发 `start_engine`(`OnceCell` 防重复) |
| 模型获取 | **在线下载为主 + 本地导入兜底** |
| 默认下载源 | **hf-mirror.com**(HuggingFace 国内镜像),URL 写进 config 可改 |
| 依赖检测 | 4 项:**VC++ 运行时** / **核心 dll** / **麦克风设备** / **麦克风隐私权限** |
| 致命 vs 警告 | 致命=VC++ 运行时 + 核心 dll(缺则引擎起不来);警告=麦克风(不挡完成) |
| LLM 步骤 | **可跳过**(模型才是听写硬依赖;不填则输出原始识别文本,日后可在设置补) |
| 向导步骤 | 欢迎 → 依赖检测 → LLM 配置 → 下载模型 → 完成 |

## 架构

### 就绪判定与启动分流(src-tauri `lib.rs`)

启动 `setup` 时判定「就绪」:

```
ready = config.toml 存在  AND  模型齐全(model_dir/model.onnx 与 tokens.txt 都在)
```

- **就绪** → `start_engine(app)` 启动引擎线程 + 窗口保持隐藏到托盘(现状不变)。
- **未就绪** → **不**启动引擎,`show()` + `set_focus()` 主窗口(承载向导)。

引擎启动逻辑抽成 `start_engine(app: &AppHandle)`,内部用 `OnceCell<()>` / `AtomicBool` 保证**只启动一次**
(防止 setup 已启动后 `finish_wizard` 再次启动,或向导完成后重复点)。`finish_wizard` 命令在写好 config +
模型就位后调用 `start_engine`,实现「当场启动、不重启进程」。

### 组件 / 文件

**核心 crate**
- `src/readiness.rs`(新):纯逻辑,可测。
  ```rust
  /// 模型文件是否齐全(model.onnx 与 tokens.txt 都存在且非空)。
  pub fn model_present(model_dir: &std::path::Path) -> bool;
  ```

**src-tauri**
- `src/wizard.rs`(新):向导后端命令模块。
  | 命令 | 行为 |
  |------|------|
  | `wizard_state() -> WizardState` | 返回 `{ ready, config_exists, model_present, model_dir }`,供前端分流 + 向导展示 |
  | `check_dependencies() -> Vec<DepCheck>` | 4 项检测,每项 `{ key, label, status: "ok"|"bad"|"warn", detail, fix_url? }` |
  | `download_model(app) -> Result<(), String>` | 从 config 的 `[model]` URL streaming 下载 model+tokens 到 `.part`,emit `bt:dl-progress`,完成校验+rename |
  | `cancel_download()` | 置取消标志;下载循环检测到后中止并删 `.part` |
  | `import_model(model_path, tokens_path) -> Result<(), String>` | 校验用户选的两个文件后复制到模型目录 |
  | `finish_wizard(llm) -> Result<(), String>` | `load_raw`(不存在→默认)合并向导 LLM 值后 `save_to`(存在用原路径,否则 exe 目录),不覆盖其它字段;空 `api_key` 则置 `llm.enabled=false`(避免无谓失败请求);再调 `start_engine` |
  | `open_external(url) -> Result<(), String>` | 用 explorer/start 打开 URL(VC++ 下载页 / `ms-settings:privacy-microphone`) |
- `src/lib.rs`(改):setup 就绪分流;`start_engine`(once);挂 wizard 命令;capability 给 main 窗口放开
  `event` 监听(已可)与新命令(自定义命令无需 capability)。

**前端 `src-ui/`**
- `main.tsx`(改):挂载前 `invoke("wizard_state")`,`ready` 渲染 `<App/>`(G4 设置),否则渲染 `<Wizard/>`。
- `src-ui/wizard/`(新):
  - `Wizard.tsx`:5 步壳(顶部步骤指示 + 内容区 + 底部按钮),`step` state 推进。
  - `WelcomeStep.tsx` / `DepsStep.tsx` / `LlmStep.tsx` / `DownloadStep.tsx` / `DoneStep.tsx`。
  - LLM 步复用 G4 的字段与「测试连接」(从 `src-ui/settings/` 引用 `testLlm`、共享小件/类型)。
  - `api.ts`:向导命令的 invoke 封装 + `bt:dl-progress` 事件监听。

### 配置变更(`src/config.rs`)

新增可选 `[model]` 段(向导/下载用),`#[serde(default)]`:

```toml
[model]
# 语音识别模型下载源(留空用内置默认 hf-mirror)。下载 int8 模型存为 model.onnx。
model_url  = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx"
tokens_url = "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt"
```

`ModelConfig { model_url: String, tokens_url: String }`,Default 填上述 hf-mirror 默认值。
**注意**:默认 URL 的仓库路径/文件名以官方 `csukuangfj/sherpa-onnx-sense-voice-...-2024-07-17` 为准,
真机首次下载时验证可达与文件名;若官方仓库结构不同,调整默认值(URL 可配置,风险可控)。
下载的 int8 模型**存为 `model.onnx`**(asr.rs 加载 `{model_dir}/model.onnx`),tokens 存为 `tokens.txt`。

## 依赖检测(4 项实现)

| 项 | 检测方法 | 级别 |
|----|---------|------|
| VC++ 运行时 | `LoadLibraryW("vcruntime140.dll")` 成功即装(失败再试 `vcruntime140_1.dll`);缺→给 aka.ms/vs/17/release/vc_redist.x64.exe 链接 | 致命 |
| 核心 dll | exe 目录下 `onnxruntime.dll`、`sherpa-onnx-c-api.dll` 存在 | 致命 |
| 麦克风设备 | `cpal` 默认 host 枚举输入设备,数量>0 | 警告 |
| 麦克风权限 | 读注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\Value`,非 "Allow" 则提示;读不到当作未知不报错 | 警告 |

致命项缺失时「下一步/完成」给出明确阻挡提示 + 修复链接 + 「重新检测」;警告项仅提示,不挡流程。
cpal 已是核心依赖;src-tauri 通过核心暴露的小函数或直接加 cpal 做枚举(实现时择简)。

## 模型下载(`download_model`)

- 用 `reqwest` blocking streaming(`Response::bytes_stream` 或逐块 `read`),边下边写 `model.onnx.part`。
- 每累积一定字节 `app.emit("bt:dl-progress", {file, received, total})`(total 取 Content-Length)。
- 先下 tokens.txt(小、快、能早暴露 URL 错误),再下 model.onnx(大)。
- 完成校验:文件大小 > 100MB(model)/ 非空(tokens);通过则 `.part` → 正式名。
- 取消(`cancel_download` 置 `AtomicBool`)或失败:删 `.part`,返回错误字符串给前端展示。
- 放在 `tauri::async_runtime::spawn_blocking` 里跑,不卡 UI。

## 数据流

```
启动 setup → readiness 判定
  就绪 → start_engine + 隐藏托盘(现状)
  未就绪 → show 主窗口
前端 main.tsx → wizard_state:ready?→ <App 设置> : <Wizard>
向导:欢迎 → 依赖检测(check_dependencies,可重测/修复链接)
      → LLM 配置(可跳过,测试连接复用 G4)
      → 下载模型(download_model + bt:dl-progress;失败→重试 / import_model)
      → finish_wizard(写 config.toml + start_engine)→ 完成屏 → 用户点完成 → 窗口隐藏,引擎已在跑
```

## 错误处理

| 情况 | 处理 |
|------|------|
| 下载失败(网络/404/磁盘满) | 错误提示 + 重试按钮 + 「导入本地文件」兜底;删 `.part` |
| 下载中取消 | 置标志中止 + 删 `.part` |
| 校验不通过(文件过小) | 当作失败处理,删 `.part`,提示重试/导入 |
| 导入文件无效(非 onnx/tokens) | 校验失败提示,不复制 |
| VC++ / 核心 dll 缺失 | 红✕ + 修复链接 + 重新检测;阻挡完成 |
| 麦克风缺失/无权限 | 黄!提示 + 设置链接;不挡完成 |
| config.toml 不存在/已存在 | finish_wizard 统一 load_raw(无则默认)+ 合并向导 LLM 后 save_to(无则落 exe 目录),只动 [llm] 不覆盖其它字段 |
| 引擎已启动后再调 finish | `start_engine` 的 once 守卫忽略二次启动 |

## 测试策略(沿用项目惯例)

- **TDD**:`readiness::model_present`(临时目录:两文件齐全→true;缺一个/空文件→false)。
- config 解析:`[model]` 段默认值与覆盖(往返)。
- **不**为下载/依赖检测/Tauri 命令写自动化测试(网络与 OS 集成,真机验证)。
- **真机端到端清单**:临时移走 `config.toml` + 模型 → 启动 → 向导弹出(主窗口);依赖检测 4 项显示正确
  (故意未装 VC++ 看红✕);LLM 填写 + 测试连接 / 跳过;真实从 hf-mirror 下载(228MB,含进度/取消)
  或「导入本地文件」;完成 → 引擎当场启动、按住热键出字(不重启进程);二次启动直接就绪不弹向导。

## 非目标(后续或不做)

- 模型多选/多语言模型切换(只下默认 SenseVoice int8)。
- 断点续传(失败重新下载整文件;`.part` 不复用)。
- 下载校验用 SHA256(只校验大小;真伪由能否被 SenseVoice 加载兜底)。
- 自动安装 VC++(只检测 + 给官方下载链接,用户自行安装)。
- 设置界面里的「模型」管理页(重新下载/换源 UI)—— 如需可后续补;G5 只做首启路径 + config 可改 URL。
- 安装包本身 —— 属 G6。
