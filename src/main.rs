use color_eyre::Result;
use sesh::config::Config;
use sesh::manifest::Manifest;
use sesh::tui::picker;
use sesh::tui::repo_search::RepoModel;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    let mut manifest = Manifest::new()?;
    //manifest.update_diff(&repos)?;
    //manifest.serialize()?;

    picker(RepoModel::new(config))?;

    Ok(())
}
