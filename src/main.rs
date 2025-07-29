use clap::Parser;
use color_eyre::Result;
use sesh::cli::{Cli, Commands};
use sesh::config::Config;
use sesh::repo_search;
use sesh::session_manager::{SessionManager, SessionProperties};
use std::io::{self, Write};

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;

    let cli = Cli::parse();
    let config = Config::new(cli.config)?;
    let mut session_manager = SessionManager::new(&config)?;

    match cli.command {
        Commands::FindRepos => {
            io::stdout().write_all(repo_search::search(&config)?.join("\n").as_bytes())?
        }
        Commands::ListSessions { opts } => session_manager.list(opts.into())?,
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
