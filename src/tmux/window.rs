use crate::tmux::{
    self, PaneTarget, Root, RootOptions, Target, TmuxExecuteExt, WindowTarget,
    pane::{self, Direction, Pane},
    session::{self, Session},
};
use crate::utils;
use color_eyre::{Result, eyre::OptionExt};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
struct WindowOptions {
    name: Option<String>,
    shell_command: Option<String>,
    root: Root,
}

#[derive(Debug)]
pub struct WindowBuilder {
    opts: WindowOptions,
    session: Arc<Session>,
}

impl WindowBuilder {
    pub fn new(session: Arc<Session>) -> Self {
        let opts = WindowOptions {
            name: None,
            shell_command: None,
            root: Root::default(),
        };

        Self { opts, session }
    }

    pub fn name(self, name: String) -> Self {
        let opts = WindowOptions {
            name: Some(name),
            ..self.opts
        };
        Self { opts, ..self }
    }

    pub fn root(self, path: PathBuf) -> Result<Self> {
        let opts = WindowOptions {
            root: Root::custom(path)?,
            ..self.opts
        };
        Ok(Self { opts, ..self })
    }

    pub fn raw_command(self, command: String) -> Self {
        let opts = WindowOptions {
            shell_command: Some(command),
            ..self.opts
        };
        Self { opts, ..self }
    }

    fn prepare_options(&self) -> Result<Vec<String>> {
        let mut options: Vec<String> = Vec::new();
        self.prepare_name(&mut options);
        self.prepare_root(&mut options)?;
        self.prepare_raw_command(&mut options);

        Ok(options)
    }

    fn prepare_name(&self, options: &mut Vec<String>) {
        let Some(name) = &self.opts.name else {
            return;
        };

        options.extend(["-n".to_owned(), name.to_owned()]);
    }

    fn prepare_raw_command(&self, options: &mut Vec<String>) {
        let Some(command) = &self.opts.shell_command else {
            return;
        };
        options.push(command.to_owned());
    }

    fn prepare_root(&self, options: &mut Vec<String>) -> Result<()> {
        let root = match self.opts.root.as_ref() {
            RootOptions::Custom(path) => utils::path_to_string(path)?,
            RootOptions::Default => "#{pane_current_path}".to_owned(), // should inherit context
                                                                       // from our session not env
        };
        options.extend(["-c".to_owned(), root]);
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

    pub fn build(self) -> Result<Window> {
        let window_core = self.create_window()?;
        session::register_window(&self.session, &window_core)?;

        if let Some(_) = self.opts.name {
            window_core.set_option("allow-rename", "off")?;
        }
        Ok(Window::new(window_core))
    }
}

impl tmux::BuilderTransform for WindowBuilder {}

impl PartialEq for WindowBuilder {
    fn eq(&self, other: &Self) -> bool {
        self.opts == other.opts
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
    fn new(window_core: WindowCore) -> Self {
        Self {
            default_pane: Arc::new(pane::build_pane(window_core.default_pane_target.clone())),
            window_core,
        }
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
    use std::{env, thread, time::Duration};
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
        let window = Window::builder(&session).root(env::temp_dir())?.build()?;

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
    fn new_window_command() -> Result<()> {
        let session = testing_session()?;

        let real_command = "cat";
        let command = format!("'{real_command}'"); // to ignore aliases
        //should run until Ctrl+C or the session is killled. Will work
        // only on most systems. Testing this without getting execution
        // is probably impossible
        let window = Window::builder(&session).raw_command(command).build()?;
        // Yes the shell is sometimes this slow
        thread::sleep(Duration::from_secs(1));
        let output = tmux::targeted_command(window.target(), "display-message")?
            .args(["-p", "#{pane_current_command}"])
            .execute()?;
        assert_eq!(output.trim(), real_command);
        Ok(())
    }

    #[test]
    fn new_window_command_mixed() -> Result<()> {
        let session = testing_session()?;

        let real_command = "cat";
        let command = format!("'{real_command}'"); // to ignore aliases
        let window = Window::builder(&session)
            .name("testt".to_owned())
            .root(env::temp_dir())?
            .raw_command(command)
            .build()?;
        thread::sleep(Duration::from_secs(1));
        let output = tmux::targeted_command(window.target(), "display-message")?
            .args(["-p", "#{pane_current_command}"])
            .execute()?;
        assert_eq!(output.trim(), real_command);
        Ok(())
    }

    #[test]
    fn root_inheritance() -> Result<()> {
        let root = env::temp_dir();
        let session = Session::builder(TESTING_SESSION.to_owned())
            .root(root.clone())?
            .build()?;
        let window = Window::builder(&session).build()?;
        let output = tmux::targeted_command(window.target(), "display-message")?
            .args(["-p", "#{pane_current_path}"])
            .execute()?;
        assert_eq!(root.to_string_lossy(), output.trim());
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
