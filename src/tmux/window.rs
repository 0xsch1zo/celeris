use crate::tmux::{
    self, PaneTarget, Root, Target, TmuxExecuteExt, WindowTarget,
    pane::{self, Direction, Pane},
    session::{self, Session},
};
use crate::utils;
use color_eyre::{Result, eyre::OptionExt};
use std::path::PathBuf;
use std::sync::Arc;

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
            session,
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
        let output = tmux::targeted_command(self.session.target(), "new-window")?
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

        let target = self.session.target().window_target(window_id);
        let default_pane_target = target.pane_target(default_pane_id);
        Ok(WindowCore::new(target, default_pane_target))
    }

    pub fn build(&mut self) -> Result<Arc<Window>> {
        let window_core = self.create_window()?;
        session::register_window(&self.session, &window_core)?;

        if let Some(_) = self.name {
            window_core.set_option("allow-rename", "off")?;
        }
        Ok(Window::new(window_core))
    }
}

#[derive(Clone, Debug)]
pub struct WindowCore {
    target: WindowTarget,
    default_pane_target: PaneTarget,
}

impl WindowCore {
    fn new(target: WindowTarget, default_pane_target: PaneTarget) -> Self {
        Self {
            default_pane_target,
            target,
        }
    }

    fn set_option(&self, option: &str, value: &str) -> Result<()> {
        tmux::targeted_command(&self.target, "set-window-option")?
            .args([option, value])
            .execute()?;
        Ok(())
    }

    fn select(&self) -> Result<()> {
        tmux::targeted_command(&self.target, "select-window")?.execute()?;
        Ok(())
    }

    fn even_out(&self, direction: Direction) -> Result<()> {
        let mut command = tmux::targeted_command(&self.target, "select-layout")?;
        match direction {
            Direction::Vertical => command.arg("even-vertical"),
            Direction::Horizontal => command.arg("even-horizontal"),
        };
        command.execute()?;
        Ok(())
    }

    // Only for the purpose of killing the default window
    pub fn move_kill(&self, other: &WindowTarget) -> Result<()> {
        // use a proper source target
        // wtf
        tmux::targeted_command(&self.target, "move-window")?
            .args(["-s", self.target.get(), "-t", other.get(), "-k"])
            .execute()?;
        Ok(())
    }
}

// all this is because I have a skill issue and in the architecture there is an inherent dependency
// cycle between the default pane and window. Couldn't think of a way to have a clear api without
// this
#[derive(Clone, Debug)]
pub struct Window {
    window_core: WindowCore,
    default_pane: Arc<Pane>,
}

impl Window {
    fn new(window_core: WindowCore) -> Arc<Self> {
        Arc::new(Self {
            default_pane: Arc::new(pane::build_pane(window_core.default_pane_target.clone())),
            window_core,
        })
    }

    pub fn builder(session: &Arc<Session>) -> WindowBuilder {
        WindowBuilder::new(Arc::clone(session))
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

    #[allow(private_interfaces)]
    pub fn target(&self) -> &WindowTarget {
        &self.window_core.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::pane::Direction;
    use color_eyre::eyre::OptionExt;
    use std::env;
    use tmux::tests::*;

    #[test]
    fn new_window() -> Result<()> {
        let session = testing_session()?;

        let window = Window::builder(&session).build()?;
        assert!(
            tmux::target_exists(&window.window_core.target)?,
            "window doesn't exist"
        );
        let output = tmux::targeted_command(session.target(), "list-windows")?.execute()?;
        let count = output.lines().count();
        assert_eq!(count, 1, "default session window hasn't been moved");
        Ok(())
    }

    #[test]
    fn new_window_custom_path() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).root(env::temp_dir()).build()?;

        let output = tmux::targeted_command(&window.window_core.target, "display-message")?
            .args(["-p", "#{pane_current_path}"])
            .execute()?;

        assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
        Ok(())
    }

    #[test]
    fn new_window_named() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).name("test".to_owned()).build()?;

        let output = tmux::targeted_command(&window.window_core.target, "display-message")?
            .args(["-p", "#{window_name}"])
            .execute()?;
        assert_eq!(output.trim(), "test");
        Ok(())
    }

    #[test]
    fn set_option() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let option = ("allow-rename", "off");
        window.window_core.set_option(option.0, option.1)?;

        let output =
            tmux::targeted_command(&window.window_core.target, "show-window-options")?.execute()?;
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
        let window1 = Window::builder(&session).build()?;
        let _window2 = Window::builder(&session).build()?;
        window1.select()?;
        let output = tmux::targeted_command(session.target(), "display-message")?
            .args(["-p", "#{window_id}"])
            .execute()?;
        assert!(window1.window_core.target.get().contains(output.trim()));
        Ok(())
    }

    // Kind of unable to test this so this just checks if there was an error
    // even if testing this is possible there is just no point because most of the logic is the
    // burden of tmux
    #[test]
    fn even_out() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        window.event_out(Direction::Horizontal)?;
        window.event_out(Direction::Vertical)?;
        Ok(())
    }

    #[test]
    fn default_pane() -> Result<()> {
        let session = testing_session()?;
        let window = Window::builder(&session).build()?;
        let pane = window.default_pane();
        assert_eq!(tmux::target_exists(pane.target())?, true);
        Ok(())
    }
}
