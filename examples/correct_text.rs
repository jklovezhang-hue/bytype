//! 离线验证 LLM 纠错链路:读取 config.toml,把一段(默认带口语词的)文本送去纠错。
//!
//! 用法:cargo run --example correct_text -- "嗯……今天天气不错那个 hello world"
//! 不带参数则用内置示例文本。

use voice_input::config::Config;
use voice_input::corrector::Corrector;

fn main() -> anyhow::Result<()> {
    let raw = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "嗯……那个今天天气不错呃 hello world 就是说挺好的".to_string());

    let config = Config::load("config.toml")?;
    println!("模型: {}  base_url: {}", config.llm.model, config.llm.base_url);
    println!("原始: {raw}");

    let corrector = Corrector::new(config.llm)?;
    let fixed = corrector.correct(&raw);
    println!("修整: {fixed}");
    Ok(())
}
