//! SenseVoice 离线识别封装。
//!
//! 基于 sherpa-rs 0.6.8 的 `sherpa_rs::sense_voice::SenseVoiceRecognizer`。
//! 模型文件期望位于 `{model_dir}/model.onnx` 与 `{model_dir}/tokens.txt`。

use anyhow::{anyhow, Context, Result};
use sherpa_rs::sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer};

/// 离线语音识别器。封装一个已加载的 SenseVoice 识别器。
pub struct Transcriber {
    recognizer: SenseVoiceRecognizer,
}

impl Transcriber {
    /// 加载 SenseVoice 离线识别器。
    /// `model_dir` 下应有 model.onnx 与 tokens.txt;`language` 取 "auto"/"zh"/"en" 等。
    pub fn load(model_dir: &str, language: &str) -> Result<Transcriber> {
        let model = format!("{model_dir}/model.onnx");
        let tokens = format!("{model_dir}/tokens.txt");

        let config = SenseVoiceConfig {
            model,
            tokens,
            language: language.to_string(),
            use_itn: true,
            ..Default::default()
        };

        let recognizer = SenseVoiceRecognizer::new(config)
            .map_err(|e| anyhow!("创建 SenseVoice 识别器失败: {e}"))
            .context("加载 SenseVoice 模型失败,请确认模型文件存在")?;

        Ok(Transcriber { recognizer })
    }

    /// 对给定的单声道 f32 采样（采样率 `sample_rate`）执行离线识别，返回识别文本。
    pub fn transcribe(&mut self, samples: &[f32], sample_rate: u32) -> Result<String> {
        // 注意 sherpa-rs 的参数顺序为 (sample_rate, samples)，且直接返回结果（非 Result）。
        let result = self.recognizer.transcribe(sample_rate, samples);
        Ok(result.text)
    }
}
