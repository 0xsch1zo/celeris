use super::*;
use crate::tmux::{Session, TmuxExecuteExt};
use color_eyre::Result;

#[test]
fn from() -> Result<()> {
    let session = super::testing_session()?;
    let session_from = Session::from(&session.session_id)?;
    let output = session_from
        .target("display-message")?
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
    let window_target = format!("{}:{}", session.session_id, session.default_window_id);
    let targets = vec![
        format!("{}:", session.session_id),
        selected_pane_id(&window_target)?,
        window_target.clone(),
        "2137:".to_owned(),
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
        Session::target_exists(&session.session_id)?,
        "session doesn't exist"
    );
    Ok(())
}

#[test]
fn new_session_custom_root() -> Result<()> {
    let session = SessionBuilder::new(TESTING_SESSION.to_owned())
        .root(env::temp_dir())?
        .build()?;
    let output = session
        .target("display-message")?
        .args(["-p", "#{pane_current_path}"])
        .execute()?;
    assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
    Ok(())
}

fn attach_test(attached: TerminalState) -> Result<()> {
    let session = testing_session()?;
    let (command, handle) = session.spawn_attach(attached.clone())?;
    let output = session
        .target("display-message")?
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
