//! 离线验证 LLM 纠错链路:读取 config.toml,把一段(默认带口语词的)文本送去纠错。
//!
//! 用法:cargo run --example correct_text -- "要整理的文本" [mode]
//!   - 第二个参数可选,覆盖配置里的 mode(clean / polish / summary)。
//! 不带参数则用内置示例文本与配置里的 mode。

use voice_input::config::Config;
use voice_input::corrector::Corrector;

fn main() -> anyhow::Result<()> {
    let raw = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "嗯……那个今天天气不错呃 hello world 就是说挺好的".to_string());

    let mut config = Config::load("config.toml")?;
    if let Some(mode) = std::env::args().nth(2) {
        config.llm.mode = mode;
    }
    println!(
        "模型: {}  mode: {}  temp: {}",
        config.llm.model, config.llm.mode, config.llm.temperature
    );
    println!("原始: {raw}");

    let corrector = Corrector::new(config.llm)?;
    let fixed = corrector.correct(&raw);
    println!("修整: {fixed}");
    Ok(())
}
