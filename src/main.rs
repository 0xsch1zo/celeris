use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use sesh::config::Config;
use sesh::repo_search;
use sesh::session_manager::{SessionManager, SessionProperties};

#[derive(Parser)]
#[command(version, about, long_about = Some("testing"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    FindRepos,
    ListSessions,
    NewSession {
        path: PathBuf,
        #[arg(short, long)]
        name: Option<String>,
    },
    EditSession {
        name: String,
    },
    LoadSession {
        name: String,
    },
    RemoveSession {
        name: String,
    },
}

// TODO: somthing something last project feature add
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
        Commands::ListSessions => {}
        Commands::NewSession { name, path } => {
            let props = SessionProperties::from(name, path);
            session_manager.create(props)?;
        }
        Commands::EditSession { name } => session_manager.edit(&name)?,
        Commands::LoadSession { name } => session_manager.run(&name)?,
        Commands::RemoveSession { name } => session_manager.remove(&name)?,
    }
    Ok(())
}
