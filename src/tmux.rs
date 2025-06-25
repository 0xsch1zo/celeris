use color_eyre::{Result, eyre::Context, eyre::eyre};
use std::process;
use std::process::Stdio;
use std::str;
use std::thread::current;

pub enum Direction {
    Horizontal,
    Vertical,
}

pub enum SplitSize {
    Percentage(u8),
    Fixed(u32),
}

// TODO: split into window and session components
pub struct TmuxSession {
    session_name: String,
}

impl TmuxSession {
    pub fn new(session_name: String) -> Self {
        Self {
            session_name: session_name,
        }
    }

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

    // TODO: Maybe replace with an enum
    fn sesssion_target(&self, command: &str) -> Result<process::Command> {
        let target = format!("{}:", self.session_name);
        let has_session_status = Self::tmux()
            .args(["has-session", "-t", &target])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .wrap_err_with(|| "has-session failed to execute")?;
        if !has_session_status.success() {
            return Err(eyre!(
                "session with name: {}, doesn't exist",
                self.session_name
            ));
        }
        let mut tmux = Self::tmux();
        tmux.args([command, "-t", &target]);
        Ok(tmux)
    }

    fn window_target(&self, window_id: &str, command: &str) -> Result<process::Command> {
        let target = format!("{}:{}", self.session_name, window_id);
        let has_session_status = Self::tmux()
            .args(["has-session", "-t", &target])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .wrap_err_with(|| "has-session failed to execute")?;
        if !has_session_status.success() {
            return Err(eyre!("target: {target}, doesn't exist"));
        }
        let mut tmux = Self::tmux();
        tmux.args([command, "-t", &target]);
        Ok(tmux)
    }

    fn pane_target(
        &self,
        window_id: &str,
        pane_id: &str,
        command: &str,
    ) -> Result<process::Command> {
        let target = format!("{}:{}.{}", self.session_name, window_id, pane_id);
        let has_session_status = Self::tmux()
            .args(["has-session", "-t", &target])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .wrap_err_with(|| "has-session failed to execute")?;
        if !has_session_status.success() {
            return Err(eyre!("target: {target}, doesn't exist"));
        }
        let mut tmux = Self::tmux();
        tmux.args([command, "-t", &target]);
        Ok(tmux)
    }

    fn set_window_option(&self, window_id: &str, option: &str, value: &str) -> Result<()> {
        let mut command = self.window_target(window_id, "set-window-option")?;
        command.args([option, value]);
        Self::execute(command)?;
        Ok(())
    }

    fn current_window(&self) -> Result<u32> {
        let mut command = self.sesssion_target("display-message")?;
        command.arg("-p").arg("#I");
        let output = Self::execute(command)?;
        Ok(output.trim().parse()?)
    }

    fn split_window_command(&self, window_id: &str, split: Direction) -> Result<process::Command> {
        let mut command = self.window_target(window_id, "split-window")?;
        match split {
            Direction::Vertical => command.arg("-v"),
            Direction::Horizontal => command.arg("-h"),
        };

        Ok(command)
    }

    // Overloads will be set while initializing the rhai engine
    pub fn new_window(&self, name: Option<&str>, shell_command: Option<&str>) -> Result<u32> {
        let mut command = self.sesssion_target("new-window")?;

        // TODO: disable name changes for named windows
        if let Some(name) = name {
            command.args(["-n", name]);
        }
        if let Some(shell_command) = shell_command {
            command.arg(shell_command);
        }

        let _output = Self::execute(command)?;

        if let Some(name) = name {
            self.set_window_option(name, "allow-rename", "off")?;
        }

        Ok(self.current_window()?)
    }

    pub fn split_window(&self, window_id: &str, split: Direction) -> Result<()> {
        Self::execute(self.split_window_command(window_id, split)?)?;
        Ok(())
    }

    // TODO: maybe add support for below 3.1
    pub fn split_window_size(
        &self,
        window_id: &str,
        split: Direction,
        size: SplitSize,
    ) -> Result<()> {
        let mut command = self.split_window_command(window_id, split)?;
        match size {
            SplitSize::Percentage(percentage) if percentage <= 100 => {
                command.args(["-l", &format!("{percentage}%")])
            }
            SplitSize::Percentage(percentage) => {
                return Err(eyre!("Percentage amount above 100: {percentage}"));
            }
            SplitSize::Fixed(fixed) => command.args(["-l", &fixed.to_string()]),
        };
        Self::execute(command)?;
        Ok(())
    }

    pub fn select_window(&self, window_id: &str) -> Result<()> {
        Self::execute(self.window_target(window_id, "select-window")?)?;
        Ok(())
    }

    pub fn select_pane(&self, window_id: &str, pane_id: &str) -> Result<()> {
        Self::execute(self.pane_target(window_id, pane_id, "select-pane")?)?;
        Ok(())
    }

    pub fn run_command(&self, window_id: &str, pane_id: &str, command: &str) -> Result<()> {
        let mut send_keys = self.pane_target(window_id, pane_id, "send-keys")?;
        send_keys.args([command, "Enter"]);

        Self::execute(send_keys)?;
        Ok(())
    }

    pub fn even_out(&self, window_id: &str, direction: Direction) -> Result<()> {
        let mut command = self.window_target(window_id, "select-layout")?;
        match direction {
            Direction::Vertical => command.arg("even-vertical"),
            Direction::Horizontal => command.arg("even-horizontal"),
        };
        Self::execute(command)?;
        Ok(())
    }
}
