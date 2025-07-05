use clap::{Parser, Subcommand};
use color_eyre::eyre::OptionExt;
use color_eyre::{Result, eyre::Context};
use sesh::config::Config;
use sesh::repos::search::search;
use sesh::session_manager::{self, SessionManager};

#[derive(Parser)]
#[command(version, about, long_about = Some("testing"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    PickRepo,
    ListSessions,
    EditSession { session_name: String },
    LoadSession { session_name: String },
}

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    let mut session_manager = SessionManager::new(&config)?;
    let cli = Cli::parse();

    match &cli.command {
        Commands::PickRepo => {
            let name_filter =
                session_manager::NameFilter::spawn(&config).wrap_err("failed to spawn filter")?;
            let repos = search(&config)?;
            let names = repos
                .iter()
                .filter(|r| !session_manager.exists(&r.name))
                .map(|r| r.name.clone())
                .collect::<Vec<_>>();
            let picked_name = name_filter.filter(&names)?;
            let repo = repos
                .into_iter()
                .find(|r| r.name == picked_name)
                .ok_or_eyre(format!("repository not found: {picked_name}"))?;
            session_manager.create(repo.into())?;
        }
        Commands::ListSessions => {}
        Commands::EditSession { session_name } => session_manager.edit(session_name)?,
        Commands::LoadSession { session_name } => session_manager.run(session_name)?,
    }
    Ok(())
}
