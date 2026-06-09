# ByType G2 — 录音浮窗(波形药丸)设计文档

日期:2026-06-10
状态:已批准设计,待写实现计划

## 背景与目标

在 G1(Tauri 外壳 + 后台听写引擎)之上,加一个**录音可视提示**:按住热键录音时,屏幕
**底部正中**浮出一个**半透明小药丸**(数字秒计时 + 动画波形);**录制中途可取消**
(鼠标点药丸 / 按 Esc),取消时**跳过 ASR+LLM**;松手后药丸显示处理 → 成功/失败反馈再淡出。

**本子项目只做录音浮窗**。提示音(G3)、设置界面(G4)、首启向导/模型下载(G5)、
安装包/版权页(G6)均为后续子项目。

## 已定决策(经 brainstorming 可视化确认)

| 决策 | 选择 |
|------|------|
| 药丸构成 | **数字秒计时(纯秒、无分钟、无单位,如「3」)+ 固定动画波形** |
| 波形 | **固定动画**(不随真实音量;实时音量列为后续增强) |
| 取消手势 | **鼠标点药丸任意处** 或 **按 Esc**,二者同效;取消即**跳过 LLM** |
| 松手后表现 | 转圈处理 → **闪一下绿 ✓** → 淡出(选项②) |
| 失败表现 | 闪一下红 ✕ → 淡出(不静默) |
| 适用热键 | 识别 / 翻译 / 命令 **三者统一弹同一个药丸**,**不加模式标签** |
| 焦点 | 浮窗**绝不抢焦点**(否则注入目标错位) |
| 显示器 | v1 仅**主屏**底部正中(跟随焦点屏列为后续增强) |
| 架构 | 核心引擎加 **Observer trait**;Tauri 侧实现观察者驱动独立浮窗(方案①) |

## 架构(方案①:Observer trait + 独立浮窗)

核心 `voice_input` crate **保持不依赖 Tauri**。`engine::run` 增加一个观察者参数,在录音
状态变化点回调;CLI 传空实现,Tauri 传驱动浮窗的实现。取消经一个 `ControlHandle` 注入。

### 核心新增 API(`src/engine.rs`)

```rust
/// 浮窗要展示的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState { Recording, Processing, Done, Cancelled, Failed }

/// 引擎状态观察者(默认空实现,CLI 用)。
pub trait EngineObserver: Send + Sync {
    /// 引擎就绪后回调一次,交回可注入取消的句柄。
    fn on_ready(&self, _control: ControlHandle) {}
    /// 录音/处理状态变化。
    fn on_state(&self, _state: OverlayState) {}
}

/// 交给 GUI 的取消句柄(内含动作发送端的克隆)。
pub struct ControlHandle { tx: crossbeam_channel::Sender<crate::hotkey::HotkeyAction> }
impl ControlHandle {
    pub fn cancel(&self) { let _ = self.tx.send(crate::hotkey::HotkeyAction::CancelRecording); }
}

pub fn run(config: Config, observer: std::sync::Arc<dyn EngineObserver>) -> anyhow::Result<()>;
```

引擎在循环里的回调点:
- `StartRecording` 成功 → `on_state(Recording)`;`Recorder::start` 失败 → `on_state(Failed)`。
- `CancelRecording | DiscardRecording` → 置 `recorder = None` → `on_state(Cancelled)`。
- `Stop*` 且 `recorder` 存在 → 取出后立即 `on_state(Processing)`;识别失败 → `on_state(Failed)`;
  注入成功 → `on_state(Done)`;注入失败 → `on_state(Failed)`。命令模式同理。
- `Stop*` 但 `recorder` 为 `None`(已取消)→ 直接 `continue`,**不回调**(浮窗已隐藏)。
- 进入循环前:`observer.on_ready(ControlHandle { tx: tx.clone() })`。

> CLI(`src/main.rs`)传 `Arc::new(NoopObserver)`(`struct NoopObserver; impl EngineObserver for NoopObserver {}`,
> 全用默认空方法)→ **命令行行为完全不变**。

### Tauri 侧(`src-tauri/src/lib.rs`)

- **浮窗 window**(label `overlay`):`tauri.conf.json` 声明第二个窗口,
  `visible:false, decorations:false, transparent:true, alwaysOnTop:true, skipTaskbar:true,
  resizable:false, focus:false, shadow:false`,尺寸约 `width:240,height:72`,`url:"overlay.html"`。
- **不抢焦点(成败关键)**:`setup` 里取浮窗 HWND,`SetWindowLongPtrW(GWL_EXSTYLE, 原值 | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)`。
  这样即使点药丸取消也不会激活浮窗(但仍收得到点击),注入目标不变;并从 Alt-Tab 隐藏。
- **ControlSlot**:`#[derive(Default)] struct ControlSlot(Mutex<Option<ControlHandle>>)`,在 `setup` 里
  先 `app.manage(ControlSlot::default())`(空槽),保证命令随时能取到已注册的 state。
- **TauriObserver**:持 `AppHandle`,实现 `EngineObserver`:
  - `on_ready(control)`:`app.state::<ControlSlot>()` 取槽,填入 `control`。
  - `on_state(Recording)`:按主屏算坐标(见下)`set_position` 后 `show()`(不 `set_focus`);再 `emit_to("overlay","bt:state","recording")`。
  - 其他状态:仅 `emit_to("overlay","bt:state", tag)`(前端负责动画与最终 `hide()`)。
- **命令 `cancel_recording`**:`State<ControlSlot>` → 取 `ControlHandle` → `.cancel()`。注册进 `invoke_handler`。
- 在 `setup` 用 `TauriObserver` 替换现有 `engine::run(cfg)` 调用为 `engine::run(cfg, Arc::new(TauriObserver{app}))`。

### 底部居中坐标(纯函数,可 TDD)

```rust
// 放核心 crate(src/engine.rs)以便 `cargo test -p voice-input` 覆盖
pub fn bottom_center(area_w: i32, area_h: i32, win_w: i32, win_h: i32, bottom_margin: i32) -> (i32, i32) {
    ((area_w - win_w) / 2, area_h - win_h - bottom_margin)
}
```
调用方喂主屏尺寸(`monitor.size()`)与 `bottom_margin`(约 80,避开任务栏)。

### 前端(`src-ui/overlay.tsx` + 根 `overlay.html`)

- Vite 加第二入口:`vite.config.ts` 的 `build.rollupOptions.input = { main:"index.html", overlay:"overlay.html" }`。
- `overlay.tsx`:用 `@tauri-apps/api/event` 的 `listen("bt:state", …)` 驱动本地状态机渲染药丸:
  - `recording`:淡入,秒计时从 0 起跑(前端 `setInterval`,显示整数秒),波形 CSS 动画。
  - `processing`:秒数位换转圈,波形定格变暗。
  - `done`:绿 ✓ 闪约 600ms → 淡出 → `getCurrentWindow().hide()`。
  - `cancelled`:立即淡出 → `hide()`。
  - `failed`:红 ✕ 闪约 800ms → 淡出 → `hide()`。
  - 点药丸:立即淡出 + `hide()` + `invoke("cancel_recording")`(本地先反馈,引擎再确认)。
- 新增依赖 `@tauri-apps/api`;能力(capabilities)放开浮窗的 `event` 监听、`window:hide`,并允许 `cancel_recording` 命令。

## Esc 取消(键盘钩子那条路)

Esc 与鼠标点击同效,但 Esc 走键盘钩子。现有 `OtherDown → CancelRecording` 是 `suppress:false`
(键透传给程序);Esc 要**吞掉**(对前台程序无副作用)。**坑**:Esc 被吞后,Windows 在按住
Win 期间没看到任何中间键,松 Win 会弹开始菜单 —— 必须走 Phase 1 的"伪装释放"。

状态机改动(`src/hotkey/state.rs`,纯逻辑,TDD):
- 新增 `Event::EscDown`。
- 新增字段 `passthrough_seen: bool`;`OtherDown` 命中(透传)时置 `true`。
- `EscDown`:若 `pressed && !combo` → `combo = true`、返回 `{ CancelRecording, suppress:true }`;否则 `{ None, suppress:false }`(空闲时 Esc 原样透传)。
- `PrimaryUp` 命中 `was_combo` 时:`suppress = !passthrough_seen`
  —— 即**全程只被吞键(如 Esc)收尾时,对主键弹起走伪装释放**(防开始菜单);有透传键则照旧放行。
- `PrimaryDown` / `PrimaryUp` 重置 `passthrough_seen`。

钩子改动(`src/hotkey/mod.rs`):`vk == VK_ESCAPE && is_down` → `Event::EscDown`(置于 `OtherDown` 兜底之前)。
`HotkeyState` 的 `CancelRecording` 经现有 `dispatch` 映射到 `HotkeyAction::CancelRecording`(已存在)。

> 两条取消路径(Esc / 鼠标)最终都汇入引擎 `HotkeyAction::CancelRecording`:置 `recorder=None`
> + `on_state(Cancelled)`。鼠标取消后若仍按住 Win,松手的 `Stop*` 因 `recorder=None` 自然空转。

## 配置

`config.toml` 新增可选表(`src/config.rs`,缺省即默认):

```toml
[overlay]
enabled = true   # 关掉则完全不弹浮窗(G4 设置页会暴露此开关)
```

`enabled=false` 时:Tauri 的 `TauriObserver.on_state` 不显示/不 emit(引擎逻辑照跑)。

## 数据流

```
按住热键 → 引擎 StartRecording → on_state(Recording)
   → TauriObserver: 主屏底部居中定位 + show(不抢焦点) + emit recording
   → 前端: 淡入 + 秒计时 + 波形动
[取消] 点药丸/按 Esc → CancelRecording → recorder=None + on_state(Cancelled) → 前端淡出 hide;跳过 LLM
[正常] 松手 → Stop* → on_state(Processing)[转圈] → ASR →(LLM)→ 注入
       成功 → on_state(Done)[绿✓闪→淡出]    失败 → on_state(Failed)[红✕闪→淡出]
```

## 错误处理

| 情况 | 处理 |
|------|------|
| 麦克风启动失败 | `on_state(Failed)`(红 ✕ 闪一下);引擎照常 eprintln |
| ASR / LLM / 注入失败 | `on_state(Failed)`;不静默 |
| 浮窗 HWND 取不到 / 设样式失败 | 记录日志,降级为"可能抢焦点"但不崩;浮窗仍显示 |
| `[overlay] enabled=false` | 不显示浮窗,功能不受影响 |
| 多显示器 | v1 固定主屏;焦点不在主屏时药丸出现在主屏(可接受,后续增强) |

## 测试策略(沿用项目惯例:纯函数 TDD + OS 集成真机)

- **TDD 单测**:
  - 状态机:`EscDown` 录音中 → `{CancelRecording, suppress:true}`;空闲 → `{None, suppress:false}`;
    Esc 取消后 `PrimaryUp` → `{None, suppress:true}`(伪装释放);保持 `other_key_cancels_as_combo` 仍 `suppress:false`。
  - `bottom_center(...)` 坐标计算。
  - `[overlay] enabled` 配置解析(缺省为 `true`)。
- **真机端到端**(手动):
  - 按住 Win 说话 → 药丸在底部正中淡入、秒数走、波形动;**焦点不被抢、粘贴正常**。
  - 录制中**点药丸**取消 → 药丸消失、不出字(不调 LLM)。
  - 录制中**按 Esc**取消 → 同上,且**不弹开始菜单**、Esc 不打扰前台程序。
  - 正常松手 → 转圈 → 绿 ✓ → 淡出,文字注入。
  - 断网/坏 key 触发失败 → 红 ✕ → 淡出。
  - 三个热键(识别/翻译/命令)都弹同一药丸。
  - `[overlay] enabled=false` → 不弹浮窗,听写照常。
- 不为前端/Tauri 写自动化测试(以真机验证为主)。

## 非目标(后续子项目)

- 提示音(开始/结束、可开关)—— G3
- 设置界面(含浮窗开关、热键、词库、LLM 配置+测试按钮)—— G4
- 首启向导 + 模型下载管理 + 依赖检测 —— G5
- 安装包 + 关于/版权页 —— G6
- 波形随真实音量起伏、跟随焦点显示器 —— 后续增强
