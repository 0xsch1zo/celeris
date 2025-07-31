#[cfg(test)]
mod tests;

use crate::utils;
use color_eyre::{
    Result,
    eyre::{Context, ContextCompat, OptionExt, eyre},
};
use itertools::Itertools;
use std::sync::{Arc, Mutex};
use std::{env, io::Read};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use std::{process::Child, str};

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

fn tmux_target_command(target: &str, command: &str) -> Result<Command> {
    if !Session::target_exists(target)? {
        return Err(eyre!("target: {target}, doesn't exist"));
    }
    let mut tmux = tmux();
    tmux.args([command, "-t", &target]);
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

// TODO: check if the directions are right
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum SplitSize {
    Percentage(u8),
    Absolute(u32),
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

#[derive(Clone, Debug)]
pub struct Session {
    session_id: String, // this is the target
    window_count: Arc<Mutex<usize>>,
    default_window_id: String,
}

impl Session {
    // Can't run this if in tmux session already
    pub fn new(session_name: &str, root: Root) -> Result<Arc<Self>> {
        if Self::target_exists(session_name)? {
            return Err(eyre!("session with name: {session_name}, already exists"));
        }
        const DELIM: &str = "|";
        let mut command = tmux();
        // need to use low level api
        command.args([
            "new-session",
            "-d",
            "-s",
            session_name,
            "-P",
            "-F",
            &format!("{}{}{}", "#{window_id}", DELIM, "#{session_id}"),
        ]);

        if let Root::Custom(root) = root {
            command.args(["-c", &utils::path_to_string(&root)?]);
        }

        let output = command.execute()?;
        let (default_window_id, session_id) =
            output.trim().split_once(DELIM).ok_or_eyre(format!(
                "failed to create session, couldn't parse session or window id: {}",
                output
            ))?;

        Ok(Arc::new(Self {
            session_id: session_id.to_string(),
            window_count: Arc::new(Mutex::new(0)),
            default_window_id: default_window_id.to_string(),
        }))
    }

    pub fn from(session_identifier: &str) -> Result<Arc<Session>> {
        if !Self::target_exists(session_identifier)? {
            return Err(eyre!("session: {session_identifier}, doesn't exist"));
        }

        const DELIM: &str = "|";
        let output = tmux()
            .args([
                "display-message",
                "-p",
                "-t",
                session_identifier,
                &format!(
                    "{}{}{}{}{}",
                    "#{window_id}", DELIM, "#{session_id}", DELIM, "#{session_windows}"
                ),
            ])
            .execute()?;

        let [default_window_id, session_id, window_count] =
            output.trim().splitn(3, DELIM).collect_vec()[..]
        else {
            return Err(eyre!(
                "incorrect count of variables returned from display-message {output}"
            ));
        };
        let window_count = window_count.parse::<usize>().wrap_err(
            "failed to parse window_count while creating session object from existing session",
        )?;

        Ok(Arc::new(Self {
            session_id: session_id.to_owned(),
            window_count: Arc::new(Mutex::new(window_count)),
            default_window_id: default_window_id.to_string(),
        }))
    }

    fn target(&self, command: &str) -> Result<Command> {
        let target = format!("{}:", self.session_id);
        tmux_target_command(&target, command)
    }

    // Checks if in the current environment there is an attached session
    fn terminal_state() -> Result<TerminalState> {
        Ok(match env::var("TMUX") {
            Ok(_) => TerminalState::InTmux,
            Err(env::VarError::NotPresent) => TerminalState::Normal,
            Err(err) => return Err(err).wrap_err("failed to check for active tmux session"),
        })
    }

    pub fn active_name() -> Result<Option<String>> {
        if !server_running()? {
            return Ok(None);
        }

        if let TerminalState::Normal = Self::terminal_state()? {
            return Ok(None);
        }

        let output = tmux()
            .args(["display-message", "-p", "#{session_name}"])
            .execute()?;
        Ok(Some(output.trim().to_owned()))
    }

    pub fn list_sessions() -> Result<Vec<String>> {
        if !server_running()? {
            return Ok(Vec::new());
        }
        let output = tmux()
            .args(["list-sessions", "-F", "#{session_name}"])
            .execute()?;
        Ok(output.trim().lines().map(ToOwned::to_owned).collect())
    }

    fn spawn_attach(&self, attached: TerminalState) -> Result<(Command, Child)> {
        let mut command = match attached {
            TerminalState::InTmux => self.target("switch-client")?,
            TerminalState::Normal => self.target("attach-session")?,
        };

        let child = command
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err("failed to execute attach session command")?;

        Ok((command, child))
    }

    fn wait_attach(&self, command: Command, mut handle: Child) -> Result<()> {
        let status = handle
            .wait()
            .wrap_err("failed to wait for attach session command, couldn't get status")?;

        if !status.success() {
            let mut error = String::new();
            handle
                .stderr
                .take()
                .wrap_err("stderr of tmux not available")
                .wrap_err(format!(
                    "failed to retrieve error from failing tmux: {:?}",
                    command
                ))?
                .read_to_string(&mut error)
                .wrap_err(format!(
                    "failed to retrieve error from failing tmux: {:?}",
                    command
                ))?;
            return Err(eyre!("tmux: {:?}, failed with: {error}", command)
                .wrap_err("failed to attach session"));
        }

        Ok(())
    }

    pub fn attach(&self) -> Result<()> {
        let (command, handle) = self.spawn_attach(Self::terminal_state()?)?;
        self.wait_attach(command, handle)?;
        Ok(())
    }

    fn register_window(&self, window: &WindowCore) -> Result<()> {
        let mut count = self.window_count.lock().unwrap();
        if *count == 0 {
            let target = format!("{}:{}", self.session_id, self.default_window_id);
            window.move_kill(&target)?;
        }
        *count += 1;
        Ok(())
    }

    fn target_exists(target: &str) -> Result<bool> {
        let has_session_status = tmux()
            .args(["has-session", "-t", &target])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .wrap_err_with(|| "has-session failed to execute")?;
        Ok(has_session_status.success())
    }
}

#[derive(Clone, Debug)]
pub struct WindowBuilder {
    name: Option<String>,
    shell_command: Option<String>,
    root: Root,
    session: Arc<Session>,
}

impl WindowBuilder {
    pub fn new(session: Arc<Session>) -> Self {
        Self {
            name: None,
            shell_command: None,
            root: Root::Default,
            session: session,
        }
    }

    pub fn name(&mut self, name: String) -> &mut Self {
        self.name = Some(name);
        self
    }

    pub fn root(&mut self, path: PathBuf) -> &mut Self {
        self.root = Root::Custom(path);
        self
    }

    pub fn shell_command(&mut self, command: String) -> &mut Self {
        self.shell_command = Some(command);
        self
    }

    fn prepare_options(&self) -> Result<Vec<String>> {
        let mut options: Vec<String> = Vec::new();
        self.prepare_name(&mut options);
        self.prepare_shell_command(&mut options);
        self.prepare_root(&mut options)?;

        Ok(options)
    }

    fn prepare_name(&self, options: &mut Vec<String>) {
        let Some(name) = &self.name else {
            return;
        };

        options.extend(["-n".to_owned(), name.to_owned()]);
    }

    fn prepare_shell_command(&self, options: &mut Vec<String>) {
        let Some(command) = &self.shell_command else {
            return;
        };
        options.push(command.to_owned());
    }

    fn prepare_root(&self, options: &mut Vec<String>) -> Result<()> {
        let Root::Custom(path) = &self.root else {
            return Ok(());
        };
        options.extend(["-c".to_owned(), utils::path_to_string(path)?]);
        Ok(())
    }

    fn create_window(&self) -> Result<WindowCore> {
        const DELIM: &str = "|";
        let output = self
            .session
            .target("new-window")?
            .args([
                "-P",
                "-F",
                &format!("{}{}{}", "#{pane_id}", DELIM, "#{window_id}"),
            ])
            .args(self.prepare_options()?)
            .execute()?;
        let (default_pane_id, window_id) = output.trim().split_once(DELIM).ok_or_eyre(format!(
            "failed to create session, couldn't parse session or window id: {}",
            output
        ))?;

        Ok(WindowCore::new(
            Arc::clone(&self.session),
            window_id,
            default_pane_id,
        ))
    }

    pub fn build(&mut self) -> Result<Arc<Window>> {
        let window_core = self.create_window()?;
        self.session.register_window(&window_core)?;

        if let Some(_) = self.name {
            window_core.set_option("allow-rename", "off")?;
        }
        Ok(Window::new(window_core))
    }
}

#[derive(Clone, Debug)]
struct WindowCore {
    session: Arc<Session>,
    window_id: String,
    target: String,
    default_pane_id: String,
}

impl WindowCore {
    fn new(session: Arc<Session>, window_id: &str, default_pane_id: &str) -> Self {
        let target = format!("{}:{}", session.session_id, window_id);
        Self {
            session: session,
            window_id: window_id.to_string(),
            default_pane_id: default_pane_id.to_string(),
            target,
        }
    }

    fn target(&self, command: &str) -> Result<Command> {
        tmux_target_command(&self.target, command)
    }

    fn set_option(&self, option: &str, value: &str) -> Result<()> {
        self.target("set-window-option")?
            .args([option, value])
            .execute()?;
        Ok(())
    }

    fn select(&self) -> Result<()> {
        self.target("select-window")?.execute()?;
        Ok(())
    }

    fn even_out(&self, direction: Direction) -> Result<()> {
        let mut command = self.target("select-layout")?;
        match direction {
            Direction::Vertical => command.arg("even-vertical"),
            Direction::Horizontal => command.arg("even-horizontal"),
        };
        command.execute()?;
        Ok(())
    }

    // Only for the purpose of killing the default window
    fn move_kill(&self, target: &str) -> Result<()> {
        // use a proper source target
        self.target("move-window")?
            .args(["-s", &self.window_id, "-t", target, "-k"])
            .execute()?;
        Ok(())
    }
}

// all this is because I have a skill issue and in the architecture there is an inherent dependency
// cycle between the default pane and window. Couldn't think of a way to have a clear api without
// this
#[derive(Clone, Debug)]
pub struct Window {
    window_core: Arc<WindowCore>,
    default_pane: Arc<Pane>,
}

impl Window {
    fn new(window_core: WindowCore) -> Arc<Self> {
        let window_core = Arc::new(window_core);
        Arc::new(Self {
            window_core: Arc::clone(&window_core),
            default_pane: Arc::new(Pane::new(
                Arc::clone(&window_core),
                &window_core.default_pane_id,
            )),
        })
    }

    pub fn builder(session: Arc<Session>) -> WindowBuilder {
        WindowBuilder::new(session)
    }

    pub fn default_pane(&self) -> Arc<Pane> {
        Arc::clone(&self.default_pane)
    }

    pub fn event_out(&self, direction: Direction) -> Result<()> {
        self.window_core.even_out(direction)
    }

    pub fn select(&self) -> Result<()> {
        self.window_core.select()
    }
}

pub struct SplitBuilder {
    sibling_pane: Arc<Pane>,
    direction: Direction,
    root: Root,
    size: Option<SplitSize>,
}

impl SplitBuilder {
    fn new(sibling_pane: Arc<Pane>, direction: Direction) -> Self {
        Self {
            direction: direction,
            size: None,
            root: Root::Default,
            sibling_pane: sibling_pane,
        }
    }

    pub fn size(&mut self, size: SplitSize) -> &mut Self {
        self.size = Some(size);
        self
    }

    pub fn root(&mut self, path: PathBuf) -> &mut Self {
        self.root = Root::Custom(path);
        self
    }

    fn prepare_options(&self) -> Result<Vec<String>> {
        let mut options = Vec::new();
        self.prepare_size(&mut options)?;
        self.prepare_root(&mut options)?;
        Ok(options)
    }

    fn prepare_size(&self, options: &mut Vec<String>) -> Result<()> {
        let Some(size) = self.size else {
            return Ok(());
        };

        match size {
            SplitSize::Percentage(percentage) if percentage <= 100 => {
                options.extend(["-l".to_owned(), format!("{percentage}%")]);
            }
            SplitSize::Percentage(percentage) => {
                return Err(eyre!("Percentage amount above 100: {percentage}"));
            }
            SplitSize::Absolute(absolute) => {
                options.extend(["-l".to_owned(), absolute.to_string()])
            }
        };

        Ok(())
    }

    fn prepare_root(&self, options: &mut Vec<String>) -> Result<()> {
        let Root::Custom(path) = &self.root else {
            return Ok(());
        };

        options.extend(["-c".to_owned(), utils::path_to_string(path)?]);
        Ok(())
    }

    fn split_command(&self) -> Result<Command> {
        let mut command = self.sibling_pane.target("split-window")?;
        command.args(["-P", "-F", "#{pane_id}"]);
        match self.direction {
            Direction::Vertical => command.arg("-v"),
            Direction::Horizontal => command.arg("-h"),
        };

        command.args(self.prepare_options()?);
        Ok(command)
    }

    pub fn build(&self) -> Result<Pane> {
        let output = self.split_command()?.execute()?;
        Ok(Pane::new(
            Arc::clone(&self.sibling_pane.window),
            output.trim(),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    window: Arc<WindowCore>,
    target: String,
}

impl Pane {
    fn new(window: Arc<WindowCore>, pane_id: &str) -> Self {
        let target = format!(
            "{}:{}.{}",
            window.session.session_id, window.window_id, pane_id
        );
        Self {
            window: window,
            target,
        }
    }

    fn target(&self, command: &str) -> Result<Command> {
        tmux_target_command(&self.target, command)
    }

    // No reasson to return arc here because it's owned which is fine with rhai
    pub fn split_builder(self: &Arc<Self>, direction: Direction) -> SplitBuilder {
        SplitBuilder::new(Arc::clone(self), direction)
    }

    pub fn select(&self) -> Result<()> {
        self.target("select-pane")?.execute()?;
        Ok(())
    }

    pub fn run_command(&self, command: &str) -> Result<()> {
        self.target("send-keys")?
            .args([command, "Enter"])
            .execute()?;
        Ok(())
    }
}
