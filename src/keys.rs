//! 把配置里的按键名(如 "LWin")解析成 Win32 虚拟键码。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};

/// 解析按键名 → VK 码。大小写不敏感。未知名报错。
pub fn vk_from_name(name: &str) -> anyhow::Result<u16> {
    let vk = match name.trim().to_ascii_uppercase().as_str() {
        "LWIN" => VK_LWIN,
        "RWIN" => VK_RWIN,
        "LALT" => VK_LMENU,
        "RALT" => VK_RMENU,
        "LCTRL" => VK_LCONTROL,
        "RCTRL" => VK_RCONTROL,
        "LSHIFT" => VK_LSHIFT,
        "RSHIFT" => VK_RSHIFT,
        other => anyhow::bail!(
            "未知热键名: {other}(可用:LWin RWin LAlt RAlt LCtrl RCtrl LShift RShift)"
        ),
    };
    Ok(vk.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LCONTROL, VK_LMENU, VK_LWIN};

    #[test]
    fn parses_known_names() {
        assert_eq!(vk_from_name("LWin").unwrap(), VK_LWIN.0);
        assert_eq!(vk_from_name("LAlt").unwrap(), VK_LMENU.0);
        assert_eq!(vk_from_name("LCtrl").unwrap(), VK_LCONTROL.0);
    }

    #[test]
    fn is_case_insensitive_and_trims() {
        assert_eq!(vk_from_name("  lwin ").unwrap(), VK_LWIN.0);
    }

    #[test]
    fn unknown_name_errors() {
        assert!(vk_from_name("Banana").is_err());
    }
}
