use clap::Parser;
use color_eyre::Result;
use sesh::cli::{Cli, Commands};
use sesh::config::Config;
use sesh::directory_manager::DirectoryManager;
use sesh::repo_search;
use sesh::session_manager::{SessionManager, SessionProperties};
use std::io::{self, Write};
use std::rc::Rc;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;

    let cli = Cli::parse();
    let mut dir_mgr = DirectoryManager::new();
    if let Some(config_dir) = cli.config_dir {
        dir_mgr.set_config_dir(config_dir)?;
    }

    if let Some(cache_dir) = cli.cache_dir {
        dir_mgr.set_cache_dir(cache_dir)?;
    }

    let config = Rc::new(Config::new(&dir_mgr)?);
    let mut session_manager = SessionManager::new(Rc::clone(&config), Rc::new(dir_mgr))?;

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
