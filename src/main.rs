mod cli;
use celeris::{Config, DirectoryManager, SessionManager};
use clap::Parser;
use cli::{Cli, Commands};
use color_eyre::Result;
use color_eyre::eyre::Context;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let cli = Cli::parse();
    let mut dir_mgr_builder = DirectoryManager::builder();
    if let Some(config_dir) = cli.config_dir {
        dir_mgr_builder.config_dir(config_dir)?;
    }

    if let Some(cache_dir) = cli.cache_dir {
        dir_mgr_builder.cache_dir(cache_dir)?;
    }
    let dir_mgr = dir_mgr_builder.build()?;

    let config = Arc::new(Config::new(&dir_mgr)?);
    let mut session_manager = SessionManager::new(Arc::clone(&config), Arc::new(dir_mgr))?;

    match cli.command {
        Commands::Edit { name } => session_manager.edit(&name)?,
        Commands::Switch { target } => session_manager.switch(target.into())?,
        Commands::Remove { names } => session_manager.remove(names)?,
        Commands::Create { opts } => session_manager.create(opts.into())?,
        Commands::CreateAll => {
            let paths = io::stdin()
                .lines()
                .map(|path| Ok(PathBuf::from(path?)))
                .collect::<Result<Vec<_>>>()?;
            session_manager.create_all(paths)?;
        }
        _ => {
            let output = match cli.command {
                Commands::Search => celeris::search(&config)?.join("\n"),
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
