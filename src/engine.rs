//! 听写引擎主循环:热键 → 录音 → ASR →(LLM 整理/翻译/命令)→ 注入。
//! 命令行 bin 与 Tauri 应用共用此入口。

use std::{thread, time::Duration};

use arboard::Clipboard;
use crossbeam_channel::unbounded;

use crate::asr::Transcriber;
use crate::audio::Recorder;
use crate::config::Config;
use crate::corrector::Corrector;
use crate::foreground::foreground_process_name;
use crate::hotkey::{self, HotkeyAction};
use crate::inject::inject_text;
use crate::keys::vk_from_name;

/// 阻塞运行听写引擎(安装键盘钩子并进入事件循环)。
pub fn run(config: Config) -> anyhow::Result<()> {
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
    thread::spawn(move || {
        if let Err(e) = hotkey::run(tx, primary, mod_a, mod_b) {
            eprintln!("钩子线程退出: {e}");
        }
    });

    let mut recorder: Option<Recorder> = None;
    for action in rx.iter() {
        match action {
            HotkeyAction::StartRecording => match Recorder::start() {
                Ok(r) => recorder = Some(r),
                Err(e) => eprintln!("录音启动失败: {e}"),
            },
            HotkeyAction::CancelRecording | HotkeyAction::DiscardRecording => {
                recorder = None;
            }
            action @ (HotkeyAction::StopAndTranscribe
            | HotkeyAction::StopAndTranslate
            | HotkeyAction::StopAndCommand) => {
                let Some(r) = recorder.take() else { continue };
                let (samples, rate) = r.stop();
                let raw = match transcriber.transcribe(&samples, rate) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("识别失败: {e}");
                        continue;
                    }
                };
                println!("识别: {raw}");
                let style = foreground_process_name().and_then(|p| config.style_for(&p));
                match action {
                    HotkeyAction::StopAndCommand => handle_command(&corrector, &raw),
                    HotkeyAction::StopAndTranslate => {
                        let text = corrector.translate(&raw, style.as_deref());
                        if text != raw {
                            println!("翻译: {text}");
                        }
                        if let Err(e) = inject_text(&text) {
                            eprintln!("注入失败: {e}");
                        }
                    }
                    _ => {
                        let text = corrector.correct(&raw, style.as_deref());
                        if text != raw {
                            println!("修整: {text}");
                        }
                        if let Err(e) = inject_text(&text) {
                            eprintln!("注入失败: {e}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_command(corrector: &Corrector, instruction: &str) {
    let selected = match copy_selection() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取选中文本失败: {e}");
            String::new()
        }
    };
    if selected.trim().is_empty() {
        let text = corrector.correct(instruction, None);
        let _ = inject_text(&text);
        return;
    }
    let result = corrector.command(instruction, &selected);
    println!("命令: {instruction}\n结果: {result}");
    if let Err(e) = inject_text(&result) {
        eprintln!("注入失败: {e}");
    }
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
