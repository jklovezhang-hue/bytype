# ByType G3 — 录音提示音 设计文档

日期:2026-06-10
状态:已批准设计,待写实现计划

## 背景与目标

给录音加**开始 / 结束提示音**(可开关)。按住热键开始录音时播一个"开始音",松手结束录音时播
一个"结束音"。默认开启,可在配置里关闭或换成自定义音效。

**本子项目只做提示音。** 与 G2 浮窗相互独立(两者同时发生即可)。

## 已定决策(经 brainstorming 确认)

| 决策 | 选择 |
|------|------|
| 触发时机 | 仅**录音开始** + **录音结束(松手 Stop)**;取消 / 丢弃(短按)不响 |
| 音源 | 内置 .wav 音效(合成的柔和「叮 / 咚」);可用配置路径覆盖 |
| 默认音来源 | 用 `examples/gen_sounds.rs` 合成并提交;`include_bytes!` **嵌入二进制** |
| 播放方式 | Windows 原生 **`PlaySoundW`**(零新依赖;异步 `SND_ASYNC`) |
| 默认开关 | **默认开**(`[sound] enabled = true`) |
| 音量 | 不提供软件内音量(PlaySound 无此能力);音量由 wav 电平 + Windows 音量混合器决定 |
| 归属 | 放**核心 crate**(非 GUI 专属;windows 已是核心依赖)→ CLI 与 GUI 都有提示音 |

## 架构

新增核心模块 **`src/sound.rs`**。提示音由引擎在已有状态切换点直接触发(不经 G2 的
`EngineObserver`——提示音不是 GUI 专属,且 `PlaySoundW` 用的就是核心已依赖的 `windows` crate)。

```rust
// src/sound.rs
enum SoundSource {
    Embedded(&'static [u8]),  // 内置默认 wav
    File(std::path::PathBuf), // 自定义路径
}

pub struct SoundPlayer {
    start: SoundSource,
    end: SoundSource,
}

impl SoundPlayer {
    /// 由配置构建。路径已由 Config::load_resolved 解析为绝对(空 = 用内置默认),此处无需再解析。
    pub fn from_config(cfg: &crate::config::SoundConfig) -> SoundPlayer;
    pub fn play_start(&self);  // best-effort,失败仅记日志
    pub fn play_end(&self);
}
```

`play_*` 调用 `PlaySoundW`:
- `Embedded(bytes)` → `PlaySoundW(PCWSTR(bytes.as_ptr() as *const u16), None/HMODULE默认, SND_MEMORY | SND_ASYNC | SND_NODEFAULT)`。
  内嵌字节为 `'static`,异步播放期间始终有效。
- `File(path)` → 把路径转 UTF-16 + NUL,`PlaySoundW(PCWSTR(wide.as_ptr()), …, SND_FILENAME | SND_ASYNC | SND_NODEFAULT)`。
- 开始 / 结束不重叠,"同时只播一个"对本场景无影响。

引擎(`src/engine.rs::run_with`)触发点:
- `StartRecording` 成功(`recorder = Some`)后 → `player.play_start()`。
- `Stop*` 且 `recorder.take()` 拿到录音 → `player.play_end()`(在 `on_state(Processing)` 附近)。
- `CancelRecording | DiscardRecording` → **不播**。
- 仅当 `config.sound.enabled` 时构建并调用 player(关则完全不发声)。

> CLI(`engine::run` → `run_with(NoopObserver)`)同样会响——一致、无需额外接线。

## 配置

`config.toml` 新增可选表(`src/config.rs`,缺省即默认):

```toml
[sound]
enabled = true
start_sound = ""   # 留空用内置默认;填 wav 路径则覆盖
end_sound = ""
```

`SoundConfig { enabled: bool=true, start_sound: String="", end_sound: String="" }`,`#[serde(default)]`。
`Config::load_resolved()` 把**非空**的 `start_sound`/`end_sound` 相对 config 目录解析为绝对路径
(复用/推广现有 `resolve_model_dir` 的相对→绝对逻辑;空字符串保持空 = 用内置默认)。

## 组件 / 文件

- `src/sound.rs`(新):`SoundSource`、`SoundPlayer`、`from_config`、`play_start`/`play_end`、内嵌默认音
  `include_bytes!("../assets/sounds/start.wav" / "end.wav")`。
- `src/config.rs`:加 `SoundConfig` + `Config.sound`;`load_resolved` 解析两个路径;`#[cfg(test)]` 加解析测试。
- `src/engine.rs`:`run_with` 里按 `sound.enabled` 构建 `SoundPlayer`,在两个触发点调用。
- `src/lib.rs`:`pub mod sound;`。
- `assets/sounds/{start.wav, end.wav}`(提交):合成的柔和提示音。
- `examples/gen_sounds.rs`(新):用 `hound`(已是 dev-dependency)合成两个 wav 并写入 `assets/sounds/`。
  16-bit 单声道 44.1kHz;正弦 + 快速起音 + 指数衰减包络(避免爆音);**start 偏高**(如 ~880Hz,
  可两音上行)、**end 偏低**(如 ~587Hz,可下行);时长 ~150–200ms;峰值电平柔和(约 0.3)。
- 根 `Cargo.toml`:`windows` features 追加 `Win32_Media_Audio`(`PlaySoundW` 与 `SND_*` 常量)。
- `config.example.toml`:追加 `[sound]` 模板。

## 数据流

```
按住热键 → StartRecording(录音开始)→ play_start "叮"
松手 → Stop*(取到录音)→ play_end "咚" → 进入处理(识别/LLM/注入)
取消(Esc/点药丸)/ 短按丢弃 → 静默
[sound] enabled = false → 全程不发声
```

## 错误处理

| 情况 | 处理 |
|------|------|
| 播放失败(无输出设备 / PlaySound 返回 false) | 忽略,best-effort,不影响听写 |
| 自定义路径不存在 | 一次 `eprintln!` 警告;不发声;听写照常 |
| `[sound] enabled = false` | 不构建 player、不发声 |
| 开始音被麦克风轻微录入 | 已知小风险(短促、在说话前);真机若有碍再调小/缩短开始音 |

## 测试

沿用项目惯例(纯逻辑 TDD + OS 集成真机):
- **TDD**:`[sound]` 配置解析(默认 `enabled=true`、两路径默认空);自定义路径相对→绝对解析
  (空字符串保持空)。
- **真机端到端**(手动):按住热键听到开始音、松手听到结束音;取消 / 短按不响;
  `enabled=false` 全程静默;填自定义 wav 生效;识别内容不被开始音污染(识别仍正确)。
- 不为 `PlaySoundW`/音频输出写自动化测试(OS 集成,真机验证)。

## 非目标(后续或不做)

- 软件内音量滑块(选了 PlaySound,无此能力)—— 如将来需要,改用 `rodio`。
- 成功 / 失败 / 取消的独立提示音(本期只做开始 + 结束)。
- 设置界面里的提示音开关 UI —— 属 G4。
