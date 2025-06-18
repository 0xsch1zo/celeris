use color_eyre::Result;
use sesh::config::Config;
use sesh::search::search;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    let repos = search(&config)?;

    Ok(())
}
