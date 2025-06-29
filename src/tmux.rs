use color_eyre::{
    Result,
    eyre::Context,
    eyre::{OptionExt, eyre},
};
use std::process;
use std::process::Stdio;
use std::str;
use std::sync::{Arc, Mutex};

// TODO: provide a custom tmux command builder for special cases
// TODO: handle tmux not being available
fn tmux() -> process::Command {
    process::Command::new("tmux")
}

// Built-in error handling
fn execute(mut command: process::Command) -> Result<String> {
    let output = command
        .output()
        .wrap_err_with(|| format!("failed to execute tmux command: {:?}", command))?;

    if !output.status.success() {
        return Err(eyre!(
            "Command: {:?}: {}",
            command,
            str::from_utf8(&output.stderr).wrap_err_with(|| "Tmux returned invalid utf-8")?
        ));
    }
    Ok(String::from_utf8(output.stdout).wrap_err_with(|| "Tmux returned invalid utf-8")?)
}

fn tmux_target_command(target: &str, command: &str) -> Result<process::Command> {
    if !Session::target_exists(target)? {
        return Err(eyre!("target: {target}, doesn't exist"));
    }
    let mut tmux = tmux();
    tmux.args([command, "-t", &target]);
    Ok(tmux)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum SplitSize {
    Percentage(u8),
    Fixed(u32),
}

#[derive(Clone, Debug)]
pub struct Session {
    session_id: String, // this is the target
    window_count: Arc<Mutex<usize>>,
    default_window_id: String,
}

impl Session {
    // Can't run this if in tmux session already
    pub fn new(session_name: &str) -> Result<Arc<Self>> {
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
        let output = execute(command)?;
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

    // TODO: Maybe replace with an enum
    fn target(&self, command: &str) -> Result<process::Command> {
        let target = format!("{}:", self.session_id);
        tmux_target_command(&target, command)
    }

    pub fn new_window(
        self: &Arc<Self>,
        name: Option<&str>,
        shell_command: Option<&str>,
    ) -> Result<Arc<Window>> {
        let window_core = WindowCore::new(Arc::clone(self), name, shell_command)?;
        let mut count = self.window_count.lock().unwrap();
        if *count == 0 {
            let target = format!("{}:{}", self.session_id, self.default_window_id);
            window_core.move_kill(&target)?;
        }
        *count += 1;
        Ok(Window::new(window_core))
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

// just a better interface can't int
/*#[derive(Clone, Debug)]
pub struct WindowBuilder<'a> {
    name: Option<&'a str>,
    shell_command: Option<&'a str>,
    session: Arc<Session>,
}

impl<'a> WindowBuilder<'a> {
    pub fn new(session: &Arc<Session>) -> Self {
        Self {
            name: None,
            shell_command: None,
            session: Arc::clone(session),
        }
    }

    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn shell_command(mut self, command: &'a str) -> Self {
        self.shell_command = Some(command);
        self
    }

    pub fn build(self) -> Result<Arc<Window>> {
        self.session.new_window(self.name, self.shell_command)
    }
}*/

#[derive(Clone, Debug)]
struct WindowCore {
    session: Arc<Session>,
    window_id: String,
    target: String,
    default_pane_id: String,
}

impl WindowCore {
    fn new(session: Arc<Session>, name: Option<&str>, shell_command: Option<&str>) -> Result<Self> {
        const DELIM: &str = "|";
        let mut command = session.target("new-window")?;
        command.args([
            "-P",
            "-F",
            &format!("{}{}{}", "#{pane_id}", DELIM, "#{window_id}"),
        ]);

        if let Some(name) = name {
            command.args(["-n", name]);
        }
        if let Some(shell_command) = shell_command {
            command.arg(shell_command);
        }

        let output = execute(command)?;
        let (default_pane_id, window_id) = output.trim().split_once(DELIM).ok_or_eyre(format!(
            "failed to create session, couldn't parse session or window id: {}",
            output
        ))?;

        let target = format!("{}:{}", session.session_id, window_id);
        let window = Self {
            session: session,
            window_id: window_id.to_string(),
            default_pane_id: default_pane_id.to_string(),
            target,
        };

        if let Some(_) = name {
            window.set_option("allow-rename", "off")?;
        }

        Ok(window)
    }

    fn target(&self, command: &str) -> Result<process::Command> {
        tmux_target_command(&self.target, command)
    }

    fn set_option(&self, option: &str, value: &str) -> Result<()> {
        let mut command = self.target("set-window-option")?;
        command.args([option, value]);
        execute(command)?;
        Ok(())
    }

    fn select(&self) -> Result<()> {
        execute(self.target("select-window")?)?;
        Ok(())
    }

    fn even_out(&self, direction: Direction) -> Result<()> {
        let mut command = self.target("select-layout")?;
        match direction {
            Direction::Vertical => command.arg("even-vertical"),
            Direction::Horizontal => command.arg("even-horizontal"),
        };
        execute(command)?;
        Ok(())
    }

    // Only for the purpose of killing the default window
    fn move_kill(&self, target: &str) -> Result<()> {
        let mut command = self.target("move-window")?;
        // use a proper source target
        command.args(["-s", &self.window_id, "-t", target, "-k"]);
        execute(command)?;
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
            default_pane: Arc::new(Pane::new(&window_core, &window_core.default_pane_id)),
        })
    }

    /*pub fn builder(session: &Arc<Session>) -> WindowBuilder {
        WindowBuilder::new(session)
    }*/

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

#[derive(Clone, Debug)]
pub struct Pane {
    window: Arc<WindowCore>,
    target: String,
}

impl Pane {
    fn new(window: &Arc<WindowCore>, pane_id: &str) -> Self {
        let target = format!(
            "{}:{}.{}",
            window.session.session_id, window.window_id, pane_id
        );
        Self {
            window: Arc::clone(window),
            target,
        }
    }

    fn target(&self, command: &str) -> Result<process::Command> {
        tmux_target_command(&self.target, command)
    }

    fn split_command(&self, split: Direction) -> Result<process::Command> {
        let mut command = self.target("split-window")?;
        command.args(["-P", "-F", "#{pane_id}"]);
        match split {
            Direction::Vertical => command.arg("-v"),
            Direction::Horizontal => command.arg("-h"),
        };

        Ok(command)
    }

    // No reasson to return arc here because it's owned which is fine with rhai
    pub fn split(&self, direction: Direction) -> Result<Pane> {
        let output = execute(self.split_command(direction)?)?;
        Ok(Pane::new(&self.window, output.trim()))
    }

    // TODO: maybe add support for below 3.1
    pub fn split_with_size(&self, direction: Direction, size: SplitSize) -> Result<Pane> {
        let mut command = self.split_command(direction)?;
        match size {
            SplitSize::Percentage(percentage) if percentage <= 100 => {
                command.args(["-l", &format!("{percentage}%")])
            }
            SplitSize::Percentage(percentage) => {
                return Err(eyre!("Percentage amount above 100: {percentage}"));
            }
            SplitSize::Fixed(fixed) => command.args(["-l", &fixed.to_string()]),
        };

        let output = execute(command)?;
        Ok(Pane::new(&self.window, output.trim()))
    }

    pub fn select(&self) -> Result<()> {
        execute(self.target("select-pane")?)?;
        Ok(())
    }

    pub fn run_command(&self, command: &str) -> Result<()> {
        let mut send_keys = self.target("send-keys")?;
        send_keys.args([command, "Enter"]);

        execute(send_keys)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kill_session(session: &Session) -> Result<()> {
        execute(Session::target(session, "kill-session")?)?;
        Ok(())
    }

    impl Drop for Session {
        fn drop(&mut self) {
            kill_session(self)
                .expect("kill-session failed - environment after test is not cleaned up");
        }
    }

    fn testing_session() -> Result<Arc<Session>> {
        // Generate "random" id. Because tests are run in parallel
        Ok(Session::new("__sesh_testing")?)
    }

    fn selected_pane_id(target: &str) -> Result<String> {
        let mut command = tmux();
        command.args(["display-message", "-p", "-t", target, "#{pane_id}"]);
        Ok(execute(command)?.trim().to_string())
    }

    mod session {
        use super::*;

        #[test]
        fn target_exits() -> Result<()> {
            let session = testing_session()?;
            // also used for pane
            let window_target = format!("{}:{}", session.session_id, session.default_window_id);
            let targets = vec![
                format!("{}:", session.session_id),
                selected_pane_id(&window_target)?,
                window_target.clone(),
                "2137:".to_string(),
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
        fn new_session() -> Result<()> {
            let session = testing_session()?;
            assert!(
                Session::target_exists(&session.session_id)?,
                "session doesn't exist"
            );
            Ok(())
        }

        #[test]
        fn new_window() -> Result<()> {
            let session = testing_session()?;

            let window = session.new_window(None, None)?;
            assert!(
                Session::target_exists(&window.window_core.target)?,
                "window doesn't exist"
            );
            let output = execute(session.target("list-windows")?)?;
            let count = output.lines().count();
            assert_eq!(count, 1, "default session window hasn't been moved");
            Ok(())
        }
    }

    mod window {
        use super::*;

        #[test]
        fn set_option() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            let option = ("allow-rename", "off");
            window.window_core.set_option(option.0, option.1)?;

            let output = execute(window.window_core.target("show-window-options")?)?;
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
            let window1 = session.new_window(None, None)?;
            let _window2 = session.new_window(None, None)?;
            window1.select()?;
            let mut command = session.target("display-message")?;
            command.args(["-p", "#{window_id}"]);
            let output = execute(command)?;
            assert!(window1.window_core.target.contains(output.trim()));
            Ok(())
        }

        // Kind of unable to test this so this just checks if there was an error
        // even if testing this is possible there is just no point because most of the logic is the
        // burden of tmux
        #[test]
        fn even_out() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            window.event_out(Direction::Horizontal)?;
            window.event_out(Direction::Vertical)?;
            Ok(())
        }

        #[test]
        fn default_pane() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            let pane = window.default_pane();
            assert_eq!(Session::target_exists(&pane.target)?, true);
            Ok(())
        }
    }

    mod pane {
        use super::*;

        #[test]
        fn split() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            let pane1 = window.default_pane();
            let pane2 = pane1.split(Direction::Vertical)?;

            assert_eq!(Session::target_exists(&pane1.target)?, true);
            assert_eq!(Session::target_exists(&pane2.target)?, true);

            let output = execute(window.window_core.target("list-panes")?)?;
            assert_eq!(output.lines().count(), 2);
            Ok(())
        }

        #[test]
        fn select() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            let pane1 = window.default_pane();
            let _pane2 = pane1.split(Direction::Vertical);
            pane1.select()?;
            let mut command = session.target("display-message")?;
            command.args(["-p", "#{pane_id}"]);
            let output = execute(command)?;
            assert!(pane1.target.contains(output.trim()));
            Ok(())
        }

        // Just checks for error. Testing this would be complicated
        #[test]
        fn run_command() -> Result<()> {
            let session = testing_session()?;
            let window = session.new_window(None, None)?;
            window.default_pane().run_command("echo test")?;
            Ok(())
        }
    }
}
