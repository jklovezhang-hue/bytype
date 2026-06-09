use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// 一次录音会话:持有 cpal 流,边录边把样本下混成单声道累积。
pub struct Recorder {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

impl Recorder {
    /// 在默认输入设备上开始录音。
    pub fn start() -> anyhow::Result<Recorder> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("没有可用的麦克风输入设备"))?;
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let buf_cb = buffer.clone();

        let err_fn = |e| eprintln!("录音流错误: {e}");
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut b = buf_cb.lock().unwrap();
                // 下混成单声道:每 channels 个样本取平均
                for frame in data.chunks(channels) {
                    let sum: f32 = frame.iter().sum();
                    b.push(sum / channels as f32);
                }
            },
            err_fn,
            None,
        )?;
        stream.play()?;

        Ok(Recorder { _stream: stream, buffer, sample_rate })
    }

    /// 停止并取出 (单声道样本, 采样率)。sherpa-onnx 会内部重采样到 16k。
    pub fn stop(self) -> (Vec<f32>, u32) {
        drop(self._stream);
        let samples = self.buffer.lock().unwrap().clone();
        (samples, self.sample_rate)
    }
}
