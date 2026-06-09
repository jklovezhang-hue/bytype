/// 注入前清理识别/纠错产生的多余包裹与空白。
pub fn prepare_text(raw: &str) -> String {
    let mut s = raw.trim();
    // 去掉 ``` 代码围栏(可能带语言标注)
    if s.starts_with("```") {
        s = s.trim_start_matches("```");
        if let Some(idx) = s.find('\n') {
            s = &s[idx + 1..];
        }
        s = s.trim_end_matches("```").trim();
    }
    // 去掉成对的首尾英文/中文双引号
    let s = s.trim();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        let is_pair = (first == '"' && last == '"') || (first == '\u{201c}' && last == '\u{201d}');
        if is_pair {
            return chars[1..chars.len() - 1].iter().collect::<String>().trim().to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_surrounding_whitespace_and_newlines() {
        assert_eq!(prepare_text("  你好世界 \n"), "你好世界");
    }

    #[test]
    fn strips_wrapping_double_quotes() {
        assert_eq!(prepare_text("\"hello world\""), "hello world");
    }

    #[test]
    fn strips_code_fences() {
        assert_eq!(prepare_text("```\n打开文件\n```"), "打开文件");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(prepare_text("   "), "");
    }

    #[test]
    fn keeps_internal_content_untouched() {
        assert_eq!(prepare_text("今天 weather 不错"), "今天 weather 不错");
    }
}
