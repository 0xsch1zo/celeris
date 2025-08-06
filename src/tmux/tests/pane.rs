use super::*;
use std::{thread, time::Duration};

#[test]
fn split() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let pane1 = window.default_pane();
    let pane2 = pane1.split_builder(Direction::Vertical).build()?;

    assert_eq!(Session::target_exists(&pane1.target)?, true);
    assert_eq!(Session::target_exists(&pane2.target)?, true);

    let output = window.window_core.target("list-panes")?.execute()?;
    assert_eq!(output.lines().count(), 2);
    Ok(())
}

#[test]
fn split_custom_path() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let pane = window
        .default_pane
        .split_builder(Direction::Vertical)
        .root(env::temp_dir())
        .build()?;
    let output = pane
        .target("display-message")?
        .args(["-p", "#{pane_current_path}"])
        .execute()?;
    assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
    Ok(())
}

#[test]
fn split_percentage_sized() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let pane = window
        .default_pane()
        .split_builder(Direction::Horizontal)
        .size(SplitSize::Percentage(0))
        .build()?;

    let output = window
        .window_core
        .target("display-message")?
        .args(["-p", "#{window_width}"])
        .execute()?;
    assert!(
        output.trim().parse::<usize>()? >= 1,
        "insufficent window size for testing"
    );

    let output = pane
        .target("display-message")?
        .args(["-p", "#{pane_width}"])
        .execute()?;

    assert_eq!(output.trim(), "1");

    let _ = window
        .default_pane()
        .split_builder(Direction::Horizontal)
        .size(SplitSize::Percentage(101))
        .build()
        .unwrap_err();
    Ok(())
}

#[test]
fn split_absolute_sized() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let pane = window
        .default_pane()
        .split_builder(Direction::Horizontal)
        .size(SplitSize::Absolute(1))
        .build()?;

    let output = window
        .window_core
        .target("display-message")?
        .args(["-p", "#{window_width}"])
        .execute()?;
    assert!(
        output.trim().parse::<usize>()? >= 1,
        "insufficent window size for testing"
    );

    let output = pane
        .target("display-message")?
        .args(["-p", "#{pane_width}"])
        .execute()?;

    assert_eq!(output.trim(), "1");
    Ok(())
}

#[test]
fn select() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(Arc::clone(&session)).build()?;
    let pane1 = window.default_pane();
    let _pane2 = pane1.split_builder(Direction::Vertical).build();
    pane1.select()?;
    let output = session
        .target("display-message")?
        .args(["-p", "#{pane_id}"])
        .execute()?;
    assert!(pane1.target.contains(output.trim()));
    Ok(())
}

// Just checks for error. Testing this would be complicated
#[test]
fn run_command() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
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
        .target("display-message")?
        .args(["-p", "#{pane_current_command}"])
        .execute()?;
    assert_eq!(output.trim(), real_command);
    Ok(())
}
