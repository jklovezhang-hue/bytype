use super::transcript::{ms_to_clock, Transcript};

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
        assert!(i_min < i_tr);
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
