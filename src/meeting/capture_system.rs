use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::thread::JoinHandle;
use anyhow::{anyhow, Result};
use wasapi::{
    get_default_device, initialize_mta, Direction, SampleType, ShareMode,
};

use super::resample::resample_to_16k;
use super::wav::WavSink;

/// WASAPI 系统环回采集:对默认渲染设备(扬声器/耳机)做 loopback 捕获 →
/// 按设备混音格式解码为 f32 → 下混单声道 → 重采样 16k → 写 WAV。
///
/// 接口与 `MicCapture` 对齐。COM 必须在采集线程内 `initialize_mta()`,与 cpal
/// 麦克风线程隔离,故整个采集循环都跑在独立线程里。
///
/// loopback 原理(wasapi 0.15):取 `Direction::Render` 的默认设备,但
/// `initialize_client(.., direction = Direction::Capture, ShareMode::Shared, ..)`
/// —— crate 内部对 (设备方向=Render, 调用方向=Capture, Shared) 这一组合会带上
/// `AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK`,即环回捕获。
pub struct SystemCapture {
    stop_tx: Sender<()>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl SystemCapture {
    /// 开始环回采集并写入 `path`(16k 单声道 WAV)。
    pub fn start(path: PathBuf) -> Result<SystemCapture> {
        let (stop_tx, stop_rx) = channel::<()>();
        let (ready_tx, ready_rx) = channel::<Result<()>>();

        let handle = std::thread::spawn(move || -> Result<()> {
            // 采集线程内初始化 COM(MTA)。已初始化时返回非致命的 HRESULT,忽略即可。
            let _ = initialize_mta();

            // 整个建客户端 + 采集循环包进闭包,任何失败先经 ready 通道回报,再返回 Err。
            // 闭包内用 ready 通道的克隆报告 start 成功;外层保留原句柄报告 start 前的失败。
            let ready_inner = ready_tx.clone();
            let run = move || -> Result<()> {
                let device = get_default_device(&Direction::Render)
                    .map_err(|e| anyhow!("取默认渲染设备失败: {e}"))?;
                let mut audio_client = device
                    .get_iaudioclient()
                    .map_err(|e| anyhow!("取 IAudioClient 失败: {e}"))?;

                // 共享模式下用设备自身的混音格式(通常 32-bit float 立体声 48k),
                // 这是共享模式必然接受的格式,无需 convert。
                let format = audio_client
                    .get_mixformat()
                    .map_err(|e| anyhow!("取混音格式失败: {e}"))?;
                let src_rate = format.get_samplespersec();
                let channels = format.get_nchannels() as usize;
                let bits = format.get_bitspersample();
                let sample_type = format
                    .get_subformat()
                    .map_err(|e| anyhow!("取采样子格式失败: {e}"))?;
                let block_align = (bits as usize / 8) * channels.max(1);

                let (_def_period, min_period) = audio_client
                    .get_periods()
                    .map_err(|e| anyhow!("取设备周期失败: {e}"))?;

                // 渲染设备 + Capture 方向 + Shared => loopback。convert=false,直接用混音格式。
                audio_client
                    .initialize_client(
                        &format,
                        min_period,
                        &Direction::Capture,
                        &ShareMode::Shared,
                        false,
                    )
                    .map_err(|e| anyhow!("初始化环回客户端失败: {e}"))?;

                let h_event = audio_client
                    .set_get_eventhandle()
                    .map_err(|e| anyhow!("创建事件句柄失败: {e}"))?;
                let capture_client = audio_client
                    .get_audiocaptureclient()
                    .map_err(|e| anyhow!("取捕获客户端失败: {e}"))?;

                let mut sink = WavSink::create(&path)?;

                audio_client
                    .start_stream()
                    .map_err(|e| anyhow!("启动环回流失败: {e}"))?;

                // 客户端就绪,通知主线程 start 成功。
                let _ = ready_inner.send(Ok(()));

                let mut byte_queue: VecDeque<u8> = VecDeque::new();
                loop {
                    // 检查停止信号。
                    match stop_rx.try_recv() {
                        Ok(()) | Err(TryRecvError::Disconnected) => break,
                        Err(TryRecvError::Empty) => {}
                    }

                    // 读一批字节(无声音播放时可能 0 帧)。
                    if let Err(e) = capture_client.read_from_device_to_deque(&mut byte_queue) {
                        eprintln!("环回读取错误: {e}");
                    }

                    if block_align > 0 {
                        // 按整帧消费队列,解码 -> 下混单声道 -> 重采样 -> 写 WAV。
                        let whole = byte_queue.len() - (byte_queue.len() % block_align);
                        if whole > 0 {
                            let bytes: Vec<u8> = byte_queue.drain(..whole).collect();
                            let mono = decode_to_mono(&bytes, bits, &sample_type, channels);
                            let r = resample_to_16k(&mono, src_rate);
                            sink.append_f32(&r);
                        }
                    }

                    // 等下一批事件;无声音时会超时,不算错误,继续循环(M1 不补零)。
                    let _ = h_event.wait_for_event(200);
                }

                let _ = audio_client.stop_stream();
                sink.finalize()?;
                Ok(())
            };

            match run() {
                Ok(()) => Ok(()),
                Err(e) => {
                    // ready 可能已发过 Ok;失败发到 ready 仅在尚未发送时有意义,
                    // 接收端只读一次,这里再发不会造成错配(满了/已读则被忽略)。
                    let _ = ready_tx.send(Err(anyhow!("{e}")));
                    Err(e)
                }
            }
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(SystemCapture { stop_tx, handle: Some(handle) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("系统环回采集线程未启动")),
        }
    }

    /// 停止采集并 finalize WAV。
    pub fn stop(mut self) -> Result<()> {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() {
            h.join().map_err(|_| anyhow!("系统环回线程 join 失败"))??;
        }
        Ok(())
    }
}

/// 把原始字节按混音格式解码成 f32,再下混成单声道([-1,1])。
/// 支持常见的 32-bit float、16-bit int、32-bit int。
fn decode_to_mono(bytes: &[u8], bits: u16, sample_type: &SampleType, channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    // 先解成逐采样 f32(交错多声道)。
    let samples: Vec<f32> = match (sample_type, bits) {
        (SampleType::Float, 32) => bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        (SampleType::Int, 16) => bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect(),
        (SampleType::Int, 32) => bytes
            .chunks_exact(4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0)
            .collect(),
        // 其他位深(如 24-bit)M1 不处理:返回静音,避免产生噪声。真机如遇到再补。
        _ => return Vec::new(),
    };

    let mut mono = Vec::with_capacity(samples.len() / channels);
    for frame in samples.chunks(channels) {
        let sum: f32 = frame.iter().copied().sum();
        mono.push(sum / channels as f32);
    }
    mono
}
