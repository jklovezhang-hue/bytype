//! 离线把一场会议的 WAV 转写成 md/json。
//! 用法: cargo run --example meeting_process -- <dir> <base>
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).expect("用法: meeting_process <dir> <base>"));
    let base = args.get(2).expect("缺 base").to_string();
    let mic = dir.join(format!("{base}.mic.wav"));
    let sys = dir.join(format!("{base}.system.wav"));
    let t = voice_input::meeting::transcribe_meeting(
        &base,
        &mic,
        &sys,
        "./models/sensevoice",
        "auto",
        "./models/silero_vad.onnx",
        voice_input::meeting::DiarOpts {
            enabled: true,
            segmentation_model: "./models/segmentation.onnx",
            embedding_model: "./models/speaker_embedding.onnx",
            speakers: 0,
        },
    )?;
    std::fs::write(dir.join(format!("{base}.md")), t.to_markdown())?;
    std::fs::write(dir.join(format!("{base}.json")), t.to_json())?;
    println!("已写 {base}.md / {base}.json,共 {} 行", t.lines.len());
    Ok(())
}
