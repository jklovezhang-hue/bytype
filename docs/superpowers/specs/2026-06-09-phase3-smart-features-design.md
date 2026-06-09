# Phase 3 — 运行时智能功能 设计文档

日期:2026-06-09
状态:已批准设计,待写实现计划

## 目标

在已完成的语音输入工具(左 Win 识别整理、左 Win+左 Alt 翻译)基础上,增加四项
"智能化"运行时功能,全部由 `config.toml` 驱动:

1. **自定义词库** — 专有名词/术语注入提示词,优先按指定拼写纠正
2. **选中文字 + 语音命令编辑** — 新热键(左 Win+左 Ctrl):选中→说指令→替换
3. **随应用切换风格** — 按前台进程名匹配,追加风格指令到提示词
4. **热键可配置** — 三个热键的按键在配置里可改

## 已确定的决策

| 决策点 | 选择 |
|--------|------|
| 命令热键 | 左 Win + 左 Ctrl |
| 应用风格映射 | 应用 → 自定义风格指令(每应用一段提示词追加) |
| 热键可配置 | 是(配置里改,需键名→VK 解析) |
| 词库作用范围 | 普通 / 翻译 / 命令 三种都生效 |
| 应用风格作用范围 | 普通 / 翻译(命令模式以指令为准,不加风格) |

## 配置(config.toml 新增)

```toml
[hotkey]
primary            = "LWin"   # 按住说话(普通识别整理)
translate_modifier = "LAlt"   # 配合 primary = 翻译成英文
command_modifier   = "LCtrl"  # 配合 primary = 对选中文字执行语音命令

[llm]
# ……(已有字段)……
vocabulary     = ["Kubernetes", "OneDrive", "Claude"]  # 专有名词;留空则不注入
command_prompt = ""           # 命令模式提示词;留空用内置默认

# 随应用风格;可写多条 [[app_style]],按前台进程名(不区分大小写)包含匹配,取第一条命中
[[app_style]]
match = "outlook"
style = "用正式、专业的书面语。"
[[app_style]]
match = "code"
style = "保留代码与技术术语原文。"
```

支持的键名(键名→VK 解析):`LWin RWin LAlt RAlt LCtrl RCtrl LShift RShift`。
未知键名 → 配置加载报错并提示合法取值。

## 架构与模块

### 键名解析 `src/keys.rs`(新增)
- `pub fn vk_from_name(name: &str) -> anyhow::Result<u16>`:把 `"LWin"` 等映射到 Win32 VK。
- 纯函数,单测覆盖(合法名、大小写、未知名报错)。

### 状态机 `src/hotkey/state.rs`(泛化)
- 从"主键 + 1 个修饰键(Alt)"泛化为"主键 + 2 个修饰键(A=translate,B=command)"。
- 事件:`PrimaryDown` / `PrimaryUp { held_ms }` / `ModADown` / `ModAUp` / `ModBDown` /
  `ModBUp` / `OtherDown`。
- 动作:`None / StartRecording / CancelRecording / StopAndTranscribe / StopAndTranslate /
  StopAndCommand / DiscardRecording`。
- 状态:`pressed`、`combo`、`mod_a_seen`、`mod_b_seen`、各修饰键的 `down` 与 `swallowing`
  (跟踪物理按下、以及"吞掉的按下需把配对弹起也吞掉")。
- 松开判定:`combo` → None(放行);否则按住时长达标时优先级 **command > translate >
  transcribe**;时长不足 → Discard。命令/翻译/普通的 `PrimaryUp` 都 `suppress=true`
  (沿用伪装释放)。修饰键在主键按住期间(或先于主键按住)被吞掉。
- 纯逻辑,TDD 全覆盖。

### 钩子 `src/hotkey/mod.rs`(配置化 VK)
- `run` 接收三个 VK(primary / mod_a / mod_b),按 VK 把原始按键归类成上面的事件。
- `HotkeyAction` 增加 `StopAndCommand`。
- 只有 `PrimaryUp` 的 suppress 触发伪装释放;修饰键 suppress 只 `return 1`。
- 维持自注入事件 `INJECTED_TAG` 忽略机制。

### 前台进程名 `src/foreground.rs`(新增)
- `pub fn foreground_process_name() -> Option<String>`:`GetForegroundWindow` →
  `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`,取文件名(小写)。
- 失败返回 `None`(降级:不加风格)。

### 纠错/命令 `src/corrector.rs`(扩展)
- 现有 `process(raw, system_prompt)` 增加可选"上下文后缀"参数:把【词库行】+【应用风格行】
  拼到 system prompt 末尾。
- `correct(raw, ctx)` / `translate(raw, ctx)`:ctx 含词库+风格。
- 新增 `command(instruction, selected_text, ctx)`:system=command_prompt(+词库),
  user=结构化的"指令 + 文本";应用风格不参与命令模式。
- 词库行:`"以下专有名词若出现请按此拼写:A、B、C。"`(词库非空时)。
- 内置 `command_prompt` 默认:"你是文本编辑器。用户选中了一段文本并口述了一条指令。
  把指令应用到文本,只输出处理后的文本,不要解释、不要引号或代码块。"

### 选中命令流程(main.rs)
`StopAndCommand`:
1. 录音 → ASR → 得到指令文本 `instruction`。
2. 备份剪贴板 → 发 Ctrl+C(`INJECTED_TAG` 标记)→ 读剪贴板 = `selected`。
3. 若 `selected` 为空(没选中)→ 退化为普通插入:`inject_text(corrector.correct(instruction, ctx))`,结束。
4. 否则 `result = corrector.command(instruction, selected, ctx)` → 写剪贴板 → Ctrl+V 替换
   → 还原剪贴板。
- 剪贴板读取需要一点延迟等待 Ctrl+C 生效(~50ms)。

### 配置串联(main.rs)
- 启动读 `[hotkey]` 三键 → 解析 VK → 传入 `hotkey::run`。
- 每次处理时:取前台进程名 → 匹配 `app_style` → 与词库一起组成 ctx,传给 corrector。

## 数据流(命令模式)

```
选中文本 → 按住 Win+Ctrl 说"改成正式语气" → 松开
  → ASR=指令"改成正式语气"
  → 备份剪贴板 → Ctrl+C → 读到选中文本
  → LLM(command_prompt + 词库 ; 指令 + 选中文本) → 结果
  → 写剪贴板 → Ctrl+V 替换 → 还原剪贴板
```

## 错误处理

| 情况 | 处理 |
|------|------|
| 键名非法 | 配置加载报错,提示合法键名,程序退出 |
| 取前台进程名失败 | 降级:不加应用风格 |
| 命令模式未选中文本 | 退化为普通插入识别文本 |
| LLM 失败/超时 | 沿用回退:普通/翻译回退原文;命令回退为"原选中文本不变" |
| 剪贴板读写失败 | 报错日志,不崩溃 |

## 测试策略

- 单测:状态机(主键单独 / +translate / +command / 组合键 / 两修饰键优先级 / 误触 /
  修饰键吞掉与配对弹起);键名解析;corrector 命令请求体与词库/风格后缀拼接;
  app_style 匹配(进程名包含、大小写、取首条)。
- 手动:前台进程名、真实 LLM 命令编辑、各热键真机端到端。

## 非目标 / 后续

- 本阶段不做流式识别、不做 GUI。
- **Phase 4(打包与开箱,后续单独 spec)**:免工作目录(exe 旁定位 config/模型)、
  无窗口后台 + 日志写文件、开机自启、**首次运行引导安装**(检测缺模型→带进度下载安置;
  检测运行依赖 onnxruntime 原生库 / VC++ 运行库 → 引导安装或随包附带)。Phase 3 的配置
  与路径设计保持向前兼容,供 Phase 4 首启向导接入。
```
