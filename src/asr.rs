//! SenseVoice 离线识别封装。
//!
//! 基于 sherpa-rs 0.6.8 的 `sherpa_rs::sense_voice::SenseVoiceRecognizer`。
//! 模型文件期望位于 `{MODEL_DIR}/model.onnx` 与 `{MODEL_DIR}/tokens.txt`。

use anyhow::{anyhow, Context, Result};
use sherpa_rs::sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer};

/// SenseVoice 模型所在目录。
pub const MODEL_DIR: &str = "./models/sensevoice";

/// 离线语音识别器。封装一个已加载的 SenseVoice 识别器。
pub struct Transcriber {
    recognizer: SenseVoiceRecognizer,
}

impl Transcriber {
    /// 加载 SenseVoice 离线识别器。
    ///
    /// 期望模型文件位于：
    /// - `{MODEL_DIR}/model.onnx`
    /// - `{MODEL_DIR}/tokens.txt`
    ///
    /// 注意：模型文件需在运行时存在，否则底层创建识别器会失败。
    pub fn load() -> Result<Transcriber> {
        let model = format!("{MODEL_DIR}/model.onnx");
        let tokens = format!("{MODEL_DIR}/tokens.txt");

        let config = SenseVoiceConfig {
            model,
            tokens,
            // "auto" 让模型自动检测语言（中英文等）。
            language: "auto".into(),
            // 启用逆文本归一化（数字、标点等）。
            use_itn: true,
            ..Default::default()
        };

        // sherpa-rs 使用 eyre::Result；转换为 anyhow。
        let recognizer = SenseVoiceRecognizer::new(config)
            .map_err(|e| anyhow!("创建 SenseVoice 识别器失败: {e}"))
            .context("加载 SenseVoice 模型失败，请确认模型文件存在")?;

        Ok(Transcriber { recognizer })
    }

    /// 对给定的单声道 f32 采样（采样率 `sample_rate`）执行离线识别，返回识别文本。
    pub fn transcribe(&mut self, samples: &[f32], sample_rate: u32) -> Result<String> {
        // 注意 sherpa-rs 的参数顺序为 (sample_rate, samples)，且直接返回结果（非 Result）。
        let result = self.recognizer.transcribe(sample_rate, samples);
        Ok(result.text)
    }
}
