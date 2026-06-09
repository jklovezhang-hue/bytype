pub mod state;

use std::sync::OnceLock;
use std::time::Instant;

use crossbeam_channel::Sender;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use state::{Decision, Event, HotkeyState};

/// 钩子触发后向主线程发送的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    StartRecording,
    CancelRecording,
    StopAndTranscribe,
    StopAndTranslate,
    StopAndCommand,
    DiscardRecording,
}

struct HookCtx {
    state: HotkeyState,
    down_at: Option<Instant>,
    sender: Sender<HotkeyAction>,
    primary_vk: u16,
    mod_a_vk: u16,
    mod_b_vk: u16,
}

static CTX: OnceLock<std::sync::Mutex<HookCtx>> = OnceLock::new();

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    if kb.dwExtraInfo == crate::INJECTED_TAG {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }
    let vk = kb.vkCode as u16;
    let msg = wparam.0 as u32;
    let is_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
    if !is_down && !is_up {
        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
    }

    let mut suppress = false;
    let mut primary_vk = 0u16;
    let mut is_primary_up = false;

    if let Some(ctx_lock) = CTX.get() {
        let mut ctx = ctx_lock.lock().unwrap_or_else(|p| p.into_inner());
        primary_vk = ctx.primary_vk;
        let event = if vk == ctx.primary_vk {
            if is_down {
                if ctx.down_at.is_none() {
                    ctx.down_at = Some(Instant::now());
                }
                Some(Event::PrimaryDown)
            } else {
                let held = ctx
                    .down_at
                    .take()
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                is_primary_up = true;
                Some(Event::PrimaryUp { held_ms: held })
            }
        } else if vk == ctx.mod_a_vk {
            Some(if is_down { Event::ModADown } else { Event::ModAUp })
        } else if vk == ctx.mod_b_vk {
            Some(if is_down { Event::ModBDown } else { Event::ModBUp })
        } else if vk == VK_ESCAPE.0 && is_down {
            Some(Event::EscDown)
        } else if is_down {
            Some(Event::OtherDown)
        } else {
            None
        };

        if let Some(event) = event {
            let decision: Decision = ctx.state.handle(event);
            dispatch(&ctx.sender, decision.action);
            suppress = decision.suppress;
        }
    }

    if suppress {
        if is_primary_up {
            disguise_release_key(primary_vk);
        }
        return LRESULT(1);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

fn dispatch(sender: &Sender<HotkeyAction>, action: state::Action) {
    use state::Action;
    let mapped = match action {
        Action::StartRecording => Some(HotkeyAction::StartRecording),
        Action::CancelRecording => Some(HotkeyAction::CancelRecording),
        Action::StopAndTranscribe => Some(HotkeyAction::StopAndTranscribe),
        Action::StopAndTranslate => Some(HotkeyAction::StopAndTranslate),
        Action::StopAndCommand => Some(HotkeyAction::StopAndCommand),
        Action::DiscardRecording => Some(HotkeyAction::DiscardRecording),
        Action::None => None,
    };
    if let Some(a) = mapped {
        let _ = sender.send(a);
    }
}

/// 吞掉物理主键弹起后:补发 Ctrl 轻敲(防开始菜单)+ 合成主键弹起(解除卡键)。
unsafe fn disguise_release_key(primary_vk: u16) {
    let inputs = [
        tagged_key(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        tagged_key(VK_CONTROL.0, KEYEVENTF_KEYUP),
        tagged_key(primary_vk, KEYEVENTF_KEYUP),
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

/// 安装钩子并进入消息循环(阻塞)。需提供三个热键 VK。
pub fn run(
    sender: Sender<HotkeyAction>,
    primary_vk: u16,
    mod_a_vk: u16,
    mod_b_vk: u16,
) -> anyhow::Result<()> {
    let _ = CTX.set(std::sync::Mutex::new(HookCtx {
        state: HotkeyState::default(),
        down_at: None,
        sender,
        primary_vk,
        mod_a_vk,
        mod_b_vk,
    }));
    unsafe {
        let _hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)?;
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    }
    Ok(())
}
