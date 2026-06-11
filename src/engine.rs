//! 听写引擎主循环:热键 → 录音 → ASR →(LLM 整理/翻译/命令)→ 注入。
//! 命令行 bin 与 Tauri 应用共用此入口。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{thread, time::Duration};

use arboard::Clipboard;
use crossbeam_channel::{unbounded, Sender};

use crate::asr::Transcriber;
use crate::audio::Recorder;
use crate::config::Config;
use crate::corrector::Corrector;
use crate::foreground::foreground_process_name;
use crate::hotkey::{self, HotkeyAction};
use crate::inject::inject_text;
use crate::keys::vk_from_name;
use crate::sound::SoundPlayer;

/// 浮窗要展示的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Recording,
    Processing,
    Done,
    Cancelled,
    Failed,
}

/// 引擎状态观察者(默认空实现,CLI 用)。
pub trait EngineObserver: Send + Sync {
    /// 引擎就绪后回调一次,交回可注入取消的句柄。
    /// GUI 实现应**保存**此句柄(供"取消录音"命令调用),不要直接丢弃。
    fn on_ready(&self, _control: ControlHandle) {}
    /// 录音/处理状态变化。
    fn on_state(&self, _state: OverlayState) {}
}

/// 什么都不做的观察者(命令行版用,行为与旧版一致)。
pub struct NoopObserver;
impl EngineObserver for NoopObserver {}

/// 交给 GUI 的取消句柄:内含动作发送端克隆,可从外部注入"取消录音"。
pub struct ControlHandle {
    tx: Sender<HotkeyAction>,
    dictation_suspended: Arc<AtomicBool>,
}

impl ControlHandle {
    /// 请求取消当前录音(等价于按 Esc / 点药丸)。
    pub fn cancel(&self) {
        let _ = self.tx.send(HotkeyAction::CancelRecording);
    }
    /// 设置是否挂起听写(会议用麦期间置位)。
    pub fn set_dictation_suspended(&self, on: bool) {
        self.dictation_suspended.store(on, Ordering::SeqCst);
    }
    /// 仅测试用构造。
    #[cfg(test)]
    pub fn new_for_test(tx: Sender<HotkeyAction>, flag: Arc<AtomicBool>) -> ControlHandle {
        ControlHandle { tx, dictation_suspended: flag }
    }
}

/// 计算浮窗在某显示器底部正中的左上角坐标(纯函数,便于测试)。
pub fn bottom_center(
    area_w: i32,
    area_h: i32,
    win_w: i32,
    win_h: i32,
    bottom_margin: i32,
) -> (i32, i32) {
    ((area_w - win_w) / 2, area_h - win_h - bottom_margin)
}

/// 阻塞运行听写引擎(命令行用:无浮窗)。
pub fn run(config: Config) -> anyhow::Result<()> {
    run_with(config, Arc::new(NoopObserver))
}

/// 阻塞运行听写引擎,并把状态变化回调给观察者(GUI 用)。
pub fn run_with(config: Config, observer: Arc<dyn EngineObserver>) -> anyhow::Result<()> {
    let primary = vk_from_name(&config.hotkey.primary)?;
    let mod_a = vk_from_name(&config.hotkey.translate_modifier)?;
    let mod_b = vk_from_name(&config.hotkey.command_modifier)?;

    let mut transcriber = Transcriber::load(&config.asr.model_dir, &config.asr.language)?;
    let corrector = Corrector::new(config.llm.clone())?;
    println!(
        "ByType 引擎就绪。{}=识别整理;{}+{}=翻译英文;{}+{}=对选中文字执行语音命令。",
        config.hotkey.primary,
        config.hotkey.primary,
        config.hotkey.translate_modifier,
        config.hotkey.primary,
        config.hotkey.command_modifier,
    );

    let (tx, rx) = unbounded::<HotkeyAction>();
    let dictation_suspended = Arc::new(AtomicBool::new(false));
    observer.on_ready(ControlHandle { tx: tx.clone(), dictation_suspended: dictation_suspended.clone() });
    thread::spawn(move || {
        if let Err(e) = hotkey::run(tx, primary, mod_a, mod_b) {
            eprintln!("钩子线程退出: {e}");
        }
    });

    let player = if config.sound.enabled {
        Some(SoundPlayer::from_config(&config.sound))
    } else {
        None
    };
    let mut recorder: Option<Recorder> = None;
    for action in rx.iter() {
        match action {
            HotkeyAction::StartRecording => {
                if dictation_suspended.load(Ordering::SeqCst) {
                    continue; // 会议占麦期间,忽略听写开始
                }
                match Recorder::start() {
                    Ok(r) => {
                        recorder = Some(r);
                        if let Some(p) = &player {
                            p.play_start();
                        }
                        observer.on_state(OverlayState::Recording);
                    }
                    Err(e) => {
                        eprintln!("录音启动失败: {e}");
                        observer.on_state(OverlayState::Failed);
                    }
                }
            }
            HotkeyAction::CancelRecording | HotkeyAction::DiscardRecording => {
                // 仅当确有进行中的录音才通知取消;否则(如迟到的取消、空取消)
                // 静默忽略,避免在 Done 之后再发一个多余的 Cancelled。
                let was_recording = recorder.is_some();
                recorder = None;
                if was_recording {
                    observer.on_state(OverlayState::Cancelled);
                }
            }
            action @ (HotkeyAction::StopAndTranscribe
            | HotkeyAction::StopAndTranslate
            | HotkeyAction::StopAndCommand) => {
                // 取消(Esc/鼠标)经同一 channel 注入。若取消消息在本次 Stop* 取走
                // recorder 之后才到达,本轮会照常完成并发 Done;那条迟到的
                // CancelRecording 因 recorder 已为 None 被上面的 was_recording 守卫忽略。
                let Some(r) = recorder.take() else { continue };
                if let Some(p) = &player {
                    p.play_end();
                }
                observer.on_state(OverlayState::Processing);
                let (samples, rate) = r.stop();
                let raw = match transcriber.transcribe(&samples, rate) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("识别失败: {e}");
                        observer.on_state(OverlayState::Failed);
                        continue;
                    }
                };
                println!("识别: {raw}");
                let style = foreground_process_name().and_then(|p| config.style_for(&p));
                let outcome: anyhow::Result<()> = match action {
                    HotkeyAction::StopAndCommand => handle_command(&corrector, &raw),
                    HotkeyAction::StopAndTranslate => {
                        let text = corrector.translate(&raw, style.as_deref());
                        if text != raw {
                            println!("翻译: {text}");
                        }
                        inject_text(&text)
                    }
                    _ => {
                        let text = corrector.correct(&raw, style.as_deref());
                        if text != raw {
                            println!("修整: {text}");
                        }
                        inject_text(&text)
                    }
                };
                match outcome {
                    Ok(()) => observer.on_state(OverlayState::Done),
                    Err(e) => {
                        eprintln!("注入失败: {e}");
                        observer.on_state(OverlayState::Failed);
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_command(corrector: &Corrector, instruction: &str) -> anyhow::Result<()> {
    let selected = match copy_selection() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取选中文本失败: {e}");
            String::new()
        }
    };
    if selected.trim().is_empty() {
        let text = corrector.correct(instruction, None);
        inject_text(&text)?;
        return Ok(());
    }
    let result = corrector.command(instruction, &selected);
    println!("命令: {instruction}\n结果: {result}");
    inject_text(&result)?;
    Ok(())
}

fn copy_selection() -> anyhow::Result<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL,
    };
    fn k(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
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
    let mut clipboard = Clipboard::new()?;
    let previous = clipboard.get_text().ok();
    let _ = clipboard.set_text(String::new());
    let inputs = [
        k(VK_CONTROL.0, KEYBD_EVENT_FLAGS(0)),
        k(VK_C.0, KEYBD_EVENT_FLAGS(0)),
        k(VK_C.0, KEYEVENTF_KEYUP),
        k(VK_CONTROL.0, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
    thread::sleep(Duration::from_millis(120));
    let selected = clipboard.get_text().unwrap_or_default();
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_handle_toggles_dictation_suspend() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = Arc::new(AtomicBool::new(false));
        let (tx, _rx) = crossbeam_channel::unbounded();
        let h = ControlHandle::new_for_test(tx, flag.clone());
        assert!(!flag.load(Ordering::SeqCst));
        h.set_dictation_suspended(true);
        assert!(flag.load(Ordering::SeqCst));
        h.set_dictation_suspended(false);
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_center_centers_horizontally_and_offsets_bottom() {
        // 1920x1080 屏,240x64 窗,底部留 80
        let (x, y) = bottom_center(1920, 1080, 240, 64, 80);
        assert_eq!(x, (1920 - 240) / 2); // 840
        assert_eq!(y, 1080 - 64 - 80); // 936
    }

    #[test]
    fn bottom_center_handles_window_wider_than_area() {
        // 窗比屏宽时 x 可能为负(调用方可自行 clamp;此处只验证公式)
        let (x, _y) = bottom_center(100, 100, 240, 64, 10);
        assert_eq!(x, (100 - 240) / 2);
    }
}
