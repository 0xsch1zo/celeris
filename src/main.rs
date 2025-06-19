use color_eyre::Result;
use color_eyre::eyre::bail;
use sesh::config::Config;
use sesh::search::search;
use sesh::tui::picker;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    //let repos = search(&config)?;

    picker()?;

    Ok(())
}
