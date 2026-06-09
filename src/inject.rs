use std::{thread, time::Duration};
use arboard::Clipboard;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, INPUT_0, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};

/// 备份剪贴板 → 写入文本 → 发 Ctrl+V → 还原剪贴板。
pub fn inject_text(text: &str) -> anyhow::Result<()> {
    let text = prepare_text(text);
    if text.is_empty() {
        return Ok(());
    }
    let mut clipboard = Clipboard::new()?;
    let previous = clipboard.get_text().ok();

    clipboard.set_text(text)?;
    thread::sleep(Duration::from_millis(30));
    send_ctrl_v();
    thread::sleep(Duration::from_millis(80));

    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(())
}

fn key_input(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_ctrl_v() {
    let inputs = [
        key_input(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_V.0, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_V.0, KEYEVENTF_KEYUP),
        key_input(VK_CONTROL.0, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

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
