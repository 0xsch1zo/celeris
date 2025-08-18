use crate::session_manager::{
    CreateSessionOptions, ListSessionsOptions as MgrListSessionsOptions, SwitchTarget,
};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "A powerful git-aware session-manager written in Rust")]
#[command(long_about = None)]
#[command(version = "v0.1.0")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Set a custom directory where the main session and scripts are stored
    #[arg(short = 'c', long, env = "CELERIS_CONFIG_DIR")]
    pub config_dir: Option<PathBuf>,

    /// Set a custom directory where last session opened is cached
    #[arg(short = 'a', long, env = "CELERIS_CACHE_DIR")]
    pub cache_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Find repos on search roots declared in the config
    Search,
    /// List configured and/or active sessions
    List {
        #[command(flatten)]
        opts: ListSessionsOptions,
    },
    /// Create a layout and open it in $EDITOR
    Create {
        #[command(flatten)]
        opts: CreateOptions,
    },
    /// Create layouts in bulk from supplied paths from stdin('~' is supported). Duplicate file
    /// names will not be deduplicated as usual
    CreateAll,
    /// Edit an existing layout
    Edit {
        /// Name of the layout to be edited
        name: String,
    },
    /// Switch to a running session if exists or load the layout
    Switch {
        #[command(flatten)]
        target: CliSwitchTarget,
    },
    /// Remove a layout
    Remove {
        /// Name of the layout to be removed
        name: String,
    },
}

#[derive(Args)]
pub struct CreateOptions {
    /// Root path of a session. By default the name is deduced automatically
    path: PathBuf,
    /// Set custom name for a layout
    #[arg(short, long)]
    name: Option<String>,
    /// Don't open the layout file in the $EDITOR
    #[arg(short, long)]
    disable_editor: bool,
    /// Print the name of the layout created in a machine readable format
    #[arg(short, long)]
    machine_readable: bool,
}

impl From<CreateOptions> for CreateSessionOptions {
    fn from(value: CreateOptions) -> Self {
        Self {
            path: value.path,
            name: value.name,
            disable_editor: value.disable_editor,
            machine_readable: value.machine_readable,
        }
    }
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct CliSwitchTarget {
    /// Switch to the last loaded layout. Name mustn't be supplied when this flag is passed
    #[arg(short, long)]
    last_session: bool,
    /// Name of the running session/predefined layout to switch into
    name: Option<String>,
}

impl From<CliSwitchTarget> for SwitchTarget {
    fn from(value: CliSwitchTarget) -> Self {
        match value.last_session {
            true => SwitchTarget::LastSession,
            false => SwitchTarget::Session(value.name.unwrap()),
        }
    }
}

#[derive(Args)]
pub struct ListSessionsOptions {
    /// Print the seessions in a format that can easily be used in a status bar of tmux
    #[arg(short, long)]
    tmux_format: bool,

    #[command(flatten)]
    conflicting: ListSessionsConflicting,
}

impl Into<MgrListSessionsOptions> for ListSessionsOptions {
    fn into(self) -> MgrListSessionsOptions {
        MgrListSessionsOptions {
            tmux_format: self.tmux_format,
            include_active: self.conflicting.include_active,
            exclude_running: self.conflicting.exclude_running,
            only_running: self.conflicting.only_running,
        }
    }
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct ListSessionsConflicting {
    /// Include currently active tmux session in the listing(if exists). Signified with an asterisk
    /// at the end
    #[arg(short, long)]
    include_active: bool,
    /// Exclude currently running tmux sessions
    #[arg(short, long)]
    exclude_running: bool,
    /// List only running sessions
    #[arg(short, long)]
    only_running: bool,
}
