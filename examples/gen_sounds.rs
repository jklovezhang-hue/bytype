//! 合成两个柔和提示音并写入 assets/sounds/。
//! 重新生成:cargo run --example gen_sounds
//! 开始音=上行两音(亮),结束音=下行两音(沉);正弦 + 快起音 + 指数衰减,柔和电平。

use std::f32::consts::PI;

fn main() -> anyhow::Result<()> {
    std::fs::create_dir_all("assets/sounds")?;
    // (频率 Hz, 时长 s)序列,依次拼接
    write_tone("assets/sounds/start.wav", &[(880.0, 0.07), (1318.5, 0.10)])?; // A5 → E6 上行
    write_tone("assets/sounds/end.wav", &[(659.3, 0.07), (440.0, 0.12)])?; // E5 → A4 下行
    println!("wrote assets/sounds/start.wav and assets/sounds/end.wav");
    Ok(())
}

fn write_tone(path: &str, notes: &[(f32, f32)]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    let sr = 44_100.0_f32;
    let peak = 0.3_f32; // 柔和电平
    let attack = 0.005_f32; // 5ms 起音,避免爆音
    for &(freq, dur) in notes {
        let n = (dur * sr) as usize;
        for s in 0..n {
            let t = s as f32 / sr;
            let env = if t < attack {
                t / attack
            } else {
                (-(t - attack) * 6.0).exp() // 指数衰减
            };
            let sample = (2.0 * PI * freq * t).sin() * env * peak;
            w.write_sample((sample * i16::MAX as f32) as i16)?;
        }
    }
    w.finalize()?;
    Ok(())
}
