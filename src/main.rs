use color_eyre::Result;
use sesh::config::Config;
use sesh::manifest::Manifest;
use sesh::repos::search::search;
use sesh::tui::picker;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    let mut manifest = Manifest::new()?;
    let repos = search(&config)?;
    //manifest.update_diff(&repos)?;
    //manifest.serialize()?;

    picker()?;

    Ok(())
}
