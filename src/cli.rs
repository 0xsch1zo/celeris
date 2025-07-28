use crate::session_manager::{ListSessionsOptions as MgrListSessionsOptions, SwitchTarget};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "A powerful git-aware session-manager written in Rust")]
#[command(long_about = None)]
#[command(version = "v0.1.0")]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Find repos on search roots declared in the config
    FindRepos,
    /// List configured sessions
    ListSessions {
        #[command(flatten)]
        opts: ListSessionsOptions,
    },
    /// Create a session config and open in $EDITOR
    NewSession {
        /// Root path of a session. By default the name is deduced automatically
        path: PathBuf,
        /// Set custom name for a session
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Edit an existing session config
    EditSession {
        /// Name of the session to be edited
        name: String,
    },
    /// Switch to a running session if exists or load it from the config
    Switch {
        #[command(flatten)]
        target: CliSwitchTarget,
    },
    /// Remove a session configuration
    RemoveSession {
        /// Name of the session to be removed
        name: String,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct CliSwitchTarget {
    /// Switch to last session. Name mustn't be supplied when this flag is passed
    #[arg(short, long)]
    last_session: bool,
    /// Name of the session to switch into
    name: Option<String>,
}

impl Into<SwitchTarget> for CliSwitchTarget {
    fn into(self) -> SwitchTarget {
        match self.last_session {
            true => SwitchTarget::LastSession,
            false => SwitchTarget::Session(self.name.unwrap()),
        }
    }
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct ListSessionsOptions {
    /// Include currently active tmux session in the listing(if exists). Signified with an asterisk
    /// at the end
    /// before the name
    #[arg(short, long)]
    include_active: bool,
    /// Exclude currently running tmux sessions. Sessions will always be loaded from the
    /// config.
    #[arg(short, long)]
    exclude_running: bool,

    #[arg(short, long)]
    only_running: bool,
}

impl Into<MgrListSessionsOptions> for ListSessionsOptions {
    fn into(self) -> MgrListSessionsOptions {
        MgrListSessionsOptions {
            include_active: self.include_active,
            exclude_running: self.exclude_running,
            only_running: self.only_running,
        }
    }
}
