use crate::session_manager::SwitchTarget;
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
    /// If a session is running switches to it, if not tries to load it from config
    Switch {
        #[command(flatten)]
        target: CliSwitchTarget,
    },
    /// Removes a session configuration
    RemoveSession { name: String },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct CliSwitchTarget {
    #[arg(short, long)]
    last_session: bool,

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
