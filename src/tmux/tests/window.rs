use super::*;

#[test]
fn new_window() -> Result<()> {
    let session = testing_session()?;

    let window = Window::builder(Arc::clone(&session)).build()?;
    assert!(
        Session::target_exists(&window.window_core.target)?,
        "window doesn't exist"
    );
    let output = session.target("list-windows")?.execute()?;
    let count = output.lines().count();
    assert_eq!(count, 1, "default session window hasn't been moved");
    Ok(())
}

#[test]
fn new_window_custom_path() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).root(env::temp_dir()).build()?;

    let output = window
        .window_core
        .target("display-message")?
        .args(["-p", "#{pane_current_path}"])
        .execute()?;

    assert_eq!(output.trim(), &utils::path_to_string(&env::temp_dir())?);
    Ok(())
}

#[test]
fn new_window_named() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).name("test".to_owned()).build()?;

    let output = window
        .window_core
        .target("display-message")?
        .args(["-p", "#{window_name}"])
        .execute()?;
    assert_eq!(output.trim(), "test");
    Ok(())
}

#[test]
fn set_option() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let option = ("allow-rename", "off");
    window.window_core.set_option(option.0, option.1)?;

    let output = window
        .window_core
        .target("show-window-options")?
        .execute()?;
    let option_got = output.lines().find(|line| line.contains(option.0));
    let option_got = option_got.ok_or_eyre("couldn't find option which was supposed to be set")?;
    let option_got = option_got.split_whitespace().collect::<Vec<_>>();
    assert_eq!(option_got.len(), 2);
    assert_eq!(option_got[0], option.0);
    assert_eq!(option_got[1], option.1);
    Ok(())
}

#[test]
fn select() -> Result<()> {
    let session = testing_session()?;
    let window1 = Window::builder(Arc::clone(&session)).build()?;
    let _window2 = Window::builder(Arc::clone(&session)).build()?;
    window1.select()?;
    let output = session
        .target("display-message")?
        .args(["-p", "#{window_id}"])
        .execute()?;
    assert!(window1.window_core.target.contains(output.trim()));
    Ok(())
}

// Kind of unable to test this so this just checks if there was an error
// even if testing this is possible there is just no point because most of the logic is the
// burden of tmux
#[test]
fn even_out() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    window.event_out(Direction::Horizontal)?;
    window.event_out(Direction::Vertical)?;
    Ok(())
}

#[test]
fn default_pane() -> Result<()> {
    let session = testing_session()?;
    let window = Window::builder(session).build()?;
    let pane = window.default_pane();
    assert_eq!(Session::target_exists(&pane.target)?, true);
    Ok(())
}
