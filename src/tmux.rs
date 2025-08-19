mod pane;
mod session;
#[cfg(any(test, feature = "integration_test"))]
#[allow(dead_code)]
mod tests;
mod window;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use std::{
    env::{self, VarError},
    str,
};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub use pane::{Direction, Pane, SplitBuilder, SplitSize};
pub use session::{Session, SessionBuilder};
pub use window::{Window, WindowBuilder};

pub fn tmux() -> Result<Command> {
    let mut command = Command::new("tmux");
    match (
        env::var("CELERIS_TMUX_SOCKET_NAME"),
        env::var("CELERIS_TMUX_SOCKET_PATH"),
    ) {
        (Ok(ref name), Err(VarError::NotPresent)) => command.args(["-L", name]),
        (Err(VarError::NotPresent), Ok(ref path)) => command.args(["-S", path]),
        (Err(VarError::NotUnicode(err)), _) | (_, Err(VarError::NotUnicode(err))) => {
            return Err(eyre!(
                "tmux socket target contains invalid unicode: {err:?}"
            ));
        }
        _ => return Ok(command),
    };
    Ok(command)
}

pub trait TmuxExecuteExt {
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

pub fn server_running() -> Result<bool> {
    let mut command = tmux()?;
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum RootOptions {
    Default,
    Custom(PathBuf),
}

#[derive(Debug, Eq, PartialEq)]
pub struct Root(RootOptions);

impl Root {
    pub fn custom(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Err(eyre!("Root path doesn't exist: {path:?}"));
        }

        Ok(Self(RootOptions::Custom(path)))
    }
}

impl Default for Root {
    fn default() -> Self {
        Self(RootOptions::Default)
    }
}

impl AsRef<RootOptions> for Root {
    fn as_ref(&self) -> &RootOptions {
        &self.0
    }
}

pub trait Target {
    fn get(&self) -> &str;

    fn target_exists(&self) -> Result<bool> {
        let has_session_status = tmux()?
            .args(["has-session", "-t", self.get()])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .wrap_err_with(|| "has-session failed to execute")?;
        Ok(has_session_status.success())
    }

    fn targeted_command(&self, command: &str) -> Result<Command> {
        if !self.target_exists()? {
            return Err(eyre!(
                "tried to execute a commend: {command} with a non-existing target: {}",
                self.get()
            ));
        }
        let mut tmux = tmux()?;
        tmux.args([command, "-t", self.get()]);
        Ok(tmux)
    }
}

#[derive(Clone, Debug)]
pub struct SessionTarget {
    session_id: String,
    target: String,
}

#[derive(Clone, Debug)]
pub struct WindowTarget {
    session_id: String,
    window_id: String,
    target: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PaneTarget {
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

pub trait BuilderTransform: Sized {
    fn try_builder_transform<T, Tr, E>(self, opt: Option<T>, transformer: Tr) -> Result<Self, E>
    where
        Tr: FnOnce(Self, T) -> Result<Self, E>,
    {
        match opt {
            Some(opt) => transformer(self, opt),
            None => Ok(self),
        }
    }

    fn builder_transform<T, Tr>(self, opt: Option<T>, transformer: Tr) -> Self
    where
        Tr: FnOnce(Self, T) -> Self,
    {
        match opt {
            Some(t) => transformer(self, t),
            None => self,
        }
    }
}
