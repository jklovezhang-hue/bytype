use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use super::resample::resample_to_16k;
use super::wav::WavSink;

/// 麦克风采集:cpal 默认输入设备 → 下混单声道 → 重采样 16k → 写 WAV。
///
/// cpal 的 `Stream` 不是 `Send`,必须在创建它的线程里持有和释放。因此整个采集
/// (建流 + play + 等停止信号 + finalize)都跑在一个独立线程内,主线程只通过
/// channel 发停止信号并在 `stop()` 时 join。
pub struct MicCapture {
    stop_tx: Sender<()>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl MicCapture {
    /// 开始采集并写入 `path`(16k 单声道 WAV)。
    pub fn start(path: PathBuf) -> Result<MicCapture> {
        let (stop_tx, stop_rx) = channel::<()>();
        let (ready_tx, ready_rx) = channel::<Result<()>>();
        let handle = std::thread::spawn(move || -> Result<()> {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err(anyhow!("没有可用麦克风")));
                    return Err(anyhow!("没有可用麦克风"));
                }
            };
            let supported = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(anyhow!("读麦克风配置失败: {e}")));
                    return Err(anyhow!("读麦克风配置失败: {e}"));
                }
            };
            let src_rate = supported.sample_rate().0;
            let channels = supported.channels() as usize;
            let fmt = supported.sample_format();
            let config: cpal::StreamConfig = supported.into();

            let sink = match WavSink::create(&path) {
                Ok(s) => std::sync::Arc::new(std::sync::Mutex::new(s)),
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return Err(anyhow!("创建 WAV 失败"));
                }
            };
            let err_fn = |e| eprintln!("麦克风流错误: {e}");

            macro_rules! make_stream {
                ($t:ty, $conv:expr) => {{
                    let sink = sink.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[$t], _: &cpal::InputCallbackInfo| {
                            let mut mono = Vec::with_capacity(data.len() / channels);
                            for frame in data.chunks(channels) {
                                let sum: f32 = frame.iter().map(|&s| ($conv)(s)).sum();
                                mono.push(sum / channels as f32);
                            }
                            let r = resample_to_16k(&mono, src_rate);
                            if let Ok(mut s) = sink.lock() {
                                s.append_f32(&r);
                            }
                        },
                        err_fn,
                        None,
                    )
                }};
            }
            let stream = match fmt {
                SampleFormat::F32 => make_stream!(f32, |s: f32| s),
                SampleFormat::I16 => make_stream!(i16, |s: i16| s as f32 / 32768.0),
                SampleFormat::U16 => make_stream!(u16, |s: u16| (s as f32 - 32768.0) / 32768.0),
                other => {
                    let _ = ready_tx.send(Err(anyhow!("不支持的麦克风格式: {other:?}")));
                    return Err(anyhow!("不支持的麦克风格式"));
                }
            };
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(anyhow!("建流失败: {e}")));
                    return Err(anyhow!("建流失败: {e}"));
                }
            };
            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(anyhow!("播放流失败: {e}")));
                return Err(anyhow!("播放流失败: {e}"));
            }
            let _ = ready_tx.send(Ok(()));
            let _ = stop_rx.recv();
            drop(stream);
            let sink = std::sync::Arc::try_unwrap(sink)
                .map_err(|_| anyhow!("sink 仍被引用"))?
                .into_inner()
                .map_err(|_| anyhow!("sink 锁中毒"))?;
            sink.finalize()?;
            Ok(())
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(MicCapture { stop_tx, handle: Some(handle) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("麦克风采集线程未启动")),
        }
    }

    /// 停止采集并 finalize WAV。
    pub fn stop(mut self) -> Result<()> {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() {
            h.join().map_err(|_| anyhow!("麦克风线程 join 失败"))??;
        }
        Ok(())
    }
}
