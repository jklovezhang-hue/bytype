pub mod state;

use std::sync::OnceLock;
use std::time::Instant;

use crossbeam_channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN;
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
    StopAndTranscribe,
    DiscardRecording,
}

struct HookCtx {
    state: HotkeyState,
    down_at: Option<Instant>,
    sender: Sender<HotkeyAction>,
}

static CTX: OnceLock<std::sync::Mutex<HookCtx>> = OnceLock::new();

fn is_hotkey(vk: u16) -> bool {
    vk == VK_LWIN.0 // 阶段一固定左 Win;阶段二改为可配置
}

enum EventKind {
    Down,
    Other,
    Up,
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode as u16;
    let msg = wparam.0 as u32;

    let kind = if is_hotkey(vk) {
        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(EventKind::Down),
            WM_KEYUP | WM_SYSKEYUP => Some(EventKind::Up),
            _ => None,
        }
    } else if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) {
        Some(EventKind::Other)
    } else {
        None
    };

    if let Some(kind) = kind {
        if let Some(ctx_lock) = CTX.get() {
            let mut ctx = ctx_lock.lock().unwrap();
            let ev = match kind {
                EventKind::Down => {
                    if ctx.down_at.is_none() {
                        ctx.down_at = Some(Instant::now());
                    }
                    Event::Down
                }
                EventKind::Other => Event::Other,
                EventKind::Up => {
                    let held = ctx
                        .down_at
                        .take()
                        .map(|t| t.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    Event::Up { held_ms: held }
                }
            };
            let Decision { action, suppress } = ctx.state.handle(ev);
            dispatch(&ctx.sender, action);
            if suppress {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn dispatch(sender: &Sender<HotkeyAction>, action: Action) {
    let mapped = match action {
        Action::StartRecording => Some(HotkeyAction::StartRecording),
        Action::CancelRecording => Some(HotkeyAction::CancelRecording),
        Action::StopAndTranscribe => Some(HotkeyAction::StopAndTranscribe),
        Action::DiscardRecording => Some(HotkeyAction::DiscardRecording),
        Action::None => None,
    };
    if let Some(a) = mapped {
        let _ = sender.send(a);
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
