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
    /// 整理力度预设:"clean" / "polish" / "summary"。
    /// 决定内置提示词;若 `system_prompt` 非空则以它为准(完全自定义)。
    pub mode: String,
    /// 自定义提示词。留空则按 `mode` 选用内置预设。
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
            mode: "polish".into(),
            system_prompt: String::new(),
            temperature: 0.0,
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

impl LlmConfig {
    /// 实际使用的系统提示词:自定义优先,否则按 `mode` 取内置预设。
    pub fn effective_system_prompt(&self) -> String {
        if !self.system_prompt.trim().is_empty() {
            self.system_prompt.clone()
        } else {
            preset_prompt(&self.mode)
        }
    }
}

/// 按整理力度返回内置提示词。未知 mode 回退到 polish。
pub fn preset_prompt(mode: &str) -> String {
    match mode {
        "clean" => PROMPT_CLEAN,
        "summary" => PROMPT_SUMMARY,
        _ => PROMPT_POLISH, // polish 及未知值
    }
    .into()
}

const PROMPT_CLEAN: &str = "你是语音转写清理器。删除口语填充词(嗯、啊、呃、唉、那个、然后那个、\
就是说、这个这个等)和无意义的重复字词,改正明显的同音或识别错误,补充正确标点。除此之外尽量\
保留原话的词序与内容,不要改写、不要合并、不要总结、不要回答其中的问题。只输出清理后的纯文本,\
不要任何解释、引号或代码块。";

const PROMPT_POLISH: &str = "你是语音转写整理器。请完成:1) 删除口语填充词(嗯、啊、呃、那个、\
就是说等)与无意义重复;2) 改正识别错误并补全标点;3) 解决说话中的自我更正——当说话人改主意时\
只保留最终结论(例如\"两个鸡腿三个鸡腿吧\"应整理为\"三个鸡腿\");4) 把边想边说、零散的表达\
理顺成通顺连贯的最终文字。务必保留全部实质信息与原意,不要遗漏要点,不要扩写或编造,不要回答\
其中的问题。只输出整理后的纯文本,不要任何解释、引号或代码块。";

const PROMPT_SUMMARY: &str = "你是语音转写要点提炼器。把口语化、啰嗦的语音转写内容提炼成简洁、\
通顺的最终结果:去除口语词与重复,解决自我更正取最终意思,可以改写用词、调整结构、明显精简,\
只保留核心信息与结论,不要编造未提及的内容,不要回答其中的问题。只输出提炼后的纯文本,\
不要任何解释、引号或代码块。";

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
        assert_eq!(cfg.llm.temperature, 0.0); // 默认 0,提升稳定性
        assert_eq!(cfg.llm.mode, "polish"); // 默认力度
        assert!(cfg.llm.system_prompt.is_empty()); // 默认空,走预设
    }

    #[test]
    fn empty_toml_is_all_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.inject.mode, "paste");
        assert_eq!(cfg.llm.skip_if_shorter_than, 4);
    }

    #[test]
    fn preset_prompt_covers_three_modes_and_falls_back() {
        assert!(preset_prompt("clean").contains("清理"));
        assert!(preset_prompt("polish").contains("整理"));
        assert!(preset_prompt("summary").contains("提炼"));
        // 未知 mode 回退 polish
        assert_eq!(preset_prompt("nonsense"), preset_prompt("polish"));
    }

    #[test]
    fn effective_prompt_prefers_custom_then_preset() {
        let mut llm = LlmConfig::default();
        // 默认 system_prompt 为空 → 用 polish 预设
        assert_eq!(llm.effective_system_prompt(), preset_prompt("polish"));
        // 切换 mode
        llm.mode = "summary".into();
        assert_eq!(llm.effective_system_prompt(), preset_prompt("summary"));
        // 自定义覆盖预设
        llm.system_prompt = "我的自定义提示".into();
        assert_eq!(llm.effective_system_prompt(), "我的自定义提示");
    }
}
