use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use sesh::config::Config;
use sesh::repo_search;
use sesh::session_manager::{SessionManager, SessionProperties};

#[derive(Parser)]
#[command(about = "A powerful git-aware session-manager written in Rust")]
#[command(long_about = None)]
#[command(version = "v0.1.0")]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Finds repos on search roots declared in the config
    FindRepos,
    /// Lists configured sessions
    ListSessions {
        #[arg(short, long)]
        include_active: bool,
    },
    /// Creates a session config and opens it in your $EDITOR
    NewSession {
        /// Root path of a session. The name will be deduced unless set explictly
        // TODO: IMPORTANT Consider making this a named argument for clarity, it could be clear
        // enough now
        path: PathBuf,
        /// Custom name for a session
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Edits an existing session config
    EditSession { name: String },
    /// Loads a session config
    LoadSession { name: String },
    /// Removes a session configuration
    RemoveSession { name: String },
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
        Commands::ListSessions { include_active } => session_manager.list(include_active)?,
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
