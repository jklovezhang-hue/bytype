use anyhow::Result;
use crate::asr::Transcriber;

/// 会议转写器:封装一个已加载的 SenseVoice,对每段 16k 单声道样本识别。
pub struct SegmentTranscriber {
    inner: Transcriber,
}

impl SegmentTranscriber {
    pub fn load(model_dir: &str, language: &str) -> Result<SegmentTranscriber> {
        Ok(SegmentTranscriber { inner: Transcriber::load(model_dir, language)? })
    }

    /// 识别一段 16k 单声道样本;失败或空返回空串。
    pub fn transcribe_seg(&mut self, samples: &[f32]) -> String {
        if samples.is_empty() {
            return String::new();
        }
        self.inner.transcribe(samples, 16000).unwrap_or_default().trim().to_string()
    }
}
