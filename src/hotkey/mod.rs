pub mod state;

use std::sync::OnceLock;
use std::time::Instant;

use crossbeam_channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_LMENU, VK_LWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use state::{Action, Decision, Event, HotkeyState};

/// 钩子触发后向主线程发送的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    StartRecording,
    CancelRecording,
    /// 普通模式(单独左 Win)。
    StopAndTranscribe,
    /// 翻译模式(左 Win + 左 Alt)。
    StopAndTranslate,
    DiscardRecording,
}

struct HookCtx {
    state: HotkeyState,
    down_at: Option<Instant>,
    sender: Sender<HotkeyAction>,
}

static CTX: OnceLock<std::sync::Mutex<HookCtx>> = OnceLock::new();

/// 钩子层把原始按键归类成几种事件类型。
enum EventKind {
    WinDown,
    WinUp,
    AltDown,
    AltUp,
    Other,
}

fn classify(vk: u16, msg: u32) -> Option<EventKind> {
    let is_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
    if vk == VK_LWIN.0 {
        if is_down {
            Some(EventKind::WinDown)
        } else if is_up {
            Some(EventKind::WinUp)
        } else {
            None
        }
    } else if vk == VK_LMENU.0 {
        if is_down {
            Some(EventKind::AltDown)
        } else if is_up {
            Some(EventKind::AltUp)
        } else {
            None
        }
    } else if is_down {
        Some(EventKind::Other)
    } else {
        None
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);

    // 忽略本程序自己注入的事件(伪装 Win 弹起、Ctrl+V 粘贴),否则会反噬状态机。
    if kb.dwExtraInfo == crate::INJECTED_TAG {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let vk = kb.vkCode as u16;
    let msg = wparam.0 as u32;

    if let Some(kind) = classify(vk, msg) {
        let is_win_up = matches!(kind, EventKind::WinUp);
        let mut suppress = false;
        if let Some(ctx_lock) = CTX.get() {
            let mut ctx = ctx_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let ev = match kind {
                EventKind::WinDown => {
                    if ctx.down_at.is_none() {
                        ctx.down_at = Some(Instant::now());
                    }
                    Event::HotkeyDown
                }
                EventKind::WinUp => {
                    let held = ctx
                        .down_at
                        .take()
                        .map(|t| t.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    Event::HotkeyUp { held_ms: held }
                }
                EventKind::AltDown => Event::AltDown,
                EventKind::AltUp => Event::AltUp,
                EventKind::Other => Event::OtherDown,
            };
            let decision: Decision = ctx.state.handle(ev);
            dispatch(&ctx.sender, decision.action);
            suppress = decision.suppress;
            // 锁在此作用域结束时释放,随后再注入,避免持锁注入。
        }
        if suppress {
            // 只有"单独 Win 弹起"需要伪装释放(吞物理弹起 + 补发 Ctrl 轻敲 + 合成 Win 弹起)。
            // Alt 的吞掉只需 return 1,不需要伪装。
            if is_win_up {
                disguise_release_win();
            }
            return LRESULT(1);
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn dispatch(sender: &Sender<HotkeyAction>, action: Action) {
    let mapped = match action {
        Action::StartRecording => Some(HotkeyAction::StartRecording),
        Action::CancelRecording => Some(HotkeyAction::CancelRecording),
        Action::StopAndTranscribe => Some(HotkeyAction::StopAndTranscribe),
        Action::StopAndTranslate => Some(HotkeyAction::StopAndTranslate),
        Action::DiscardRecording => Some(HotkeyAction::DiscardRecording),
        Action::None => None,
    };
    if let Some(a) = mapped {
        let _ = sender.send(a);
    }
}

/// 吞掉物理 Win 弹起后调用:补发 `Ctrl 轻敲` + `合成 Win 弹起`。
/// - `Ctrl` 轻敲让系统认为 Win 不是"单独按放",从而不弹开始菜单;
/// - 合成的 Win 弹起让系统知道 Win 已松开,避免 Win 被"卡住"(后续按键变成 Win+键)。
/// 所有事件带 `INJECTED_TAG` 标记,会被本钩子忽略。
unsafe fn disguise_release_win() {
    let inputs = [
        tagged_key(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        tagged_key(VK_CONTROL.0, KEYEVENTF_KEYUP),
        tagged_key(VK_LWIN.0, KEYEVENTF_KEYUP),
    ];
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

fn tagged_key(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::INJECTED_TAG,
            },
        },
    }
}

/// 安装钩子并进入消息循环(阻塞当前线程)。
pub fn run(sender: Sender<HotkeyAction>) -> anyhow::Result<()> {
    let _ = CTX.set(std::sync::Mutex::new(HookCtx {
        state: HotkeyState::default(),
        down_at: None,
        sender,
    }));
    unsafe {
        // SetWindowsHookExW 在失败时返回 Err,? 直接向上传播。
        let _hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)?;
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    }
    Ok(())
}
