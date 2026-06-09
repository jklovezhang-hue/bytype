use voice_input::config::Config;

fn main() -> anyhow::Result<()> {
    let config = Config::load("config.toml")?;
    voice_input::engine::run(config)
}
