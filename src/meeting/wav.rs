use std::path::Path;
use anyhow::{Context, Result};

/// 16k 单声道 16-bit WAV 流式写入器。边采集边 `append_f32`,结束 `finalize`。
pub struct WavSink {
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
}

impl WavSink {
    /// 在 `path` 新建(覆盖)一个 16k 单声道 16-bit WAV。
    pub fn create(path: &Path) -> Result<WavSink> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(path, spec)
            .with_context(|| format!("创建 WAV 失败: {}", path.display()))?;
        Ok(WavSink { writer })
    }

    /// 追加一批单声道 f32 采样([-1,1]),内部量化为 i16。
    /// -1.0 → i16::MIN(-32768),+1.0 → i16::MAX(32767)。
    pub fn append_f32(&mut self, samples: &[f32]) {
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            let v = if clamped < 0.0 {
                (clamped * -(i16::MIN as f32)).round() as i32
            } else {
                (clamped * i16::MAX as f32).round() as i32
            };
            let v = v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let _ = self.writer.write_sample(v);
        }
    }

    /// 收尾(写入正确的头长度)。
    pub fn finalize(self) -> Result<()> {
        self.writer.finalize().context("WAV finalize 失败")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WavSink;

    #[test]
    fn writes_and_reads_back_16k_mono() {
        let path = std::env::temp_dir().join("bytype_test_wav_roundtrip.wav");
        let _ = std::fs::remove_file(&path);

        let mut sink = WavSink::create(&path).unwrap();
        sink.append_f32(&[0.0, 0.5, -0.5, 1.0]);
        sink.append_f32(&[-1.0]);
        sink.finalize().unwrap();

        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[3], i16::MAX);
        assert_eq!(samples[4], i16::MIN);

        let _ = std::fs::remove_file(&path);
    }
}
