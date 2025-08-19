use crate::tmux::{self, PaneTarget, Root, RootOptions, Target, TmuxExecuteExt};
use crate::utils;
use color_eyre::{Result, eyre::eyre};
use std::path::PathBuf;
use std::process::Command;

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

#[derive(PartialEq, Eq, Debug)]
struct SplitOptions {
    direction: Direction,
    root: Root,
    size: Option<SplitSize>,
}

#[derive(Debug)]
pub struct SplitBuilder {
    opts: SplitOptions,
    sibling_target: PaneTarget,
}

impl SplitBuilder {
    fn new(sibling_target: PaneTarget, direction: Direction) -> Self {
        let opts = SplitOptions {
            direction: direction,
            size: None,
            root: Root::default(),
        };
        Self {
            opts,
            sibling_target,
        }
    }

    pub fn size(self, size: SplitSize) -> Self {
        let opts = SplitOptions {
            size: Some(size),
            ..self.opts
        };

        Self { opts, ..self }
    }

    pub fn root(self, path: PathBuf) -> Result<Self> {
        let opts = SplitOptions {
            root: Root::custom(path)?,
            ..self.opts
        };

        Ok(Self { opts, ..self })
    }

    fn prepare_options(&self) -> Result<Vec<String>> {
        let mut options = Vec::new();
        self.prepare_size(&mut options)?;
        self.prepare_root(&mut options)?;
        Ok(options)
    }

    // requires tmxu 3.1 and up
    fn prepare_size(&self, options: &mut Vec<String>) -> Result<()> {
        let Some(size) = self.opts.size else {
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
        let root = match self.opts.root.as_ref() {
            RootOptions::Custom(path) => utils::path_to_string(path)?,
            RootOptions::Default => "#{pane_current_path}".to_owned(),
        };

        options.extend(["-c".to_owned(), root]);
        Ok(())
    }

    fn split_command(&self) -> Result<Command> {
        let mut command = self.sibling_target.targeted_command("split-window")?;
        command.args(["-P", "-F", "#{pane_id}"]);
        match self.opts.direction {
            Direction::Vertical => command.arg("-v"),
            Direction::Horizontal => command.arg("-h"),
        };

        command.args(self.prepare_options()?);
        Ok(command)
    }

    pub fn build(&self) -> Result<Pane> {
        let output = self.split_command()?.execute()?;
        let pane_id = output.trim();
        let target = PaneTarget::from_sibling(&self.sibling_target, pane_id);
        Ok(build_pane(target))
    }
}

impl tmux::BuilderTransform for SplitBuilder {}

impl Eq for SplitBuilder {}

impl PartialEq for SplitBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.opts == other.opts
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    target: PaneTarget,
}

pub fn build_pane(target: PaneTarget) -> Pane {
    Pane { target }
}

impl Pane {
    // No reasson to return arc here because it's owned which is fine with rhai
    pub fn split(&self, direction: Direction) -> SplitBuilder {
        SplitBuilder::new(self.target.clone(), direction)
    }

    pub fn select(&self) -> Result<()> {
        self.target().targeted_command("select-pane")?.execute()?;
        Ok(())
    }

    pub fn run_command(&self, command: &str) -> Result<()> {
        self.target()
            .targeted_command("send-keys")?
            .args([command, "Enter"])
            .execute()?;
        Ok(())
    }

    pub fn target(&self) -> &PaneTarget {
        &self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::Session;
    use crate::tmux::tests::TESTING_SESSION;
    use crate::tmux::{Target, Window, tests::testing_session};
    use std::env;
    use std::{thread, time::Duration};

    #[test]
    fn split() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane1 = window.default_pane();
        let pane2 = pane1.split(Direction::Vertical).build()?;

        assert_eq!(pane1.target().target_exists()?, true);
        assert_eq!(pane2.target().target_exists()?, true);

        let output = window.target().targeted_command("list-panes")?.execute()?;
        assert_eq!(output.lines().count(), 2);
        Ok(())
    }

    #[test]
    fn split_custom_path() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane = window
            .default_pane()
            .split(Direction::Vertical)
            .root(env::temp_dir())?
            .build()?;
        let output = pane
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_current_path}"])
            .execute()?;
        assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
        Ok(())
    }

    #[test]
    fn split_percentage_sized() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane = window
            .default_pane()
            .split(Direction::Horizontal)
            .size(SplitSize::Percentage(0))
            .build()?;

        let output = window
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{window_width}"])
            .execute()?;
        assert!(
            output.trim().parse::<usize>()? >= 1,
            "insufficent window size for testing"
        );

        let output = pane
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_width}"])
            .execute()?;

        assert_eq!(output.trim(), "1");

        let _ = window
            .default_pane()
            .split(Direction::Horizontal)
            .size(SplitSize::Percentage(101))
            .build()
            .unwrap_err();
        Ok(())
    }

    #[test]
    fn split_absolute_sized() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane = window
            .default_pane()
            .split(Direction::Horizontal)
            .size(SplitSize::Absolute(1))
            .build()?;

        let output = window
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{window_width}"])
            .execute()?;
        assert!(
            output.trim().parse::<usize>()? >= 1,
            "insufficent window size for testing"
        );

        let output = pane
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_width}"])
            .execute()?;

        assert_eq!(output.trim(), "1");
        Ok(())
    }

    #[test]
    fn root_inheritance() -> Result<()> {
        let root = env::temp_dir();
        let session = Session::builder(TESTING_SESSION.to_owned())
            .root(root.clone())?
            .build()?;
        let window = Window::builder(&session).build()?;
        let another_pane = window.default_pane().split(Direction::Vertical).build()?;
        let output = another_pane
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_current_path}"])
            .execute()?;
        assert_eq!(root.to_string_lossy(), output.trim());
        Ok(())
    }

    #[test]
    fn select() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane1 = window.default_pane();
        let _pane2 = pane1.split(Direction::Vertical).build();
        pane1.select()?;
        let output = session
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_id}"])
            .execute()?;
        assert!(pane1.target.get().contains(output.trim()));
        Ok(())
    }

    // Just checks for error. Testing this would be complicated
    #[test]
    fn run_command() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
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
            .target()
            .targeted_command("display-message")?
            .args(["-p", "#{pane_current_command}"])
            .execute()?;
        assert_eq!(output.trim(), real_command);
        Ok(())
    }
}
