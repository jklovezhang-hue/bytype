pub mod engine;
pub mod hotkey;
pub mod audio;
pub mod asr;
pub mod inject;
pub mod config;
pub mod corrector;
pub mod keys;
pub mod foreground;
pub mod sound;
pub mod readiness;
pub mod download;
pub mod meeting;

/// 本程序通过 SendInput 注入的按键事件,在 `dwExtraInfo` 上打的标记。
/// 低级键盘钩子据此忽略自己注入的事件(伪装 Win 弹起、Ctrl+V 粘贴),避免反噬。
pub const INJECTED_TAG: usize = 0x564F_4943; // "VOIC"
