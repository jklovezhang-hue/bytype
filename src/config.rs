//! TOML 配置。所有字段都有默认值,缺字段不报错,便于随时增减配置项。

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// 触发热键配置。
    pub hotkey: HotkeyConfig,
    pub asr: AsrConfig,
    pub llm: LlmConfig,
    pub inject: InjectConfig,
    #[serde(default)]
    pub app_style: Vec<AppStyle>,
    pub overlay: OverlayConfig,
    pub sound: SoundConfig,
    pub model: ModelConfig,
    pub meeting: MeetingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AsrConfig {
    pub model_dir: String,
    pub language: String,
}

// PartialEq 仅用于测试断言;temperature 为 f32,NaN 不会出现(值只来自 TOML 解析或默认 0.0)。
// 设置 UI 的脏检查在前端用 JSON 快照对比,不依赖此处。
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
    /// 翻译模式(Win+Alt)提示词。留空则用内置默认(去语气词+中英互译:中→英,其他→中)。
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
            overlay: OverlayConfig::default(),
            sound: SoundConfig::default(),
            model: ModelConfig::default(),
            meeting: MeetingConfig::default(),
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OverlayConfig {
    /// 是否显示录音浮窗。false 则完全不弹(引擎逻辑不受影响)。
    pub enabled: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct SoundConfig {
    /// 是否播放录音开始/结束提示音。
    pub enabled: bool,
    /// 自定义开始音 wav 路径;留空用内置默认。
    pub start_sound: String,
    /// 自定义结束音 wav 路径;留空用内置默认。
    pub end_sound: String,
}

impl Default for SoundConfig {
    fn default() -> Self {
        SoundConfig {
            enabled: true,
            start_sound: String::new(),
            end_sound: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ModelConfig {
    /// 语音识别模型(int8 onnx)下载源;下载后存为 model.onnx。
    pub model_url: String,
    /// tokens.txt 下载源。
    pub tokens_url: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            model_url: "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx".into(),
            tokens_url: "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt".into(),
        }
    }
}

/// 会议录音模式。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    /// 麦克风 + 系统声音。
    MicSystem,
    /// 只录系统声音。
    System,
    /// 只录麦克风。
    Mic,
}

/// 音频保留档。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioRetention {
    /// 都删,只剩转写+纪要。
    None,
    /// 只留 <base>.mp3 存档(默认)。
    Mixed,
    /// 留 mp3 + 双轨 WAV。
    Tracks,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct MeetingConfig {
    /// 会议文件夹根目录(相对路径按 resolve 规则解析)。
    pub output_dir: String,
    /// 开始弹窗预选的默认模式。
    pub default_mode: RecordMode,
    /// 是否分说话人(有系统轨时生效;M1 不实现,仅存配置)。
    pub diarization: bool,
    /// 音频保留档。
    pub audio_retention: AudioRetention,
    /// 自动删几天前音频(0=永久;M1 仅存配置)。
    pub audio_retention_days: u32,
    /// 存档 MP3 比特率(kbps,单声道)。
    pub archive_bitrate: u32,
    /// Silero VAD 模型路径(相对路径按 resolve 规则解析)。
    pub vad_model: String,
    /// 自定义纪要提示词;留空则用内置默认。
    pub minutes_prompt: String,
    /// 是否对会议转写逐段做 LLM 清理(去语气词/纠错/标点);需 LLM 启用。
    pub clean_transcript: bool,
    /// 说话人分段模型(pyannote)路径。
    pub segmentation_model: String,
    /// 声纹嵌入模型路径。
    pub embedding_model: String,
    /// 期望说话人数;0/负=自动(按阈值聚类)。
    pub diarization_speakers: i32,
}

impl Default for MeetingConfig {
    fn default() -> Self {
        MeetingConfig {
            output_dir: "./meetings".into(),
            default_mode: RecordMode::MicSystem,
            diarization: true,
            audio_retention: AudioRetention::Mixed,
            audio_retention_days: 7,
            archive_bitrate: 48,
            vad_model: "./models/silero_vad.onnx".into(),
            minutes_prompt: String::new(),
            clean_transcript: true,
            segmentation_model: "./models/segmentation.onnx".into(),
            embedding_model: "./models/speaker_embedding.onnx".into(),
            diarization_speakers: 0,
        }
    }
}

impl MeetingConfig {
    /// 实际纪要提示词:自定义优先,否则内置默认。
    pub fn effective_minutes_prompt(&self) -> String {
        if self.minutes_prompt.trim().is_empty() {
            PROMPT_MINUTES.to_string()
        } else {
            self.minutes_prompt.clone()
        }
    }
}

/// 内置会议纪要提示词。
const PROMPT_MINUTES: &str = "你是会议纪要助理。下面是一段带时间戳与说话人(我/对方)的会议转写。\
请整理成结构化的中文会议纪要,包含:1) 会议主题(若能判断);2) 关键讨论点;3) 决议/结论;\
4) 待办事项(含负责人与时限,若提及)。忠实原意,不要编造未提及的内容;条理清晰,用 Markdown\
(二级标题与列表)。只输出纪要正文,不要复述原始转写。";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
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

/// 翻译模式内置提示词:去语气词、纠错;中文译成英文,其他语言(含英文)译成中文;翻译时顺带纠正
/// 原文的语法用词错误;只输出译文。
const PROMPT_TRANSLATE: &str = "You are a speech-transcription post-processor and translator. \
The input is a voice transcription that may be Chinese, English, or another language. First remove \
filler words and false starts, resolve self-corrections (keep only the final statement), and fix \
obvious recognition errors. Then translate: if the content is mainly Chinese, translate it into \
natural, fluent, grammatically correct English; otherwise (English or any other language), translate \
it into natural, fluent Chinese. Fix grammar and wording mistakes from the source instead of copying \
them. Preserve the original intent; do not add or invent content; do not answer any question in it. \
Output ONLY the final translated text — no explanations, no quotes, no code blocks.";

/// 按整理力度返回内置提示词。未知 mode 回退到 polish。
pub fn preset_prompt(mode: &str) -> String {
    match mode {
        "clean" => PROMPT_CLEAN,
        "summary" => PROMPT_SUMMARY,
        _ => PROMPT_POLISH, // polish 及未知值
    }
    .into()
}

const PROMPT_CLEAN: &str = "你是语音转写清理器。删除口语填充词与应答词(中文:嗯、啊、呃、唉、那个、\
然后那个、就是说、这个这个等;英文:uh、um、er、yeah、yep、ok、okay、mm、mhm 等)和无意义的重复字词,\
改正明显的同音或识别错误,补充正确标点。除此之外尽量保留原话的词序与内容,不要改写、不要合并、\
不要总结、不要回答其中的问题。若整段只剩填充/应答词而无实质内容,则输出空。\
只输出清理后的纯文本,不要任何解释、引号或代码块。";

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

    /// 原样加载(路径字段不做相对→绝对解析),返回配置与 config.toml 路径。
    /// 设置界面用它,保证 "./models/sensevoice" 这类相对路径原样写回。
    /// (find_config_file 的第二个返回值是所在目录,这里不解析路径故不需要。)
    pub fn load_raw() -> anyhow::Result<(Config, PathBuf)> {
        let (path, _base) = find_config_file()?;
        let cfg = Config::load(&path.to_string_lossy())?;
        Ok((cfg, path))
    }

    /// 序列化为 TOML 并整文件写回(手写注释会丢失,字段值全部保留)。
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let text = toml::to_string_pretty(self).context("序列化配置失败")?;
        std::fs::write(path, text)
            .with_context(|| format!("写入配置文件失败: {}", path.display()))?;
        Ok(())
    }

    /// 不依赖工作目录地加载:查找 config.toml,并把相对的 `asr.model_dir`、
    /// 提示音路径、`meeting.vad_model` 解析到 config 所在目录,得到绝对路径。
    pub fn load_resolved() -> anyhow::Result<Config> {
        let (mut cfg, path) = Config::load_raw()?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        cfg.asr.model_dir = resolve_model_dir(&base, &cfg.asr.model_dir);
        cfg.sound.start_sound = resolve_sound_path(&base, &cfg.sound.start_sound);
        cfg.sound.end_sound = resolve_sound_path(&base, &cfg.sound.end_sound);
        cfg.meeting.vad_model = resolve_model_dir(&base, &cfg.meeting.vad_model);
        cfg.meeting.output_dir = resolve_model_dir(&base, &cfg.meeting.output_dir);
        cfg.meeting.segmentation_model = resolve_model_dir(&base, &cfg.meeting.segmentation_model);
        cfg.meeting.embedding_model = resolve_model_dir(&base, &cfg.meeting.embedding_model);
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

/// 解析提示音路径:空字符串保持空(用内置默认);非空相对 base 解析为绝对。
pub fn resolve_sound_path(base: &Path, p: &str) -> String {
    if p.trim().is_empty() {
        String::new()
    } else {
        resolve_model_dir(base, p)
    }
}

/// 查找 config.toml,返回 (文件路径, 所在目录)。
/// 顺序:当前工作目录 → 可执行文件目录 → 其各级父目录。
pub fn find_config_file() -> anyhow::Result<(PathBuf, PathBuf)> {
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

    #[test]
    fn overlay_defaults_enabled_true() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.overlay.enabled);
    }

    #[test]
    fn overlay_can_be_disabled() {
        let cfg: Config = toml::from_str("[overlay]\nenabled = false\n").unwrap();
        assert!(!cfg.overlay.enabled);
    }

    #[test]
    fn sound_defaults_enabled_paths_empty() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.sound.enabled);
        assert!(cfg.sound.start_sound.is_empty());
        assert!(cfg.sound.end_sound.is_empty());
    }

    #[test]
    fn sound_can_be_disabled_and_pathed() {
        let cfg: Config =
            toml::from_str("[sound]\nenabled = false\nstart_sound = \"a.wav\"\n").unwrap();
        assert!(!cfg.sound.enabled);
        assert_eq!(cfg.sound.start_sound, "a.wav");
    }

    #[test]
    fn resolve_sound_path_empty_stays_empty_else_absolute() {
        let base = Path::new("C:\\base");
        assert_eq!(resolve_sound_path(base, ""), "");
        assert_eq!(resolve_sound_path(base, "   "), "");
        let r = resolve_sound_path(base, "snd\\a.wav");
        assert!(Path::new(&r).is_absolute());
        assert!(r.contains("base"));
        // 绝对路径原样返回
        assert_eq!(resolve_sound_path(base, "C:\\x\\a.wav"), "C:\\x\\a.wav");
    }

    #[test]
    fn serialize_roundtrip_preserves_values() {
        let mut cfg = Config::default();
        cfg.hotkey.primary = "RWin".into();
        cfg.asr.model_dir = "./models/sensevoice".into();
        cfg.llm.api_key = "sk-test".into();
        cfg.llm.vocabulary = vec!["Kubernetes".into(), "ByType".into()];
        cfg.app_style = vec![AppStyle { match_: "outlook".into(), style: "正式".into() }];
        cfg.sound.enabled = false;
        let text = toml::to_string_pretty(&cfg).unwrap();
        // rename 生效:写出的是 match 而不是 match_
        assert!(text.contains("match = \"outlook\""), "got: {text}");
        // 相对路径原样保留
        assert!(text.contains("./models/sensevoice"));
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn save_to_then_load_roundtrips() {
        let mut cfg = Config::default();
        cfg.llm.model = "deepseek-v4-flash".into();
        cfg.llm.vocabulary = vec!["OneDrive".into()];
        let dir = std::env::temp_dir().join(format!("bytype-g4-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        cfg.save_to(&path).unwrap();
        let back = Config::load(&path.to_string_lossy()).unwrap();
        assert_eq!(back, cfg);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn model_section_defaults_and_override() {
        // 默认:URL 指向 hf-mirror
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.model.model_url.contains("hf-mirror.com"));
        assert!(cfg.model.tokens_url.contains("tokens.txt"));
        // 覆盖
        let cfg: Config =
            toml::from_str("[model]\nmodel_url = \"https://x/m.onnx\"\n").unwrap();
        assert_eq!(cfg.model.model_url, "https://x/m.onnx");
        // 往返
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn meeting_config_has_vad_model_default() {
        let m = MeetingConfig::default();
        assert_eq!(m.vad_model, "./models/silero_vad.onnx");
    }

    #[test]
    fn meeting_minutes_prompt_defaults_to_builtin() {
        let m = MeetingConfig::default();
        assert_eq!(m.minutes_prompt, "");
        assert!(m.effective_minutes_prompt().contains("会议纪要"));
    }

    #[test]
    fn meeting_effective_minutes_prompt_prefers_custom() {
        let mut m = MeetingConfig::default();
        m.minutes_prompt = "自定义纪要提示".into();
        assert_eq!(m.effective_minutes_prompt(), "自定义纪要提示");
    }

    #[test]
    fn meeting_config_defaults() {
        let m = MeetingConfig::default();
        assert_eq!(m.output_dir, "./meetings");
        assert_eq!(m.default_mode, RecordMode::MicSystem);
        assert!(m.diarization);
        assert_eq!(m.audio_retention, AudioRetention::Mixed);
        assert_eq!(m.audio_retention_days, 7);
        assert_eq!(m.archive_bitrate, 48);
    }

    #[test]
    fn meeting_config_partial_toml_uses_defaults() {
        let cfg: Config = toml::from_str("[meeting]\naudio_retention = \"none\"\n").unwrap();
        assert_eq!(cfg.meeting.audio_retention, AudioRetention::None);
        assert_eq!(cfg.meeting.default_mode, RecordMode::MicSystem);
    }

    #[test]
    fn meeting_clean_transcript_defaults_true() {
        assert!(MeetingConfig::default().clean_transcript);
    }

    #[test]
    fn meeting_diarization_model_defaults() {
        let m = MeetingConfig::default();
        assert_eq!(m.segmentation_model, "./models/segmentation.onnx");
        assert_eq!(m.embedding_model, "./models/speaker_embedding.onnx");
        assert_eq!(m.diarization_speakers, 0);
    }

    #[test]
    fn record_mode_serde_roundtrip() {
        for (m, s) in [
            (RecordMode::MicSystem, "mic_system"),
            (RecordMode::System, "system"),
            (RecordMode::Mic, "mic"),
        ] {
            let toml_s = toml::to_string(&MeetingConfig { default_mode: m, ..Default::default() }).unwrap();
            assert!(toml_s.contains(s), "{toml_s} should contain {s}");
        }
    }
}
