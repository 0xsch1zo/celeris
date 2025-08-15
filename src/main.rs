use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::Context;
use sesh::cli::{Cli, Commands};
use sesh::config::Config;
use sesh::directory_manager::DirectoryManager;
use sesh::repo_search;
use sesh::session_manager::SessionManager;
use std::io::{self, Write};
use std::sync::Arc;

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

    let config = Arc::new(Config::new(&dir_mgr)?);
    let mut session_manager = SessionManager::new(Arc::clone(&config), Arc::new(dir_mgr))?;

    match cli.command {
        Commands::Edit { name } => session_manager.edit(&name)?,
        Commands::Switch { target } => session_manager.switch(target.into())?,
        Commands::Remove { name } => session_manager.remove(&name)?,
        _ => {
            let output = match cli.command {
                Commands::Search => repo_search::search(&config)?.join("\n"),
                Commands::New { name, path } => session_manager.create(name, path)?,
                Commands::List { opts } => session_manager.list(opts.into())?,
                _ => unreachable!(),
            };

            io::stdout()
                .write_all(output.as_bytes())
                .wrap_err("failed to write result of subcommand to stdout")?
        }
    }
    Ok(())
}
