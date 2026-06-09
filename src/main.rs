use voice_input::config::Config;

fn main() -> anyhow::Result<()> {
    let config = Config::load_resolved()?;
    voice_input::engine::run(config)
}
