mod pane;
mod session;
mod window;

use super::*;
const TESTING_SESSION: &str = "__sesh_testing";

impl Session {
    fn kill(&self) -> Result<()> {
        self.target("kill-session")?.execute()?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if Session::target_exists(&self.session_id)
            .expect("failed to check if there is a session to kill, cleanup failed")
        {
            self.kill()
                .expect("kill-session failed - environment after test is not cleaned up");
        }
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

fn testing_session() -> Result<Arc<Session>> {
    Ok(SessionBuilder::new(TESTING_SESSION.to_owned()).build()?)
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
