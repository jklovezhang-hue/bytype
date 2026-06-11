use anyhow::{anyhow, Result};
use sherpa_rs::silero_vad::{SileroVad, SileroVadConfig};

/// 一段语音:起始采样点(16k 域)+ 单声道样本。
#[derive(Debug, Clone)]
pub struct Segment {
    pub start_sample: usize,
    pub samples: Vec<f32>,
}

impl Segment {
    pub fn start_ms(&self) -> u64 {
        (self.start_sample as u64) * 1000 / 16000
    }
    pub fn end_ms(&self) -> u64 {
        ((self.start_sample + self.samples.len()) as u64) * 1000 / 16000
    }
}

/// 用 Silero VAD 把 16k 单声道样本切成语音段。`model` 为 silero_vad.onnx 路径。
pub fn vad_segments(samples: &[f32], model: &str) -> Result<Vec<Segment>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let config = SileroVadConfig {
        model: model.to_string(),
        min_silence_duration: 0.5,
        min_speech_duration: 0.25,
        max_speech_duration: 15.0,
        threshold: 0.5,
        sample_rate: 16000,
        window_size: 512,
        provider: None,
        num_threads: Some(1),
        debug: false,
    };
    let buffer_secs = (samples.len() as f32 / 16000.0) + 5.0;
    let mut vad = SileroVad::new(config, buffer_secs)
        .map_err(|e| anyhow!("创建 Silero VAD 失败(确认模型存在): {e}"))?;

    let mut out = Vec::new();
    let win = 512usize;
    let mut i = 0;
    while i < samples.len() {
        let end = (i + win).min(samples.len());
        vad.accept_waveform(samples[i..end].to_vec());
        while !vad.is_empty() {
            let seg = vad.front();
            out.push(Segment { start_sample: seg.start.max(0) as usize, samples: seg.samples });
            vad.pop();
        }
        i = end;
    }
    vad.flush();
    while !vad.is_empty() {
        let seg = vad.front();
        out.push(Segment { start_sample: seg.start.max(0) as usize, samples: seg.samples });
        vad.pop();
    }
    Ok(out)
}
