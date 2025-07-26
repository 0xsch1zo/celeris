use clap::Parser;
use color_eyre::Result;
use sesh::cli::{Cli, Commands};
use sesh::config::Config;
use sesh::repo_search;
use sesh::session_manager::{SessionManager, SessionProperties};

// TODO: somthing something last project feature add
// TODO: add flag to exclude runnintg session from list-sessions
fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;

    let config = Config::new()?;
    let mut session_manager = SessionManager::new(&config)?;
    let cli = Cli::parse();

    match cli.command {
        Commands::FindRepos => {
            let repos = repo_search::search(&config)?;
            repos.iter().for_each(|r| println!("{r}"));
        }
        Commands::ListSessions { include_active } => session_manager.list(include_active)?,
        Commands::NewSession { name, path } => {
            let props = SessionProperties::from(name, path);
            session_manager.create(props)?;
        }
        Commands::EditSession { name } => session_manager.edit(&name)?,
        Commands::Switch { target } => session_manager.switch(target.into())?,
        Commands::RemoveSession { name } => session_manager.remove(&name)?,
    }
    Ok(())
}
