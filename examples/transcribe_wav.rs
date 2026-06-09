//! 离线验证 ASR:读取一个 WAV 文件,喂给 Transcriber,打印识别文本。
//!
//! 用法:cargo run --example transcribe_wav -- <path-to.wav>
//! 需要模型已放在 ./models/sensevoice/(model.onnx + tokens.txt)。

use voice_input::asr::Transcriber;

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("用法: cargo run --example transcribe_wav -- <path.wav>"))?;

    let mut reader = hound::WavReader::open(&path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    // 读成 f32 单声道。
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max)
                .collect()
        }
    };
    let mono: Vec<f32> = if channels <= 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|f| f.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    println!(
        "文件: {path}  采样率: {} Hz  声道: {channels}  样本数(单声道): {}",
        spec.sample_rate,
        mono.len()
    );

    let mut transcriber = Transcriber::load()?;
    let text = transcriber.transcribe(&mono, spec.sample_rate)?;
    println!("识别: {text}");
    Ok(())
}
