use std::path::Path;
use anyhow::{anyhow, Context, Result};
use mp3lame_encoder::{Builder, FlushNoGap, MonoPcm};

/// 把 16k 单声道 i16 PCM 编码为 MP3 写到 `path`,`bitrate_kbps` 如 48。
pub fn encode_mp3_file(path: &Path, pcm: &[i16], bitrate_kbps: u32) -> Result<()> {
    let mut builder = Builder::new().ok_or_else(|| anyhow!("LAME builder 创建失败"))?;
    builder.set_num_channels(1).map_err(|e| anyhow!("set_num_channels: {e:?}"))?;
    builder.set_sample_rate(16000).map_err(|e| anyhow!("set_sample_rate: {e:?}"))?;
    builder
        .set_brate(bitrate_to_lame(bitrate_kbps))
        .map_err(|e| anyhow!("set_brate: {e:?}"))?;
    let mut encoder = builder.build().map_err(|e| anyhow!("LAME build: {e:?}"))?;

    let mut out: Vec<u8> = Vec::with_capacity(pcm.len() / 2 + 7200);
    encoder
        .encode_to_vec(MonoPcm(pcm), &mut out)
        .map_err(|e| anyhow!("encode: {e:?}"))?;
    encoder
        .flush_to_vec::<FlushNoGap>(&mut out)
        .map_err(|e| anyhow!("flush: {e:?}"))?;

    std::fs::write(path, &out).with_context(|| format!("写 MP3 失败: {}", path.display()))?;
    Ok(())
}

/// 把 kbps 映射到 LAME 的 Bitrate 枚举(取最接近的常用档)。
fn bitrate_to_lame(kbps: u32) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate::*;
    match kbps {
        0..=24 => Kbps24,
        25..=32 => Kbps32,
        33..=40 => Kbps40,
        41..=48 => Kbps48,
        49..=64 => Kbps64,
        _ => Kbps96,
    }
}

#[cfg(test)]
mod tests {
    use super::encode_mp3_file;

    #[test]
    fn encodes_mono_16k_to_nonempty_mp3() {
        let path = std::env::temp_dir().join("bytype_test_mp3.mp3");
        let _ = std::fs::remove_file(&path);

        let samples: Vec<i16> = (0..16000)
            .map(|i| ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 8000.0) as i16)
            .collect();
        encode_mp3_file(&path, &samples, 48).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 200, "mp3 太小: {}", bytes.len());
        let has_frame = bytes.windows(2).any(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0);
        assert!(has_frame, "未发现 MP3 帧同步字");

        let _ = std::fs::remove_file(&path);
    }
}
