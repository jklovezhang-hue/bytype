//! 离线验证命令模式:读 config.toml,把"指令"应用到"选中文本"。
//!
//! 用法:cargo run --example command_text -- "<指令>" "<选中文本>"

use voice_input::config::Config;
use voice_input::corrector::Corrector;

fn main() -> anyhow::Result<()> {
    let instruction = std::env::args().nth(1).unwrap_or_else(|| "改成正式语气".into());
    let selected = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "嘿哥们这事儿稳了你就瞧好吧".into());

    let config = Config::load("config.toml")?;
    println!("指令: {instruction}");
    println!("原文: {selected}");

    let corrector = Corrector::new(config.llm)?;
    let result = corrector.command(&instruction, &selected);
    println!("结果: {result}");
    Ok(())
}
