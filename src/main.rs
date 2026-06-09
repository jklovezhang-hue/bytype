use crossbeam_channel::unbounded;
use voice_input::asr::Transcriber;
use voice_input::audio::Recorder;
use voice_input::config::Config;
use voice_input::corrector::Corrector;
use voice_input::hotkey::{self, HotkeyAction};
use voice_input::inject::inject_text;

fn main() -> anyhow::Result<()> {
    let config = Config::load("config.toml")?;
    let mut transcriber = Transcriber::load(&config.asr.model_dir, &config.asr.language)?;
    let corrector = Corrector::new(config.llm.clone())?;
    if config.llm.enabled {
        println!("SenseVoice + LLM 纠错已就绪。按住左 Win 说话,松开输出。");
    } else {
        println!("SenseVoice 已就绪(LLM 纠错已禁用)。按住左 Win 说话,松开输出。");
    }

    let (tx, rx) = unbounded::<HotkeyAction>();

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
                recorder = None;
            }
            HotkeyAction::StopAndTranscribe => {
                if let Some(r) = recorder.take() {
                    let (samples, rate) = r.stop();
                    match transcriber.transcribe(&samples, rate) {
                        Ok(raw) => {
                            println!("识别: {raw}");
                            let text = corrector.correct(&raw);
                            if text != raw {
                                println!("修整: {text}");
                            }
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
