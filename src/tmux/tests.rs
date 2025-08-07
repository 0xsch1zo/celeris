use super::*;
use crate::tmux;
use crate::tmux::session::{Session, SessionBuilder};
use std::sync::Arc;
pub const TESTING_SESSION: &str = "__sesh_testing";

impl Session {
    pub fn kill(&self) -> Result<()> {
        tmux::targeted_command(self.target(), "kill-session")?.execute()?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if tmux::target_exists(self.target())
            .expect("failed to check if there is a session to kill, cleanup failed")
        {
            self.kill()
                .expect("kill-session failed - environment after test is not cleaned up");
        }
    }
}

impl Session {
    pub fn detach_clients(&self) -> Result<()> {
        tmux()
            .args(["detach-client", "-s", self.target().get()])
            .execute()?;
        Ok(())
    }
}

pub fn testing_session() -> Result<Arc<Session>> {
    Ok(SessionBuilder::new(TESTING_SESSION.to_owned()).build()?)
}

pub fn selected_pane_id(target: &str) -> Result<String> {
    Ok(tmux()
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
