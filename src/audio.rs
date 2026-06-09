use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

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
        let supported = device.default_input_config()?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();

        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let err_fn = |e| eprintln!("录音流错误: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        push_mono(&buf, data, channels, |s| s);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        push_mono(&buf, data, channels, |s| s as f32 / 32768.0);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        push_mono(&buf, data, channels, |s| (s as f32 - 32768.0) / 32768.0);
                    },
                    err_fn,
                    None,
                )?
            }
            other => anyhow::bail!("不支持的麦克风采样格式: {other:?}"),
        };
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

/// 把多声道帧下混成单声道并以 f32 追加到缓冲。
fn push_mono<T: Copy>(
    buf: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    to_f32: impl Fn(T) -> f32,
) {
    let mut b = buf.lock().unwrap();
    for frame in data.chunks(channels) {
        let sum: f32 = frame.iter().map(|&s| to_f32(s)).sum();
        b.push(sum / channels as f32);
    }
}
