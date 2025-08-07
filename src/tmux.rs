mod pane;
mod session;
#[cfg(test)]
mod tests;
mod window;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use std::{fmt::Display, str};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub use pane::{Direction, Pane, SplitBuilder, SplitSize};
pub use session::{Session, SessionBuilder};
pub use window::{Window, WindowBuilder};

// TODO: provide a custom tmux command builder for special cases
fn tmux() -> Command {
    Command::new("tmux")
}

trait TmuxExecuteExt {
    fn execute(&mut self) -> Result<String>;
}

impl TmuxExecuteExt for Command {
    fn execute(&mut self) -> Result<String> {
        let output = self
            .output()
            .wrap_err_with(|| format!("failed to execute tmux command: {:?}", self))?;

        if !output.status.success() {
            return Err(eyre!(
                "Command: {:?}: {}",
                self,
                str::from_utf8(&output.stderr).wrap_err_with(|| "Tmux returned invalid utf-8")?
            ));
        }
        Ok(String::from_utf8(output.stdout).wrap_err_with(|| "Tmux returned invalid utf-8")?)
    }
}

fn targeted_command<T: Target + Display>(target: &T, command: &str) -> Result<Command> {
    if !target_exists(target)? {
        return Err(eyre!("target: {target}, doesn't exist"));
    }
    let mut tmux = tmux();
    tmux.args([command, "-t", target.get()]);
    Ok(tmux)
}

pub fn server_running() -> Result<bool> {
    let mut command = tmux();
    command.args(["display-message", "-p", "#{socket_path}"]);

    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .wrap_err_with(|| format!("failed to execute tmux command: {:?}", command))?;

    Ok(status.success())
}

#[derive(Clone, Debug)]
enum TerminalState {
    InTmux,
    Normal,
}

#[derive(Clone, Debug)]
pub enum Root {
    Default,
    Custom(PathBuf),
}

trait Target {
    fn get(&self) -> &str;
}

#[derive(Clone, Debug)]
struct SessionTarget {
    session_id: String,
    target: String,
}

#[derive(Clone, Debug)]
struct WindowTarget {
    session_id: String,
    window_id: String,
    target: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct PaneTarget {
    session_id: String,
    window_id: String,
    pane_id: String,
    target: String,
}

impl SessionTarget {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_owned(),
            target: session_id.to_owned(),
        }
    }

    pub fn window_target(&self, window_id: &str) -> WindowTarget {
        WindowTarget::new(self.session_id.clone(), window_id.to_owned())
    }
}

impl Target for SessionTarget {
    fn get(&self) -> &str {
        &self.target
    }
}

// TODO: figure out something better
impl Display for SessionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl WindowTarget {
    fn new(session_id: String, window_id: String) -> Self {
        Self {
            target: format!("{session_id}:{window_id}"),
            session_id,
            window_id,
        }
    }

    pub fn pane_target(&self, pane_id: &str) -> PaneTarget {
        PaneTarget::new(
            self.session_id.clone(),
            self.window_id.clone(),
            pane_id.to_owned(),
        )
    }
}

impl Display for WindowTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl Target for WindowTarget {
    fn get(&self) -> &str {
        &self.target
    }
}

impl PaneTarget {
    fn new(session_id: String, window_id: String, pane_id: String) -> Self {
        Self {
            target: format!("{session_id}:{window_id}.{pane_id}"),
            session_id,
            window_id,
            pane_id,
        }
    }

    fn from_sibling(sibling: &Self, pane_id: &str) -> Self {
        Self {
            target: format!("{}:{}.{}", sibling.session_id, sibling.window_id, pane_id),
            pane_id: pane_id.to_owned(),
            window_id: sibling.session_id.clone(),
            session_id: sibling.session_id.clone(),
        }
    }
}

impl Target for PaneTarget {
    fn get(&self) -> &str {
        &self.target
    }
}

impl Display for PaneTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

fn target_exists<T: Target>(target: &T) -> Result<bool> {
    let has_session_status = tmux()
        .args(["has-session", "-t", target.get()])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .wrap_err_with(|| "has-session failed to execute")?;
    Ok(has_session_status.success())
}
