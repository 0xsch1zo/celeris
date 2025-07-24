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

    // TODO: Maybe replace with an enum
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

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use super::*;
    const TESTING_SESSION: &str = "__sesh_testing";

    struct CleanSession {
        inner: Arc<Session>,
    }

    impl CleanSession {
        fn kill(&self) -> Result<()> {
            self.inner.target("kill-session")?.execute()?;
            Ok(())
        }
    }

    impl Deref for CleanSession {
        type Target = Session;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl Drop for CleanSession {
        fn drop(&mut self) {
            self.kill()
                .expect("kill-session failed - environment after test is not cleaned up");
        }
    }

    impl Session {
        fn detach_clients(&self) -> Result<()> {
            tmux()
                .args(["detach-client", "-s", &self.session_id])
                .execute()?;
            Ok(())
        }
    }

    fn testing_session() -> Result<CleanSession> {
        let session = Session::new(TESTING_SESSION, Root::Default)?;
        Ok(CleanSession(Arc::try_unwrap(session).unwrap()))
    }

    fn selected_pane_id(target: &str) -> Result<String> {
        Ok(tmux()
            .args(["display-message", "-p", "-t", target, "#{pane_id}"])
            .execute()?
            .trim()
            .to_owned())
    }

    // TODO: maybe a more complete test would be nice
    #[test]
    fn server_running_t() -> Result<()> {
        let _session = testing_session();
        assert_eq!(server_running()?, true);
        Ok(())
    }

    mod session {
        use super::*;

        mod from {
            use crate::tmux::{Session, TmuxExecuteExt};
            use color_eyre::Result;

            #[test]
            fn from() -> Result<()> {
                let session = super::testing_session()?;
                let session_from = Session::from(&session.session_id)?;
                let output = session_from
                    .target("display-message")?
                    .args(["-p", "test"])
                    .execute()?;
                assert_eq!(output.trim(), "test");
                session.kill()?;
                Ok(())
            }
        }

        #[test]
        fn target_exits() -> Result<()> {
            let session = testing_session()?;
            // also used for pane
            let window_target = format!("{}:{}", session.session_id, session.default_window_id);
            let targets = vec![
                format!("{}:", session.session_id),
                selected_pane_id(&window_target)?,
                window_target.clone(),
                "2137:".to_owned(),
                format!("{}:2137", session.session_id),
                format!("{}:{}.2137", session.session_id, session.default_window_id),
            ];

            targets.into_iter().try_for_each(|target| -> Result<()> {
                let exists = Session::target_exists(&target)?;
                let mut command = tmux();
                let status = command.args(["has-session", "-t", &target]).status()?;
                assert_eq!(
                    exists,
                    status.success(),
                    "incorrect status of a session/window/pane"
                );
                Ok(())
            })?;

            Ok(())
        }

        #[test]
        fn terminal_state() -> Result<()> {
            let state = Session::terminal_state()?;
            match (env::var("TMUX"), state) {
                (Ok(_), TerminalState::InTmux) => Ok(()),
                (Ok(_), TerminalState::Normal) => {
                    Err(eyre!("terminal state is Normal when TMUX var exists"))
                }
                (Err(env::VarError::NotPresent), TerminalState::Normal) => Ok(()),
                (Err(env::VarError::NotPresent), TerminalState::InTmux) => Err(eyre!(
                    "terminal state state is InTmux when TMUX var doesn't exist"
                )),
                (Err(err), _) => Err(err.into()),
            }
        }

        #[test]
        fn active_name() -> Result<()> {
            let session = testing_session()?;
            session.attach()?;
            let Some(active_name) = Session::active_name()? else {
                panic!("active_name claimed that session is not attached")
            };

            let output = tmux()
                .args(["display-message", "-p", "#{session_name}"])
                .execute()?;
            assert_eq!(active_name, output.trim());
            Ok(())
        }

        #[test]
        fn list_sessions() -> Result<()> {
            let session_name_1 = "__sesh_testing_1";
            let session_name_2 = "__sesh_testing_2";
            let _session1 = Session::new(session_name_1, Root::Default)?; // to stop the session
            // from being dropped
            let _session2 = Session::new(session_name_2, Root::Default)?;
            let sessions = Session::list_sessions()?;
            assert!(
                sessions
                    .iter()
                    .find(|name| *name == session_name_1)
                    .is_some()
            );
            assert!(
                sessions
                    .iter()
                    .find(|name| *name == session_name_2)
                    .is_some()
            );
            Ok(())
        }

        #[test]
        fn new_session() -> Result<()> {
            let session = testing_session()?;
            assert!(
                Session::target_exists(&session.session_id)?,
                "session doesn't exist"
            );
            Ok(())
        }

        #[test]
        fn new_session_custom_root() -> Result<()> {
            let session = Session::new(TESTING_SESSION, Root::Custom(env::temp_dir()))?;
            let output = session
                .target("display-message")?
                .args(["-p", "#{pane_current_path}"])
                .execute()?;
            assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
            Ok(())
        }

        fn attach_test(attached: TerminalState) -> Result<()> {
            let session = testing_session()?;
            let (command, handle) = session.spawn_attach(attached.clone())?;
            let output = session
                .target("display-message")?
                .args(["-p", "#{session_attached}"])
                .execute()?;
            if output.trim() != "1" {
                return Err(session.wait_attach(command, handle).unwrap_err());
            }
            match attached {
                TerminalState::Normal => session.detach_clients()?,
                // we assume that switch-client doesn't completly block
                TerminalState::InTmux => session.wait_attach(command, handle)?,
            };
            Ok(())
        }

        #[test_with::env(TMUX)]
        #[test]
        fn attach_in_tmux() -> Result<()> {
            attach_test(TerminalState::InTmux)?;
            Ok(())
        }

        #[test_with::no_env(TMUX)]
        #[test]
        fn attach_not_in_tmux() -> Result<()> {
            attach_test(TerminalState::Normal)?;
            Ok(())
        }
    }

    mod window {
        use super::*;

        #[test]
        fn new_window() -> Result<()> {
            let session = testing_session()?;

            let window = Window::builder(Arc::clone(&session)).build()?;
            assert!(
                Session::target_exists(&window.window_core.target)?,
                "window doesn't exist"
            );
            let output = session.target("list-windows")?.execute()?;
            let count = output.lines().count();
            assert_eq!(count, 1, "default session window hasn't been moved");
            Ok(())
        }

        #[test]
        fn new_window_custom_path() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).root(env::temp_dir()).build()?;

            let output = window
                .window_core
                .target("display-message")?
                .args(["-p", "#{pane_current_path}"])
                .execute()?;

            assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
            Ok(())
        }

        #[test]
        fn new_window_named() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).name("test".to_owned()).build()?;

            let output = window
                .window_core
                .target("display-message")?
                .args(["-p", "#{window_name}"])
                .execute()?;
            assert_eq!(output.trim(), "test");
            Ok(())
        }

        #[test]
        fn set_option() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            let option = ("allow-rename", "off");
            window.window_core.set_option(option.0, option.1)?;

            let output = window
                .window_core
                .target("show-window-options")?
                .execute()?;
            let option_got = output.lines().find(|line| line.contains(option.0));
            let option_got =
                option_got.ok_or_eyre("couldn't find option which was supposed to be set")?;
            let option_got = option_got.split_whitespace().collect::<Vec<_>>();
            assert_eq!(option_got.len(), 2);
            assert_eq!(option_got[0], option.0);
            assert_eq!(option_got[1], option.1);
            Ok(())
        }

        #[test]
        fn select() -> Result<()> {
            let session = testing_session()?;
            let window1 = Window::builder(Arc::clone(&session)).build()?;
            let _window2 = Window::builder(Arc::clone(&session)).build()?;
            window1.select()?;
            let output = session
                .target("display-message")?
                .args(["-p", "#{window_id}"])
                .execute()?;
            assert!(window1.window_core.target.contains(output.trim()));
            Ok(())
        }

        // Kind of unable to test this so this just checks if there was an error
        // even if testing this is possible there is just no point because most of the logic is the
        // burden of tmux
        #[test]
        fn even_out() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            window.event_out(Direction::Horizontal)?;
            window.event_out(Direction::Vertical)?;
            Ok(())
        }

        #[test]
        fn default_pane() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            let pane = window.default_pane();
            assert_eq!(Session::target_exists(&pane.target)?, true);
            Ok(())
        }
    }

    mod pane {
        use std::{thread, time::Duration};

        use super::*;

        #[test]
        fn split() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            let pane1 = window.default_pane();
            let pane2 = pane1.split_builder(Direction::Vertical).build()?;

            assert_eq!(Session::target_exists(&pane1.target)?, true);
            assert_eq!(Session::target_exists(&pane2.target)?, true);

            let output = window.window_core.target("list-panes")?.execute()?;
            assert_eq!(output.lines().count(), 2);
            Ok(())
        }

        #[test]
        fn split_custom_path() -> Result<()> {
            let session = Session::new(TESTING_SESSION, Root::Default)?;
            let window = Window::builder(session).build()?;
            let pane = window
                .default_pane
                .split_builder(Direction::Vertical)
                .root(env::temp_dir())
                .build()?;
            let output = pane
                .target("display-message")?
                .args(["-p", "#{pane_current_path}"])
                .execute()?;
            assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
            Ok(())
        }

        #[test]
        fn split_percentage_sized() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            let pane = window
                .default_pane()
                .split_builder(Direction::Horizontal)
                .size(SplitSize::Percentage(0))
                .build()?;

            let output = window
                .window_core
                .target("display-message")?
                .args(["-p", "#{window_width}"])
                .execute()?;
            assert!(
                output.trim().parse::<usize>()? >= 1,
                "insufficent window size for testing"
            );

            let output = pane
                .target("display-message")?
                .args(["-p", "#{pane_width}"])
                .execute()?;

            assert_eq!(output.trim(), "1");

            let _ = window
                .default_pane()
                .split_builder(Direction::Horizontal)
                .size(SplitSize::Percentage(101))
                .build()
                .unwrap_err();
            Ok(())
        }

        #[test]
        fn split_absolute_sized() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            let pane = window
                .default_pane()
                .split_builder(Direction::Horizontal)
                .size(SplitSize::Absolute(1))
                .build()?;

            let output = window
                .window_core
                .target("display-message")?
                .args(["-p", "#{window_width}"])
                .execute()?;
            assert!(
                output.trim().parse::<usize>()? >= 1,
                "insufficent window size for testing"
            );

            let output = pane
                .target("display-message")?
                .args(["-p", "#{pane_width}"])
                .execute()?;

            assert_eq!(output.trim(), "1");
            Ok(())
        }

        #[test]
        fn select() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(Arc::clone(&session)).build()?;
            let pane1 = window.default_pane();
            let _pane2 = pane1.split_builder(Direction::Vertical).build();
            pane1.select()?;
            let output = session
                .target("display-message")?
                .args(["-p", "#{pane_id}"])
                .execute()?;
            assert!(pane1.target.contains(output.trim()));
            Ok(())
        }

        // Just checks for error. Testing this would be complicated
        #[test]
        fn run_command() -> Result<()> {
            let session = testing_session()?;
            let window = Window::builder(session).build()?;
            //should run until Ctrl+C or the session is killled. Will work
            // only on most systems. Testing this without getting execution
            // is probably impossible
            let real_command = "cat";
            let command = format!("'{real_command}'"); // to ignore aliases
            let pane = window.default_pane();
            pane.run_command(&command)?;
            // Yes the shell is sometimes this slow
            thread::sleep(Duration::from_secs(1));
            let output = pane
                .target("display-message")?
                .args(["-p", "#{pane_current_command}"])
                .execute()?;
            assert_eq!(output.trim(), real_command);
            Ok(())
        }
    }
}
