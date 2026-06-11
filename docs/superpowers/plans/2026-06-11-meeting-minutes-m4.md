# 会议纪要 M4(LLM 纪要 + 会议页)Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development。步骤用 `- [ ]` 勾选。

**Goal:** 会议结束后把转写交 LLM 生成结构化纪要、写进 `<base>.md`(纪要在前 + 转写附录);主窗口新增「会议纪要」页:列历史会议、看纪要/转写、复制/导出/重新生成纪要/删除/打开文件夹。

**Architecture:** 复用 `corrector` 的 OpenAI 兼容调用新增 `generate_minutes`(长超时);`meeting::minutes` 负责把 `Transcript` 拼成 LLM 输入与最终 md;`MeetingSession::stop` 的后台步骤在转写后追加纪要生成;新增 Tauri 会议命令 + React 会议页。纯逻辑(提示词组装、md 拼装、列目录解析)TDD;LLM/命令/前端为集成,真机验证。

**Tech Stack:** reqwest blocking(已用)、serde、Tauri 2 命令、React/TS/Tailwind。**Branch:** `2.x`(v2.0.0)。**Spec:** `docs/superpowers/specs/2026-06-11-meeting-minutes-design.md`。

---

## 范围与不做(M4)

**做**:`minutes_prompt` 配置 + 内置默认;`generate_minutes` LLM 调用;`<base>.md` = 纪要 + 转写附录;`stop` 后台接纪要;会议命令(list/get/regenerate/delete/open-folder);会议页(列表 + 详情 + 操作);output_dir 解析统一(绝对化)。

**不做**:说话人聚类分人(M3,独立里程碑);会议页里的富 Markdown 渲染(M4 用滚动 `<pre>` 显示 md 原文,够用);进度条动画(用文字状态);音频内嵌播放器(给"打开文件夹"即可)。

---

## 文件结构

| 文件 | 改动 |
|---|---|
| `src/config.rs` | `MeetingConfig` 加 `minutes_prompt`;`PROMPT_MINUTES` 常量;`MeetingConfig::effective_minutes_prompt()`;`load_resolved` 解析 `output_dir` 为绝对 |
| `src/corrector.rs` | 加 `generate_minutes(cfg:&LlmConfig, prompt:&str, content:&str)->Result<String>`(长超时) |
| `src/meeting/minutes.rs` | `transcript_to_input(&Transcript)->String`、`assemble_md(base, minutes:Option<&str>, &Transcript)->String`(纯,TDD) |
| `src/meeting/transcript.rs` | 加 `lines_markdown()`(无标题的行 md),`to_markdown` 复用它 |
| `src/meeting/session.rs` | `stop` 后台:转写后调纪要,写 `<base>.md`=assemble_md |
| `src-tauri/src/meeting_cmd.rs` | 新:会议命令(list/get/regenerate/delete/open_folder) |
| `src-tauri/src/lib.rs` | 注册会议命令;`start_meeting`/`stop_meeting` 用解析后的 output_dir + 传 llm/纪要参数 |
| `src-ui/settings/MeetingPage.tsx` | 新:会议页 |
| `src-ui/settings/meetingApi.ts` | 新:命令 invoke 封装 + 类型 |
| `src-ui/App.tsx` | 导航加「会议纪要」页 |

---

## Task 1: minutes_prompt 配置 + output_dir 解析(TDD)

**Files:** Modify `src/config.rs`;Test 同文件。

- [ ] **Step 1: 失败测试**

`#[cfg(test)] mod tests` 内追加:
```rust
#[test]
fn meeting_minutes_prompt_defaults_to_builtin() {
    let m = MeetingConfig::default();
    assert_eq!(m.minutes_prompt, ""); // 空=用内置
    assert!(m.effective_minutes_prompt().contains("会议纪要"));
}

#[test]
fn meeting_effective_minutes_prompt_prefers_custom() {
    let mut m = MeetingConfig::default();
    m.minutes_prompt = "自定义纪要提示".into();
    assert_eq!(m.effective_minutes_prompt(), "自定义纪要提示");
}
```
Run: `cargo test -p voice-input meeting_minutes_prompt` → 编译失败。

- [ ] **Step 2: 实现**

`MeetingConfig` 加字段(在 `vad_model` 后):
```rust
    /// 会议纪要提示词;留空用内置默认。
    pub minutes_prompt: String,
```
`impl Default for MeetingConfig` 加:
```rust
            minutes_prompt: String::new(),
```
在 `MeetingConfig` 加 impl(放结构体/Default 之后):
```rust
impl MeetingConfig {
    /// 实际纪要提示词:自定义优先,否则内置默认。
    pub fn effective_minutes_prompt(&self) -> String {
        if self.minutes_prompt.trim().is_empty() {
            PROMPT_MINUTES.to_string()
        } else {
            self.minutes_prompt.clone()
        }
    }
}

/// 内置会议纪要提示词。
const PROMPT_MINUTES: &str = "你是会议纪要助理。下面是一段带时间戳与说话人(我/对方)的会议转写。\
请整理成结构化的中文会议纪要,包含:1) 会议主题(若能判断);2) 关键讨论点;3) 决议/结论;\
4) 待办事项(含负责人与时限,若提及)。忠实原意,不要编造未提及的内容;条理清晰,用 Markdown\
(二级标题与列表)。只输出纪要正文,不要复述原始转写。";
```

`load_resolved` 内,在 `vad_model` 解析行后加 output_dir 解析:
```rust
        cfg.meeting.output_dir = resolve_model_dir(&base, &cfg.meeting.output_dir);
```

- [ ] **Step 3: 通过 + 提交**

Run: `cargo test -p voice-input meeting` → 全 PASS。
```powershell
git add src/config.rs
git commit -m "feat(meeting): minutes_prompt 配置 + 内置默认 + output_dir 解析绝对化"
```

---

## Task 2: corrector::generate_minutes(集成)

**Files:** Modify `src/corrector.rs`。

- [ ] **Step 1: 加函数**(放 `test_connection` 之后,复用 `build_request_body`/`parse_response`):
```rust
/// 生成会议纪要:用 [llm] 配置 + 给定纪要提示词,把整段转写作为用户消息发给 LLM。
/// 超时取 max(120, timeout_secs)(转写较长,LLM 用时更久);失败返回 Err 由调用方处理。
/// **不受** enabled/skip_if_shorter_than 影响(是否调用由调用方决定)。
pub fn generate_minutes(cfg: &LlmConfig, prompt: &str, content: &str) -> anyhow::Result<String> {
    if cfg.base_url.trim().is_empty() {
        anyhow::bail!("未配置 LLM 接口地址");
    }
    let secs = cfg.timeout_secs.max(120);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(secs))
        .build()?;
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = build_request_body(cfg, prompt, content);
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()?
        .error_for_status()?;
    let value: Value = resp.json()?;
    parse_response(&value).ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))
}
```

- [ ] **Step 2: 编译 + 提交**

Run: `cargo build -p voice-input` → 通过;`cargo test -p voice-input` 无回归。
```powershell
git add src/corrector.rs
git commit -m "feat(meeting): corrector::generate_minutes(长超时纪要生成)"
```

---

## Task 3: 纪要输入 + md 拼装(TDD)

**Files:** Create `src/meeting/minutes.rs`;Modify `src/meeting/mod.rs`、`src/meeting/transcript.rs`。

- [ ] **Step 1: transcript.rs 抽出 lines_markdown**

把 `to_markdown` 改为复用 `lines_markdown`。在 `impl Transcript` 内:把现有 `to_markdown` 替换为:
```rust
    /// 仅转写正文(无标题),每行 `[mm:ss] **说话人**:文本`。
    pub fn lines_markdown(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            out.push_str(&format!(
                "`[{}]` **{}**:{}\n\n",
                ms_to_clock(l.start_ms),
                l.speaker.label(),
                l.text.trim()
            ));
        }
        out
    }

    /// 渲染为 Markdown(标题 + 转写正文)。
    pub fn to_markdown(&self) -> String {
        format!("# 会议转写 {}\n\n{}", self.base, self.lines_markdown())
    }
```
(原 `to_markdown` 的两个测试仍应通过——header 与行格式不变。)

- [ ] **Step 2: 挂模块**

`src/meeting/mod.rs` 追加:
```rust
pub mod minutes;
pub use minutes::{assemble_md, transcript_to_input};
```

- [ ] **Step 3: 失败测试**

创建 `src/meeting/minutes.rs`:
```rust
use super::transcript::{ms_to_clock, Transcript};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::transcript::{Line, Speaker};

    fn t() -> Transcript {
        Transcript {
            base: "2026-06-11_120000".into(),
            lines: vec![
                Line { start_ms: 0, end_ms: 1000, speaker: Speaker::Me, text: "下周交付".into() },
                Line { start_ms: 2000, end_ms: 3000, speaker: Speaker::Other, text: "好的".into() },
            ],
        }
    }

    #[test]
    fn input_has_speaker_and_time_lines() {
        let s = transcript_to_input(&t());
        assert!(s.contains("[00:00] 我:下周交付"));
        assert!(s.contains("[00:02] 对方:好的"));
    }

    #[test]
    fn assemble_with_minutes_puts_minutes_first_then_transcript() {
        let md = assemble_md("2026-06-11_120000", Some("## 决议\n- 下周交付"), &t());
        let i_min = md.find("决议").unwrap();
        let i_tr = md.find("完整转写").unwrap();
        assert!(i_min < i_tr, "纪要应在转写之前");
        assert!(md.contains("# 会议纪要 2026-06-11_120000"));
        assert!(md.contains("`[00:00]` **我**:下周交付"));
    }

    #[test]
    fn assemble_without_minutes_notes_absence() {
        let md = assemble_md("b", None, &t());
        assert!(md.contains("未生成纪要"));
        assert!(md.contains("完整转写"));
    }
}
```
Run: `cargo test -p voice-input meeting::minutes` → 编译失败。

- [ ] **Step 4: 实现**(加在 `use` 之后、tests 之前):
```rust
/// 把转写拼成 LLM 的用户输入(每行「[mm:ss] 说话人:文本」)。
pub fn transcript_to_input(t: &Transcript) -> String {
    let mut s = String::new();
    for l in &t.lines {
        s.push_str(&format!(
            "[{}] {}:{}\n",
            ms_to_clock(l.start_ms),
            l.speaker.label(),
            l.text.trim()
        ));
    }
    s
}

/// 拼最终 `<base>.md`:纪要(有则)在前 + 「完整转写」附录。
pub fn assemble_md(base: &str, minutes: Option<&str>, t: &Transcript) -> String {
    let mut out = format!("# 会议纪要 {base}\n\n");
    match minutes {
        Some(m) if !m.trim().is_empty() => {
            out.push_str(m.trim());
            out.push_str("\n\n");
        }
        _ => out.push_str("> 未生成纪要(LLM 未配置或调用失败)。\n\n"),
    }
    out.push_str("---\n\n## 完整转写\n\n");
    out.push_str(&t.lines_markdown());
    out
}
```

- [ ] **Step 5: 通过 + 提交**

Run: `cargo test -p voice-input meeting::transcript meeting::minutes` → 全 PASS。
```powershell
git add src/meeting/mod.rs src/meeting/minutes.rs src/meeting/transcript.rs
git commit -m "feat(meeting): 纪要 LLM 输入 + <base>.md 拼装(纪要+转写附录)(TDD)"
```

---

## Task 4: stop 后台接纪要(集成)

**Files:** Modify `src/meeting/session.rs`、`src-tauri/src/lib.rs`。

- [ ] **Step 1: 扩 stop 签名,后台转写后生成纪要**

`MeetingSession::stop` 再加两个参数:`llm: crate::config::LlmConfig`、`minutes_prompt: String`。后台线程在写 md 处改为:转写→(若 llm.enabled 且有 base_url)生成纪要→`assemble_md` 写 `<base>.md`;json 照写。把原来直接 `t.to_markdown()` 写 md 的那段替换:
```rust
                Ok(t) => {
                    let json = paths.dir.join(format!("{base}.json"));
                    let _ = std::fs::write(&json, t.to_json());
                    // 生成纪要(LLM 启用且配了地址时);失败则 md 仅含转写并标注。
                    let minutes = if llm.enabled && !llm.base_url.trim().is_empty() {
                        let input = super::minutes::transcript_to_input(&t);
                        match crate::corrector::generate_minutes(&llm, &minutes_prompt, &input) {
                            Ok(m) => Some(m),
                            Err(e) => {
                                eprintln!("会议纪要生成失败(转写已保存):{e}");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let md = paths.dir.join(format!("{base}.md"));
                    let _ = std::fs::write(&md, super::minutes::assemble_md(&base, minutes.as_deref(), &t));
                    eprintln!("会议成稿:{}({} 行转写{})", md.display(), t.lines.len(),
                        if minutes.is_some() { " + 纪要" } else { "" });
                }
```
`llm`、`minutes_prompt` 要 `move` 进线程闭包(已是 owned)。

- [ ] **Step 2: 更新 lib.rs 调用 + output_dir 统一**

`start_meeting` 里把 root 改为用解析后的 output_dir(load_resolved 已绝对化):
```rust
    let root = std::path::PathBuf::from(&cfg.meeting.output_dir);
```
(删掉原来的 `current_exe()...join(...)` 那段。)

`stop_meeting` 里 `sess.stop(...)` 增传 llm 与纪要提示:
```rust
        match sess.stop(
            cfg.meeting.audio_retention,
            cfg.meeting.archive_bitrate,
            cfg.asr.model_dir.clone(),
            cfg.asr.language.clone(),
            cfg.meeting.vad_model.clone(),
            cfg.llm.clone(),
            cfg.meeting.effective_minutes_prompt(),
        ) {
```

- [ ] **Step 3: 编译 + 测试 + 提交**

Run(仓库根):`cargo build` 通过;`cargo test -p voice-input` 无回归。
```powershell
git add src/meeting/session.rs src-tauri/src/lib.rs
git commit -m "feat(meeting): stop 后台生成纪要,写 <base>.md(纪要+转写)"
```

---

## Task 5: Tauri 会议命令(集成)

**Files:** Create `src-tauri/src/meeting_cmd.rs`;Modify `src-tauri/src/lib.rs`(`mod` + 注册)。

- [ ] **Step 1: 创建命令模块**

创建 `src-tauri/src/meeting_cmd.rs`:
```rust
//! 会议页后端:列历史会议、读单场、重新生成纪要、删除、打开文件夹。
use std::path::PathBuf;
use serde::Serialize;
use voice_input::config::Config;

/// 会议根目录(load_resolved 已把 output_dir 绝对化)。
fn meetings_root() -> PathBuf {
    match Config::load_resolved() {
        Ok(c) => PathBuf::from(c.meeting.output_dir),
        Err(_) => PathBuf::from("./meetings"),
    }
}

#[derive(Serialize)]
pub struct MeetingSummary {
    pub base: String,
    pub has_md: bool,
    pub has_mp3: bool,
}

/// 列历史会议(按文件夹名倒序=最新在前)。
#[tauri::command]
pub fn list_meetings() -> Vec<MeetingSummary> {
    let root = meetings_root();
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                let base = e.file_name().to_string_lossy().to_string();
                let dir = e.path();
                out.push(MeetingSummary {
                    has_md: dir.join(format!("{base}.md")).exists(),
                    has_mp3: dir.join(format!("{base}.mp3")).exists(),
                    base,
                });
            }
        }
    }
    out.sort_by(|a, b| b.base.cmp(&a.base));
    out
}

#[derive(Serialize)]
pub struct MeetingDetail {
    pub base: String,
    pub md: String,       // <base>.md 内容(没有则空串)
    pub has_json: bool,
    pub has_mp3: bool,
}

/// 读单场会议的 md 内容与产物存在性。
#[tauri::command]
pub fn get_meeting(base: String) -> MeetingDetail {
    let dir = meetings_root().join(&base);
    let md = std::fs::read_to_string(dir.join(format!("{base}.md"))).unwrap_or_default();
    MeetingDetail {
        has_json: dir.join(format!("{base}.json")).exists(),
        has_mp3: dir.join(format!("{base}.mp3")).exists(),
        base,
        md,
    }
}

/// 用 <base>.json 重新生成纪要,重写 <base>.md。返回新的 md 内容。
#[tauri::command]
pub fn regenerate_minutes(base: String) -> Result<String, String> {
    let dir = meetings_root().join(&base);
    let json = std::fs::read_to_string(dir.join(format!("{base}.json")))
        .map_err(|_| "找不到转写数据(.json),无法重新生成".to_string())?;
    let t: voice_input::meeting::Transcript =
        serde_json::from_str(&json).map_err(|e| format!("解析转写失败:{e}"))?;
    let cfg = Config::load_resolved().map_err(|e| format!("加载配置失败:{e}"))?;
    let input = voice_input::meeting::transcript_to_input(&t);
    let minutes = if cfg.llm.enabled && !cfg.llm.base_url.trim().is_empty() {
        match voice_input::corrector::generate_minutes(
            &cfg.llm,
            &cfg.meeting.effective_minutes_prompt(),
            &input,
        ) {
            Ok(m) => Some(m),
            Err(e) => return Err(format!("纪要生成失败:{e}")),
        }
    } else {
        return Err("未启用/配置 LLM,无法生成纪要".into());
    };
    let md = voice_input::meeting::assemble_md(&base, minutes.as_deref(), &t);
    std::fs::write(dir.join(format!("{base}.md")), &md).map_err(|e| format!("写 md 失败:{e}"))?;
    Ok(md)
}

/// 删除整场会议文件夹。
#[tauri::command]
pub fn delete_meeting(base: String) -> Result<(), String> {
    let dir = meetings_root().join(&base);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("删除失败:{e}"))
}

/// 在资源管理器打开会议文件夹。
#[tauri::command]
pub fn open_meeting_folder(base: String) -> Result<(), String> {
    let dir = meetings_root().join(&base);
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map_err(|e| format!("打开失败:{e}"))?;
    Ok(())
}
```
> `Transcript`/`transcript_to_input`/`assemble_md`/`generate_minutes` 需经 `voice_input` 暴露:`Transcript` 与两个函数已在 `meeting::mod` re-export;`generate_minutes` 是 `corrector` 的 pub 函数(`voice_input::corrector::generate_minutes`)。

- [ ] **Step 2: 注册到 lib.rs**

`src-tauri/src/lib.rs` 顶部加 `mod meeting_cmd;`;`generate_handler!` 列表里加:
```rust
            meeting_cmd::list_meetings,
            meeting_cmd::get_meeting,
            meeting_cmd::regenerate_minutes,
            meeting_cmd::delete_meeting,
            meeting_cmd::open_meeting_folder,
```

- [ ] **Step 3: 编译 + 提交**

Run(仓库根):`cargo build` 通过。
```powershell
git add src-tauri/src/meeting_cmd.rs src-tauri/src/lib.rs
git commit -m "feat(meeting): Tauri 会议命令(列表/读取/重生成/删除/打开文件夹)"
```

---

## Task 6: 会议页前端(集成)

**Files:** Create `src-ui/settings/meetingApi.ts`、`src-ui/settings/MeetingPage.tsx`;Modify `src-ui/App.tsx`。

- [ ] **Step 1: API 封装**

创建 `src-ui/settings/meetingApi.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";

export interface MeetingSummary { base: string; has_md: boolean; has_mp3: boolean; }
export interface MeetingDetail { base: string; md: string; has_json: boolean; has_mp3: boolean; }

export const listMeetings = () => invoke<MeetingSummary[]>("list_meetings");
export const getMeeting = (base: string) => invoke<MeetingDetail>("get_meeting", { base });
export const regenerateMinutes = (base: string) => invoke<string>("regenerate_minutes", { base });
export const deleteMeeting = (base: string) => invoke<void>("delete_meeting", { base });
export const openMeetingFolder = (base: string) => invoke<void>("open_meeting_folder", { base });
```

- [ ] **Step 2: 会议页组件**

创建 `src-ui/settings/MeetingPage.tsx`:
```tsx
import { useEffect, useState } from "react";
import {
  deleteMeeting, getMeeting, listMeetings, openMeetingFolder, regenerateMinutes,
  type MeetingDetail, type MeetingSummary,
} from "./meetingApi";

export default function MeetingPage() {
  const [list, setList] = useState<MeetingSummary[]>([]);
  const [sel, setSel] = useState<MeetingDetail | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = () => listMeetings().then(setList).catch((e) => setErr(String(e)));
  useEffect(() => { refresh(); }, []);

  const open = async (base: string) => {
    setErr(null);
    setSel(await getMeeting(base));
  };
  const onRegen = async () => {
    if (!sel) return;
    setBusy("正在重新生成纪要…"); setErr(null);
    try {
      const md = await regenerateMinutes(sel.base);
      setSel({ ...sel, md });
    } catch (e) { setErr(String(e)); } finally { setBusy(null); }
  };
  const onCopy = () => { if (sel) navigator.clipboard.writeText(sel.md).catch(() => {}); };
  const onDelete = async () => {
    if (!sel) return;
    setBusy("删除中…");
    try { await deleteMeeting(sel.base); setSel(null); await refresh(); }
    catch (e) { setErr(String(e)); } finally { setBusy(null); }
  };

  return (
    <div className="flex gap-4 h-full">
      <aside className="w-52 flex-none overflow-y-auto border-r border-neutral-200 dark:border-neutral-700 pr-2">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-sm font-medium">历史会议</h2>
          <button onClick={refresh} className="text-xs text-blue-500 hover:underline">刷新</button>
        </div>
        {list.length === 0 && <p className="text-xs text-neutral-400">还没有会议记录</p>}
        {list.map((m) => (
          <button
            key={m.base}
            onClick={() => open(m.base)}
            className={`block w-full text-left px-2 py-1.5 rounded text-xs mb-1 ${
              sel?.base === m.base ? "bg-blue-500 text-white" : "hover:bg-neutral-100 dark:hover:bg-neutral-800"
            }`}
          >
            {m.base}
          </button>
        ))}
      </aside>
      <section className="flex-1 min-w-0 flex flex-col">
        {!sel && <p className="text-sm text-neutral-400">选择左侧的会议查看纪要与转写。</p>}
        {sel && (
          <>
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <span className="text-sm font-medium">{sel.base}</span>
              <span className="flex-1" />
              <button onClick={onCopy} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800">复制</button>
              <button onClick={onRegen} disabled={!sel.has_json || !!busy} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800 disabled:opacity-40">重新生成纪要</button>
              <button onClick={() => openMeetingFolder(sel.base)} className="text-xs px-2 py-1 rounded border border-neutral-300 dark:border-neutral-700 hover:bg-neutral-50 dark:hover:bg-neutral-800">打开文件夹</button>
              <button onClick={onDelete} disabled={!!busy} className="text-xs px-2 py-1 rounded border border-red-300 text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 disabled:opacity-40">删除</button>
            </div>
            {busy && <p className="text-xs text-blue-500 mb-1">{busy}</p>}
            {err && <p className="text-xs text-red-600 mb-1">{err}</p>}
            <pre className="flex-1 overflow-auto text-xs whitespace-pre-wrap bg-neutral-50 dark:bg-neutral-950 rounded p-3 border border-neutral-200 dark:border-neutral-800">
              {sel.md || "(此会议还没有 .md;可能正在后台转写,稍后刷新)"}
            </pre>
          </>
        )}
        {err && !sel && <p className="text-xs text-red-600">{err}</p>}
      </section>
    </div>
  );
}
```

- [ ] **Step 3: 接进导航**

`src-ui/App.tsx`:import `MeetingPage`;`PAGES` 加一项(放 help 前):
```tsx
  { id: "meeting", icon: "🎙", label: "会议纪要" },
```
内容区加渲染(在 `style` 与 `help` 之间):
```tsx
          {page === "meeting" && <MeetingPage />}
```

- [ ] **Step 4: 前端构建检查**

Run(仓库根):`npm run build` → 通过(两入口都打包,无 TS 报错)。

- [ ] **Step 5: 提交**

```powershell
git add src-ui/settings/meetingApi.ts src-ui/settings/MeetingPage.tsx src-ui/App.tsx
git commit -m "feat(meeting): 会议纪要页(历史列表 + 纪要/转写查看 + 复制/导出/重生成/删除)"
```

---

## Task 7: 真机端到端验收(M4)

**Files:** 无。

- [ ] **Step 1**: 仓库根 `npm run tauri dev`(先退别的实例)。开一场中文会议(放中文音频 + 自己说几句)→ 结束。
- [ ] **Step 2**: 等日志 `会议成稿:...md(N 行转写 + 纪要)`。
- [ ] **Step 3**: 主窗口(托盘→设置)→「会议纪要」页:左侧应出现刚才的会议;点开,右侧显示**纪要(在前)+ 完整转写**;试「复制」「打开文件夹」。
- [ ] **Step 4**: 点「重新生成纪要」→ 纪要刷新。
- [ ] **Step 5**: 「删除」一场 → 从列表消失、文件夹被删。
- [ ] **Step 6**: 边界:LLM 未配置/失败时,`.md` 应仍有转写 + "未生成纪要"提示(可临时把 [llm] enabled=false 验)。

---

## 自检(对照 spec)

**1. 覆盖**:LLM 纪要(T2+T4)、`<base>.md`=纪要+转写(T3+T4)、minutes_prompt 配置(T1)、会议页列表/查看/复制/导出(打开文件夹)/重生成/删除(T5+T6)、output_dir 统一(T1+T4)。**不含**分人(M3)、富 md 渲染/播放器(范围声明)。
**2. 占位符**:无 TBD;集成任务给完整代码 + 真机验收。
**3. 类型一致**:`effective_minutes_prompt`、`generate_minutes(&LlmConfig,&str,&str)`、`transcript_to_input(&Transcript)`、`assemble_md(&str,Option<&str>,&Transcript)`、`Transcript::lines_markdown`、`stop(retention,bitrate,asr_dir,lang,vad,llm,minutes_prompt)`、命令名(list_meetings/get_meeting/regenerate_minutes/delete_meeting/open_meeting_folder)前后端一致。
**4. 风险**:① 长转写 LLM 超时——已用 max(120s);② 后台线程顺序:json 先写(重生成依赖它)再纪要再 md;③ output_dir 改为 load_resolved 解析后,start/list 用同一根,dev 落 repo 根 `meetings/`(与旧 `target/debug/meetings` 不同,属预期)。
