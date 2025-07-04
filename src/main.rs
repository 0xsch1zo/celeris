use clap::{Parser, Subcommand};
use color_eyre::eyre::OptionExt;
use color_eyre::{Result, eyre::Context};
use sesh::config::Config;
use sesh::manifest::Manifest;
use sesh::repos::search::search;
use sesh::{script, session_manager};
use std::sync::mpsc;
use std::thread;

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
    NewSession { session_name: String },
    LoadSession { session_name: String },
}

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    let config = Config::new()?;
    let mut manifest = Manifest::new()?;

    let cli = Cli::parse();

    match &cli.command {
        Commands::PickRepo => {
            let name_filter =
                session_manager::NameFilter::spawn(&config).wrap_err("failed to spawn filter")?;
            let repos = search(&config)?;
            let names = repos
                .iter()
                .filter(|r| !manifest.contains(&r.name))
                .map(|r| r.name.clone())
                .collect::<Vec<_>>();
            let picked_name = name_filter.filter(&names)?;
            let repo = repos
                .into_iter()
                .find(|r| r.name == picked_name)
                .ok_or_eyre(format!("repository not found: {picked_name}"))?;
            session_manager::create(&mut manifest, &config, repo.into())?;
        }
        Commands::ListSessions => {}
        Commands::EditSession { session_name } => {
            let entry = manifest.entry(session_name).wrap_err("session not found")?;
            script::edit(&script::path(&entry.hash)?, &config)?;
        }
        Commands::NewSession { session_name } => {}
        Commands::LoadSession { session_name } => {
            let entry = manifest.entry(session_name).wrap_err("session not found")?;
            script::run(&script::path(&entry.hash)?, session_name.to_string())?;
        }
    }
    //picker(RepoModel::new(config))?;
    /*let session = Session::new(String::from("test"))?;
    let nvim = session.new_window(Some("neovim"), None)?;
    nvim.default_pane().run_command("nvim")?;

    let build = session.new_window(Some("build"), None)?;
    build.default_pane().run_command("echo hello")?;

    let tests = build.default_pane().split(Direction::Horizontal)?;
    tests.run_command("cargo test")?;*/
    //window.even_out(Direction::Vertical)?;
    //session.windows()[0].panes()[0].run_command("echo deeez nuts");
    //t.split_window("3", Split::Vertical)?;
    //t.run_command("3", "1", "ls")?;
    //t.even_out("3", Direction::Horizontal)?;

    //let script = fs::read_to_string("test.rhai")?;
    //session_config::run_script(&script, String::from("1"))?;
    Ok(())
}
