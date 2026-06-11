use serde::{Deserialize, Serialize};

/// 说话人(M2:仅区分我/对方;M3 再细分对方个体)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speaker {
    Me,
    Other,
    OtherId(u32),
}

impl Speaker {
    pub fn label(&self) -> String {
        match self {
            Speaker::Me => "我".to_string(),
            Speaker::Other => "对方".to_string(),
            Speaker::OtherId(n) => format!("对方·说话人{n}"),
        }
    }
}

/// 一行转写:起止毫秒 + 说话人 + 文本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Speaker,
    pub text: String,
}

/// 一场会议的完整转写。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub base: String,
    pub lines: Vec<Line>,
}

/// 毫秒 → 时钟串:>=1 小时为 H:MM:SS,否则 MM:SS。
pub fn ms_to_clock(ms: u64) -> String {
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

impl Transcript {
    /// 归并两条带说话人的行列表,按起始时间升序;丢弃空文本行(trim 后为空)。
    pub fn merge(base: &str, mine: Vec<Line>, theirs: Vec<Line>) -> Transcript {
        let mut lines: Vec<Line> = mine
            .into_iter()
            .chain(theirs)
            .filter(|l| !l.text.trim().is_empty())
            .collect();
        lines.sort_by_key(|l| l.start_ms);
        Transcript { base: base.to_string(), lines }
    }

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

    /// 渲染为结构化 JSON(供重新生成纪要)。
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// 从结构化 JSON 反序列化(供重新生成纪要时回读 `<base>.json`)。
    pub fn from_json(s: &str) -> serde_json::Result<Transcript> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: u64, sp: Speaker, t: &str) -> Line {
        Line { start_ms, end_ms: start_ms + 1000, speaker: sp, text: t.into() }
    }

    #[test]
    fn merge_orders_by_start_time() {
        let mine = vec![line(0, Speaker::Me, "你好"), line(5000, Speaker::Me, "好的")];
        let theirs = vec![line(2000, Speaker::Other, "在吗")];
        let t = Transcript::merge("m", mine, theirs);
        let starts: Vec<u64> = t.lines.iter().map(|l| l.start_ms).collect();
        assert_eq!(starts, vec![0, 2000, 5000]);
        assert_eq!(t.lines[1].speaker, Speaker::Other);
        assert_eq!(t.base, "m");
    }

    #[test]
    fn merge_skips_empty_text() {
        let mine = vec![line(0, Speaker::Me, "  "), line(1000, Speaker::Me, "在")];
        let t = Transcript::merge("m", mine, vec![]);
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.lines[0].text, "在");
    }

    #[test]
    fn ms_to_clock_formats() {
        assert_eq!(super::ms_to_clock(0), "00:00");
        assert_eq!(super::ms_to_clock(5000), "00:05");
        assert_eq!(super::ms_to_clock(65000), "01:05");
        assert_eq!(super::ms_to_clock(3_725_000), "1:02:05");
    }

    #[test]
    fn render_markdown_has_header_and_lines() {
        let t = Transcript {
            base: "2026-06-11_120000".into(),
            lines: vec![line(0, Speaker::Me, "你好"), line(2000, Speaker::Other, "在")],
        };
        let md = t.to_markdown();
        assert!(md.contains("# 会议转写 2026-06-11_120000"));
        assert!(md.contains("`[00:00]` **我**:你好"));
        assert!(md.contains("`[00:02]` **对方**:在"));
    }

    #[test]
    fn speaker_otherid_label() {
        assert_eq!(Speaker::Me.label(), "我");
        assert_eq!(Speaker::Other.label(), "对方");
        assert_eq!(Speaker::OtherId(1).label(), "对方·说话人1");
        assert_eq!(Speaker::OtherId(3).label(), "对方·说话人3");
    }

    #[test]
    fn render_json_roundtrips() {
        let t = Transcript {
            base: "b".into(),
            lines: vec![line(0, Speaker::Me, "hi")],
        };
        let json = t.to_json();
        let back: Transcript = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
