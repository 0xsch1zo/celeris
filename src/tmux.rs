use color_eyre::section::PanicMessage;
use color_eyre::{Result, eyre::Context, eyre::eyre};
use std::cell::{Ref, RefCell};
use std::process;
use std::process::Stdio;
use std::rc::Rc;
use std::str;
use std::thread::current;

// Maybe leave this thing procedural and make a higher level object oriented abstraction

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
    //let target = format!("{}:{}.{}", self.session_id, window_id, pane_id);
    let has_session_status = tmux()
        .args(["has-session", "-t", &target])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .wrap_err_with(|| "has-session failed to execute")?;
    if !has_session_status.success() {
        return Err(eyre!("target: {target}, doesn't exist"));
    }
    let mut tmux = tmux();
    tmux.args([command, "-t", &target]);
    Ok(tmux)
}

pub enum Direction {
    Horizontal,
    Vertical,
}

pub enum SplitSize {
    Percentage(u8),
    Fixed(u32),
}

// TODO: split into window and session components
pub struct Session<'a> {
    session_id: String,
    windows: RefCell<Vec<Window<'a>>>,
}

impl<'a> Session<'a> {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id: session_id,
            windows: RefCell::new(Vec::new()),
        }
    }

    // TODO: Maybe replace with an enum
    fn target(&self, command: &str) -> Result<process::Command> {
        let target = format!("{}:", self.session_id);
        tmux_target_command(&target, command)
    }

    pub fn windows(&self) -> Ref<Vec<Window<'a>>> {
        self.windows.borrow()
    }

    pub fn new_window(&'a self, name: Option<&str>, shell_command: Option<&str>) -> Result<()> {
        self.windows
            .borrow_mut()
            .push(Window::new(&self, name, shell_command)?);
        Ok(())
    }
}

pub struct Window<'a> {
    session: &'a Session<'a>,
    window_id: String,
    panes: RefCell<Vec<Pane<'a>>>,
}

impl<'a> Window<'a> {
    // Overloads will be set while initializing the rhai engine
    // Can't do builder pattern because the command needs to be executed at the end of
    // construction. To have the caller call a fininalizing function would be to much responsibility to the caller
    fn new(
        session: &'a Session<'a>,
        name: Option<&str>,
        shell_command: Option<&str>,
    ) -> Result<Self> {
        let mut command = session.target("new-window")?;
        command.args(["-P", "-F", "#{window_index}"]);

        // TODO: disable name changes for named windows
        if let Some(name) = name {
            command.args(["-n", name]);
        }
        if let Some(shell_command) = shell_command {
            command.arg(shell_command);
        }

        let output = execute(command)?;
        let id: u32 = output
            .trim()
            .parse()
            .wrap_err("faield to parse tmux window id")?;

        if let Some(_) = name {
            let window = Self {
                session: session,
                //window_id: name.to_string(),
                window_id: id.to_string(),
                panes: RefCell::new(Vec::new()),
            };
            window.set_option("allow-rename", "off")?;
            return Ok(window);
        }

        Ok(Self {
            session: &session,
            window_id: id.to_string(),
            panes: RefCell::new(Vec::new()),
        })
    }

    fn target(&self, command: &str) -> Result<process::Command> {
        let target = format!("{}:{}", self.session.session_id, self.window_id);
        tmux_target_command(&target, command)
    }

    fn set_option(&self, option: &str, value: &str) -> Result<()> {
        let mut command = self.target("set-window-option")?;
        command.args([option, value]);
        execute(command)?;
        Ok(())
    }

    pub fn select(&self) -> Result<()> {
        execute(self.target("select-window")?)?;
        Ok(())
    }

    pub fn even_out(&self, direction: Direction) -> Result<()> {
        let mut command = self.target("select-layout")?;
        match direction {
            Direction::Vertical => command.arg("even-vertical"),
            Direction::Horizontal => command.arg("even-horizontal"),
        };
        execute(command)?;
        Ok(())
    }

    fn new_pane(&self, pane: Pane<'a>) {
        self.panes.borrow_mut().push(pane);
    }

    pub fn panes(&self) -> Ref<Vec<Pane<'a>>> {
        self.panes.borrow()
    }
}

pub struct Pane<'a> {
    pane_id: String,
    window: &'a Window<'a>,
}

impl<'a> Pane<'a> {
    fn new(window: &'a Window<'a>, pane_id: &str) -> Self {
        Self {
            pane_id: pane_id.to_string(),
            window,
        }
    }

    fn target(&self, command: &str) -> Result<process::Command> {
        let target = format!(
            "{}:{}.{}",
            self.window.session.session_id, self.window.window_id, self.pane_id
        );
        tmux_target_command(&target, command)
    }

    fn split_command(&self, split: Direction) -> Result<process::Command> {
        let mut command = self.target("split-window")?;
        command.args(["-P", "-F", "#{pane_index}"]);
        match split {
            Direction::Vertical => command.arg("-v"),
            Direction::Horizontal => command.arg("-h"),
        };

        Ok(command)
    }

    pub fn split(&self, split: Direction) -> Result<()> {
        let output = execute(self.split_command(split)?)?;
        let id: u32 = output.trim().parse()?;
        self.window
            .new_pane(Pane::new(self.window, &id.to_string()));
        Ok(())
    }

    // TODO: maybe add support for below 3.1
    pub fn split_with_size(&self, split: Direction, size: SplitSize) -> Result<()> {
        let mut command = self.split_command(split)?;
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
        let id: u32 = output.trim().parse()?;
        self.window
            .new_pane(Pane::new(self.window, &id.to_string()));

        Ok(())
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
