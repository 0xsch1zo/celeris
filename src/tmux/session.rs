use crate::tmux::{
    self, Root, SessionTarget, TerminalState, TmuxExecuteExt, WindowTarget, tmux,
    window::WindowCore,
};
use crate::utils;
use color_eyre::eyre::ContextCompat;
use color_eyre::{
    Result,
    eyre::{OptionExt, WrapErr, eyre},
};
use itertools::Itertools;
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub struct SessionBuilder {
    root: Root,
    session_name: String,
}

impl SessionBuilder {
    const OUTPUT_DELIM: &str = "|";
    pub fn new(session_name: String) -> Self {
        Self {
            root: Root::Default,
            session_name,
        }
    }

    pub fn root(&mut self, path: PathBuf) -> Result<&mut Self> {
        if !path.exists() {
            return Err(eyre!(
                "session: {}: root doesn't exist: {path:?}",
                self.session_name
            ));
        }

        self.root = Root::Custom(path);
        Ok(self)
    }

    fn prepare(&self) -> Result<Command> {
        let mut command = tmux();
        // need to use low level api
        command.args([
            "new-session",
            "-d",
            "-s",
            &self.session_name,
            "-P",
            "-F",
            &format!(
                "{}{}{}",
                "#{window_id}",
                Self::OUTPUT_DELIM,
                "#{session_id}"
            ),
        ]);

        self.prepare_root(&mut command)?;
        Ok(command)
    }

    fn prepare_root(&self, command: &mut Command) -> Result<()> {
        if let Root::Custom(root) = &self.root {
            command.args(["-c", &utils::path_to_string(root)?]);
        }
        Ok(())
    }

    pub fn build(&mut self) -> Result<Arc<Session>> {
        if tmux::target_exists(&SessionTarget::new(&self.session_name))? {
            return Err(eyre!(
                "session with name: {}, already exists",
                self.session_name
            ));
        }

        let mut command = self.prepare()?;
        let output = command.execute()?;
        let (default_window_id, session_id) = output
            .trim()
            .split_once(Self::OUTPUT_DELIM)
            .ok_or_eyre(format!(
                "failed to create session, couldn't parse session or window id: {}",
                output
            ))?;
        let session_target = SessionTarget::new(session_id);
        let default_window_target = session_target.window_target(default_window_id);
        Ok(Session::new(session_target, default_window_target))
    }
}

impl tmux::BuilderTransform for SessionBuilder {}

#[derive(Debug)]
pub struct Session {
    window_count: Mutex<usize>,
    target: SessionTarget,
    default_window_target: WindowTarget,
}

impl Session {
    // Can't run this if in tmux session already
    fn new(target: SessionTarget, default_window_target: WindowTarget) -> Arc<Self> {
        Arc::new(Self {
            window_count: Mutex::new(0),
            target,
            default_window_target,
        })
    }

    pub fn from(session_identifier: &str) -> Result<Arc<Session>> {
        if !tmux::target_exists(&SessionTarget::new(session_identifier))? {
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

        let target = SessionTarget::new(session_id);
        let default_window_target = target.window_target(default_window_id);
        Ok(Arc::new(Self {
            window_count: window_count.into(),
            target,
            default_window_target,
        }))
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
        if !tmux::server_running()? {
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
        if !tmux::server_running()? {
            return Ok(Vec::new());
        }
        let output = tmux()
            .args(["list-sessions", "-F", "#{session_name}"])
            .execute()?;
        Ok(output.trim().lines().map(ToOwned::to_owned).collect())
    }

    fn spawn_attach(&self, attached: TerminalState) -> Result<(Command, Child)> {
        let mut command = match attached {
            TerminalState::InTmux => tmux::targeted_command(&self.target, "switch-client")?,
            TerminalState::Normal => tmux::targeted_command(&self.target, "attach-session")?,
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

    #[allow(private_interfaces)]
    pub fn target(&self) -> &SessionTarget {
        &self.target
    }
}

pub fn register_window(session: &Session, window: &WindowCore) -> Result<()> {
    let mut count = session.window_count.lock().unwrap();
    if *count == 0 {
        window.move_kill(&session.default_window_target)?;
    }
    *count += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::session::{Session, TmuxExecuteExt};
    use crate::tmux::tests::*;
    use color_eyre::Result;

    #[test]
    fn from() -> Result<()> {
        let session = testing_session()?;
        let session_from = Session::from(&session.target.session_id)?;
        let output = tmux::targeted_command(&session_from.target, "display-message")?
            .args(["-p", "test"])
            .execute()?;
        assert_eq!(output.trim(), "test");
        session.kill()?;
        Ok(())
    }

    #[test]
    fn target_exits() -> Result<()> {
        let session = testing_session()?;
        // also used for pane
        let window_target = format!(
            "{}:{}",
            session.target.session_id, session.default_window_target.window_id
        );
        let targets = vec![
            format!("{}:", session.target.session_id),
            selected_pane_id(&window_target)?,
            window_target.clone(),
            "2137:".to_owned(),
            format!("{}:2137", session.target.session_id),
            format!(
                "{}:{}.2137",
                session.target.session_id, session.default_window_target.window_id
            ),
        ];

        targets.into_iter().try_for_each(|target| -> Result<()> {
            let exists = tmux::target_exists(&SessionTarget::new(&target))?;
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
        let _session1 = SessionBuilder::new(session_name_1.to_owned()).build()?; // to stop the session
        // from being dropped
        let _session2 = SessionBuilder::new(session_name_2.to_lowercase()).build()?;
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
            tmux::target_exists(&session.target)?,
            "session doesn't exist"
        );
        Ok(())
    }

    #[test]
    fn new_session_custom_root() -> Result<()> {
        let session = SessionBuilder::new(TESTING_SESSION.to_owned())
            .root(env::temp_dir())?
            .build()?;
        let output = tmux::targeted_command(&session.target, "display-message")?
            .args(["-p", "#{pane_current_path}"])
            .execute()?;
        assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
        Ok(())
    }

    fn attach_test(attached: TerminalState) -> Result<()> {
        let session = testing_session()?;
        let (command, handle) = session.spawn_attach(attached.clone())?;
        let output = tmux::targeted_command(&session.target, "display-message")?
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
