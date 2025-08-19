use super::*;
use crate::tmux::Target;
use crate::tmux::session::{Session, SessionBuilder};
use color_eyre::Result;
use std::sync::Arc;

pub const TESTING_SESSION: &str = "__celeris_testing";

impl Session {
    pub fn kill(&self) -> Result<()> {
        self.target().targeted_command("kill-session")?.execute()?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self
            .target()
            .target_exists()
            .expect("failed to check if there is a session to kill, cleanup failed")
        {
            self.kill()
                .expect("kill-session failed - environment after test is not cleaned up");
        }
    }
}

impl Session {
    pub fn detach_clients(&self) -> Result<()> {
        tmux()?
            .args(["detach-client", "-s", self.target().get()])
            .execute()?;
        Ok(())
    }
}

pub fn testing_session() -> Result<Arc<Session>> {
    Ok(SessionBuilder::new(TESTING_SESSION.to_owned()).build()?)
}

pub fn selected_pane_id(target: &str) -> Result<String> {
    Ok(tmux()?
        .args(["display-message", "-p", "-t", target, "#{pane_id}"])
        .execute()?
        .trim()
        .to_owned())
}

#[test]
fn server_running_t() -> Result<()> {
    let _session = testing_session();
    assert_eq!(server_running()?, true);
    Ok(())
}

const TARGET_TEST_STR: &str = "test";

#[test]
fn session_target() -> Result<()> {
    assert_eq!(
        SessionTarget::new(TARGET_TEST_STR).get(),
        &format!("{TARGET_TEST_STR}")
    );
    Ok(())
}

#[test]
fn window_target() -> Result<()> {
    let session_target = SessionTarget::new(TARGET_TEST_STR);
    let window_target = session_target.window_target(TARGET_TEST_STR);
    assert_eq!(
        window_target.get(),
        &format!("{TARGET_TEST_STR}:{TARGET_TEST_STR}")
    );
    Ok(())
}

#[test]
fn pane_target() -> Result<()> {
    let session_target = SessionTarget::new(TARGET_TEST_STR);
    let window_target = session_target.window_target(TARGET_TEST_STR);
    let pane_target = window_target.pane_target(TARGET_TEST_STR);
    assert_eq!(
        pane_target.get(),
        &format!("{TARGET_TEST_STR}:{TARGET_TEST_STR}.{TARGET_TEST_STR}")
    );
    Ok(())
}

#[test]
fn targeted_command() -> Result<()> {
    let session = testing_session()?;
    let output = session
        .target()
        .targeted_command("display-message")?
        .args(["-p", "testing"])
        .execute()?;
    assert_eq!(output.trim(), "testing");
    Ok(())
}

#[test]
fn target_exists() -> Result<()> {
    let session = testing_session()?;
    let exists = session.target().target_exists()?;
    assert_eq!(exists, true);

    let window = Window::builder(&session).build()?;
    let exists = window.target().target_exists()?;
    assert_eq!(exists, true);

    let new_pane = window.default_pane().split(Direction::Vertical).build()?;
    let pane1_exists = window.default_pane().target().target_exists()?;
    let pane2_exists = new_pane.target().target_exists()?;
    assert_eq!(pane1_exists, true);
    assert_eq!(pane2_exists, true);
    Ok(())
}

#[test]
fn execute() -> Result<()> {
    let _session = testing_session(); // startup tmux server
    let output = tmux()?.args(["display-message", "-p", "test"]).execute()?;
    assert_eq!(output.trim(), "test");

    let _ = tmux()?
        .args(["non-existent-command", "-p", "test"])
        .execute()
        .expect_err("execute should fail when tmux fails");
    Ok(())
}

#[test]
fn tmux_test() -> Result<()> {
    let _session = testing_session(); // startup tmux server
    let socket_name = "__celeris_tmux_testing";
    unsafe {
        env::set_var("CELERIS_TMUX_SOCKET_NAME", socket_name);
    }
    let command = tmux()?;
    assert_eq!(
        format!("{command:?}"),
        format!("{:?}", Command::new("tmux").args(["-L", socket_name])),
    );

    let socket_path = env::temp_dir().join(socket_name);
    unsafe {
        env::remove_var("CELERIS_TMUX_SOCKET_NAME");
        env::set_var("CELERIS_TMUX_SOCKET_PATH", &socket_path);
    }
    let command = tmux()?;
    assert_eq!(
        format!("{command:?}"),
        format!(
            "{:?}",
            Command::new("tmux").args(["-S", &socket_path.to_string_lossy()])
        ),
    );

    unsafe {
        env::remove_var("CELERIS_TMUX_SOCKET_PATH");
    }
    Ok(())
}
