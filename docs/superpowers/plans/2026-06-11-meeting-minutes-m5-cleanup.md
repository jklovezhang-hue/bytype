# 会议纪要 M5(转写 LLM 纠错)Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development。步骤用 `- [ ]`。

**Goal:** 给会议转写每段过一遍 LLM「忠实清理」(去语气词/纠错别字/补标点,不改写)+ 词库,让 `<base>.md` 的转写正文与据其生成的纪要都更干净。可配置开关。

**Architecture:** 复用 `corrector` 的「clean」预设 + 词库,新增 `Corrector::clean_line`;`MeetingSession::stop` 在写 json/纪要**之前**对转写逐行清理(LLM 启用且开关开时)。纯逻辑无新增;清理为集成,真机验证。

**Branch:** `2.x`。前置:M2/M4 已完成(transcribe_meeting 产原始 Transcript;stop 后台写 json/md;corrector 有 process/compose_system_prompt;config.preset_prompt("clean") 可用)。

---

## 范围
**做**:`[meeting] clean_transcript` 开关(默认 true);`Corrector::clean_line`;stop 后台清理逐行;lib.rs 传开关。
**不做**:批量/整篇单次清理(本期逐段,复用听写同款路径,最稳);离线 example 的清理(example 保持原始,仅验证流水线)。

---

## Task 1: clean_transcript 配置(TDD)

**Files:** `src/config.rs`(+测试)。

- [ ] **Step 1 失败测试**(config tests mod):
```rust
#[test]
fn meeting_clean_transcript_defaults_true() {
    assert!(MeetingConfig::default().clean_transcript);
}
```
Run `cargo test -p voice-input meeting_clean_transcript_defaults_true` → 编译失败。

- [ ] **Step 2 实现**:`MeetingConfig` 在 `minutes_prompt` 后加字段:
```rust
    /// 是否对会议转写逐段做 LLM 清理(去语气词/纠错/标点);需 LLM 启用。
    pub clean_transcript: bool,
```
Default 里加 `clean_transcript: true,`。

- [ ] **Step 3 通过 + 提交**:
```powershell
git add src/config.rs
git commit -m "feat(meeting): clean_transcript 开关(默认开)"
```

---

## Task 2: Corrector::clean_line(集成 + 小测)

**Files:** `src/corrector.rs`(+测试)。

- [ ] **Step 1 失败测试**(corrector tests mod):
```rust
#[test]
fn clean_line_disabled_returns_raw() {
    let mut c = cfg();
    c.enabled = false;
    let corrector = Corrector::new(c).unwrap();
    assert_eq!(corrector.clean_line("嗯那个文本啊"), "嗯那个文本啊");
}
```
Run `cargo test -p voice-input clean_line_disabled_returns_raw` → 失败(无方法)。

- [ ] **Step 2 实现**:在 `impl Corrector` 内加(`command` 方法附近):
```rust
    /// 会议转写逐段清理:用「clean」预设(忠实清理,不改写)+ 词库。失败/禁用回退原文。
    pub fn clean_line(&self, text: &str) -> String {
        let sys = compose_system_prompt(
            &crate::config::preset_prompt("clean"),
            self.cfg.vocabulary_line().as_deref(),
            None,
        );
        self.process(text, &sys)
    }
```
(`process` 已会处理 enabled / skip_if_shorter_than / 失败回退。)

- [ ] **Step 3 通过 + 提交**:
```powershell
git add src/corrector.rs
git commit -m "feat(meeting): Corrector::clean_line(clean 预设+词库 逐段清理)"
```

---

## Task 3: stop 后台逐行清理(集成)

**Files:** `src/meeting/pipeline.rs`、`src/meeting/mod.rs`、`src/meeting/session.rs`、`src-tauri/src/lib.rs`。

- [ ] **Step 1**:`src/meeting/pipeline.rs` 末尾加:
```rust
use crate::corrector::Corrector;
use crate::meeting::transcript::Transcript;

/// 对转写逐行做 LLM 清理(就地修改 text)。
pub fn clean_transcript(t: &mut Transcript, corrector: &Corrector) {
    for l in &mut t.lines {
        l.text = corrector.clean_line(&l.text);
    }
}
```
(若 `Transcript`/`Corrector` 已在文件内可见则不重复 import;`Line.text` 是 pub。)
`src/meeting/mod.rs` 追加 re-export:
```rust
pub use pipeline::{clean_transcript, transcribe_meeting};
```
(把原来的 `pub use pipeline::transcribe_meeting;` 合并成这一行。)

- [ ] **Step 2**:`src/meeting/session.rs` 的 `stop` 再加参数 `clean: bool`(放最后)。后台线程在 `Ok(t) =>` 分支,**写 json 之前**插入清理:
```rust
                Ok(mut t) => {
                    if clean && llm.enabled && !llm.base_url.trim().is_empty() {
                        if let Ok(c) = crate::corrector::Corrector::new(llm.clone()) {
                            super::pipeline::clean_transcript(&mut t, &c);
                        }
                    }
                    let json = paths.dir.join(format!("{base}.json"));
                    let _ = std::fs::write(&json, t.to_json());
                    // ……(原纪要生成 + assemble_md 写 md 不变)……
```
注意把分支头从 `Ok(t) =>` 改为 `Ok(mut t) =>`;清理后 t 即为干净版,后续 json/纪要/ md 都用它。其余不变。

- [ ] **Step 3**:`src-tauri/src/lib.rs` 的 `stop_meeting` 给 `sess.stop(...)` 末尾加一个参数:
```rust
            cfg.meeting.clean_transcript,
```
(在 `cfg.meeting.effective_minutes_prompt(),` 之后。)

- [ ] **Step 4 编译 + 测试 + 提交**:
```powershell
cargo build
cargo test -p voice-input
git add src/meeting/pipeline.rs src/meeting/mod.rs src/meeting/session.rs src-tauri/src/lib.rs
git commit -m "feat(meeting): stop 后台对转写逐行 LLM 清理(去错字),再出 json/纪要"
```

---

## Task 4: 真机验收

- [ ] dev 跑一场中文会议 → 结束 → 等「会议成稿」。打开 `<base>.md`:**完整转写**应明显更干净(少错字/无语气词/有标点);纪要也更准。
- [ ] 对比:把 `[meeting] clean_transcript=false` 再录一场 → 转写恢复为原始 SenseVoice 输出(有错字),确认开关生效。
- [ ] 若 LLM 未配置:转写为原始(清理被跳过),不报错。

---

## 自检
- 覆盖:开关(T1)、clean_line(T2)、stop 接清理(T3)。
- 类型一致:`clean_line(&self,&str)->String`、`clean_transcript(&mut Transcript,&Corrector)`、`stop(...,clean:bool)`。
- 风险:逐段 N 次 LLM 调用——会议在后台跑、不卡 UI;长会调用多,开关可关。清理在写 json 前 → 重新生成纪要(读 json)也基于干净文本。
