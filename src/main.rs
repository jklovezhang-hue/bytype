use crossbeam_channel::unbounded;
use voice_input::asr::Transcriber;
use voice_input::audio::Recorder;
use voice_input::hotkey::{self, HotkeyAction};
use voice_input::inject::inject_text;

fn main() -> anyhow::Result<()> {
    let mut transcriber = Transcriber::load()?;
    println!("SenseVoice 已加载。按住左 Win 说话,松开输出。");

    let (tx, rx) = unbounded::<HotkeyAction>();

    // 钩子需在自己的线程跑消息循环。
    std::thread::spawn(move || {
        if let Err(e) = hotkey::run(tx) {
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
                recorder = None; // drop 即停止
            }
            HotkeyAction::StopAndTranscribe => {
                if let Some(r) = recorder.take() {
                    let (samples, rate) = r.stop();
                    match transcriber.transcribe(&samples, rate) {
                        Ok(text) => {
                            println!("识别: {text}");
                            if let Err(e) = inject_text(&text) {
                                eprintln!("注入失败: {e}");
                            }
                        }
                        Err(e) => eprintln!("识别失败: {e}"),
                    }
                }
            }
        }
    }
    Ok(())
}
