//! TOML 配置。所有字段都有默认值,缺字段不报错,便于随时增减配置项。

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 触发热键配置。
    pub hotkey: HotkeyConfig,
    pub asr: AsrConfig,
    pub llm: LlmConfig,
    pub inject: InjectConfig,
    #[serde(default)]
    pub app_style: Vec<AppStyle>,
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
    /// 翻译模式(Win+Alt)提示词。留空则用内置默认(去语气词+译成英文)。
    pub translate_prompt: String,
    pub temperature: f32,
    pub timeout_secs: u64,
    /// 文本字符数小于该值时跳过 LLM(短词没必要纠错)。
    pub skip_if_shorter_than: usize,
    /// 专有名词;非空时注入提示词,优先按此拼写。
    pub vocabulary: Vec<String>,
    /// 命令模式提示词;留空用内置默认。
    pub command_prompt: String,
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
            hotkey: HotkeyConfig::default(),
            asr: AsrConfig::default(),
            llm: LlmConfig::default(),
            inject: InjectConfig::default(),
            app_style: Vec::new(),
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
            translate_prompt: String::new(),
            temperature: 0.0,
            timeout_secs: 10,
            skip_if_shorter_than: 4,
            vocabulary: Vec::new(),
            command_prompt: String::new(),
        }
    }
}

impl Default for InjectConfig {
    fn default() -> Self {
        InjectConfig { mode: "paste".into() }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub primary: String,
    pub translate_modifier: String,
    pub command_modifier: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            primary: "LWin".into(),
            translate_modifier: "LAlt".into(),
            command_modifier: "LCtrl".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppStyle {
    /// 前台进程名包含此串(不区分大小写)即命中。
    #[serde(rename = "match")]
    pub match_: String,
    /// 命中后追加到提示词的风格指令。
    pub style: String,
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

    /// 实际使用的翻译提示词:自定义优先,否则用内置默认。
    pub fn effective_translate_prompt(&self) -> String {
        if !self.translate_prompt.trim().is_empty() {
            self.translate_prompt.clone()
        } else {
            PROMPT_TRANSLATE.into()
        }
    }

    /// 词库提示行;词库为空返回 None。
    pub fn vocabulary_line(&self) -> Option<String> {
        let terms: Vec<&str> = self
            .vocabulary
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if terms.is_empty() {
            None
        } else {
            Some(format!("以下专有名词若出现请按此拼写:{}。", terms.join("、")))
        }
    }

    /// 命令模式实际提示词:自定义优先,否则内置默认。
    pub fn effective_command_prompt(&self) -> String {
        if !self.command_prompt.trim().is_empty() {
            self.command_prompt.clone()
        } else {
            PROMPT_COMMAND.into()
        }
    }
}

const PROMPT_COMMAND: &str = "你是文本编辑器。用户选中了一段文本,并口述了一条修改指令。\
请把指令应用到这段文本,只输出修改后的文本本身,不要解释、不要引号或代码块。\
若指令要求翻译,按要求翻译;否则保持与原文一致的语言。";

/// 翻译模式内置提示词:去语气词、纠错;非英文译成英文,本就是英文则润色纠语法;只输出英文。
const PROMPT_TRANSLATE: &str = "You are a speech-transcription post-processor and translator. \
The input is a voice transcription that may be Chinese, English, or a mix. First remove filler \
words and false starts, and fix obvious recognition errors. Then: if the content is not English, \
translate its meaning into natural, fluent English; if it is already English, just polish it and \
fix any grammar mistakes. Preserve the original intent; do not add or invent content; do not answer \
any question in it. Output ONLY the final English text — no explanations, no quotes, no code blocks, \
no Chinese.";

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
    /// 按前台进程名匹配应用风格,返回首条命中的 style。
    pub fn style_for(&self, process_name: &str) -> Option<String> {
        let pname = process_name.to_ascii_lowercase();
        self.app_style
            .iter()
            .find(|a| !a.match_.trim().is_empty() && pname.contains(&a.match_.to_ascii_lowercase()))
            .map(|a| a.style.clone())
    }

    /// 从 TOML 文件读取配置。
    pub fn load(path: &str) -> anyhow::Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {path}(可复制 config.example.toml 为 config.toml)"))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("解析配置文件失败: {path}"))?;
        Ok(cfg)
    }

    /// 不依赖工作目录地加载:按 CWD → 程序目录 → 逐级父目录 查找 config.toml,
    /// 并把相对的 `asr.model_dir` 解析到 config 所在目录,得到绝对路径。
    pub fn load_resolved() -> anyhow::Result<Config> {
        let (path, base) = find_config_file()?;
        let mut cfg = Config::load(&path.to_string_lossy())?;
        cfg.asr.model_dir = resolve_model_dir(&base, &cfg.asr.model_dir);
        Ok(cfg)
    }
}

/// 把相对的 model_dir 解析到 base 目录;绝对路径原样返回。
pub fn resolve_model_dir(base: &Path, model_dir: &str) -> String {
    let md = Path::new(model_dir);
    if md.is_relative() {
        base.join(md).to_string_lossy().to_string()
    } else {
        model_dir.to_string()
    }
}

/// 查找 config.toml,返回 (文件路径, 所在目录)。
/// 顺序:当前工作目录 → 可执行文件目录 → 其各级父目录。
fn find_config_file() -> anyhow::Result<(PathBuf, PathBuf)> {
    if let Ok(cwd) = std::env::current_dir() {
        let c = cwd.join("config.toml");
        if c.is_file() {
            return Ok((c, cwd));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent();
        while let Some(d) = dir {
            let c = d.join("config.toml");
            if c.is_file() {
                return Ok((c, d.to_path_buf()));
            }
            dir = d.parent();
        }
    }
    anyhow::bail!("找不到 config.toml(已查找工作目录与程序所在目录及其父目录)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_model_dir_makes_relative_absolute() {
        let base = Path::new("C:\\base\\dir");
        let r = resolve_model_dir(base, "./models/sensevoice");
        assert!(r.contains("base"));
        assert!(r.contains("models"));
        assert!(Path::new(&r).is_absolute());
        // 绝对路径原样返回
        assert_eq!(
            resolve_model_dir(base, "C:\\abs\\models"),
            "C:\\abs\\models"
        );
    }

    #[test]
    fn parses_full_config() {
        let toml = r#"
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
        assert_eq!(cfg.hotkey.primary, "LWin");
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

    #[test]
    fn hotkey_table_parses_with_defaults() {
        let cfg: Config = toml::from_str(
            r#"
[hotkey]
command_modifier = "RCtrl"
"#,
        )
        .unwrap();
        assert_eq!(cfg.hotkey.primary, "LWin");
        assert_eq!(cfg.hotkey.translate_modifier, "LAlt");
        assert_eq!(cfg.hotkey.command_modifier, "RCtrl");
    }

    #[test]
    fn vocabulary_line_joins_or_none() {
        let mut llm = LlmConfig::default();
        assert!(llm.vocabulary_line().is_none());
        llm.vocabulary = vec!["Kubernetes".into(), " ".into(), "OneDrive".into()];
        assert_eq!(
            llm.vocabulary_line().unwrap(),
            "以下专有名词若出现请按此拼写:Kubernetes、OneDrive。"
        );
    }

    #[test]
    fn style_for_matches_first_by_substring_ci() {
        let cfg: Config = toml::from_str(
            r#"
[[app_style]]
match = "OUTLOOK"
style = "正式"
[[app_style]]
match = "code"
style = "技术"
"#,
        )
        .unwrap();
        assert_eq!(cfg.style_for("OUTLOOK.EXE").as_deref(), Some("正式"));
        assert_eq!(cfg.style_for("Code.exe").as_deref(), Some("技术"));
        assert_eq!(cfg.style_for("notepad.exe"), None);
    }

    #[test]
    fn effective_command_prompt_default_then_custom() {
        let mut llm = LlmConfig::default();
        assert!(llm.effective_command_prompt().contains("文本编辑器"));
        llm.command_prompt = "自定义命令".into();
        assert_eq!(llm.effective_command_prompt(), "自定义命令");
    }
}
