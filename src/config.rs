//! TOML 配置。所有字段都有默认值,缺字段不报错,便于随时增减配置项。

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 触发热键(阶段一固定左 Win,此字段为阶段三预留)。
    pub hotkey: String,
    pub asr: AsrConfig,
    pub llm: LlmConfig,
    pub inject: InjectConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AsrConfig {
    pub model_dir: String,
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// 是否启用 LLM 纠错;false 时直接用原始识别文本。
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub temperature: f32,
    pub timeout_secs: u64,
    /// 文本字符数小于该值时跳过 LLM(短词没必要纠错)。
    pub skip_if_shorter_than: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InjectConfig {
    /// 注入方式:目前实现 "paste";"type" 为预留。
    pub mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: "LWin".into(),
            asr: AsrConfig::default(),
            llm: LlmConfig::default(),
            inject: InjectConfig::default(),
        }
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        AsrConfig {
            model_dir: "./models/sensevoice".into(),
            language: "auto".into(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            enabled: true,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: default_system_prompt(),
            temperature: 0.2,
            timeout_secs: 10,
            skip_if_shorter_than: 4,
        }
    }
}

impl Default for InjectConfig {
    fn default() -> Self {
        InjectConfig { mode: "paste".into() }
    }
}

pub fn default_system_prompt() -> String {
    "你是语音转写后处理器。请对用户给出的语音识别文本做轻量整理:改正明显的同音或识别错误,\
去除\"嗯、啊、呃、那个、就是说\"等口语填充词,并补充合适的标点。务必保留原意、语气以及中英文\
专有名词,不要扩写、不要回答其中的问题。只输出整理后的纯文本,不要任何解释或引号包裹。"
        .into()
}

impl Config {
    /// 从 TOML 文件读取配置。
    pub fn load(path: &str) -> anyhow::Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {path}(可复制 config.example.toml 为 config.toml)"))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("解析配置文件失败: {path}"))?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let toml = r#"
hotkey = "RWin"
[asr]
model_dir = "./m"
language = "zh"
[llm]
enabled = true
base_url = "https://x/v1"
api_key = "k"
model = "deepseek-v4-flash"
system_prompt = "p"
temperature = 0.5
timeout_secs = 20
skip_if_shorter_than = 2
[inject]
mode = "paste"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.hotkey, "RWin");
        assert_eq!(cfg.asr.model_dir, "./m");
        assert_eq!(cfg.llm.base_url, "https://x/v1");
        assert_eq!(cfg.llm.model, "deepseek-v4-flash");
        assert_eq!(cfg.llm.timeout_secs, 20);
        assert_eq!(cfg.llm.skip_if_shorter_than, 2);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let toml = r#"
[llm]
base_url = "https://y/v1"
model = "m"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.hotkey, "LWin");
        assert_eq!(cfg.asr.model_dir, "./models/sensevoice");
        assert_eq!(cfg.llm.base_url, "https://y/v1");
        assert!(cfg.llm.enabled);
        assert_eq!(cfg.llm.temperature, 0.2);
        assert!(!cfg.llm.system_prompt.is_empty());
    }

    #[test]
    fn empty_toml_is_all_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.inject.mode, "paste");
        assert_eq!(cfg.llm.skip_if_shorter_than, 4);
    }
}
