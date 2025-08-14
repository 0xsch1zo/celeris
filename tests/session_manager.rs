#[allow(dead_code)]
mod common;

use color_eyre::eyre::eyre;
use color_eyre::{Result, eyre::Context};
use common::TestDirectoryManager;
use handlebars::Handlebars;
use rust_embed::Embed;
use serde::Serialize;
use sesh::session_manager::ListSessionsOptions;
use sesh::session_manager::SwitchTarget;
use sesh::tmux::Session;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn list_sessions() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let dummy_layouts = ["test1", "test2", "test3"];
    common::create_dummy_layouts(&dummy_layouts, dir_mgr.as_ref())?;
    let session_manager = common::test_session_manager(Arc::clone(dir_mgr.inner()))?;

    let opts = ListSessionsOptions {
        tmux_format: false,
        include_active: false,
        exclude_running: true,
    };
    let output = session_manager.list(opts)?;
    output
        .lines()
        .map(str::trim)
        .zip(dummy_layouts)
        .for_each(|(output, session)| assert_eq!(output, session));
    Ok(())
}

#[test]
fn remove_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    common::create_dummy_layouts(&["test"], dir_mgr.as_ref())?;
    let mut session_manager = common::test_session_manager(Arc::clone(dir_mgr.inner()))?;
    let layout_path = dir_mgr
        .as_ref()
        .layouts_dir()?
        .join("test")
        .with_extension("lua");

    println!(
        "{}",
        session_manager.list(ListSessionsOptions {
            tmux_format: false,
            include_active: false,
            exclude_running: true
        })?
    );
    assert!(layout_path.exists());
    session_manager.remove("test")?;
    assert!(!layout_path.exists());

    Ok(())
}

#[test]
fn new_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let session_manager = Mutex::new(common::test_session_manager(Arc::clone(dir_mgr.inner()))?);
    let layout_path = dir_mgr.layouts_dir()?.join("test");
    File::create_new(&layout_path).wrap_err("failed to create layout's target")?;
    let layout_path_c = layout_path.clone();
    let (err_tx, err_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = session_manager.lock().unwrap().create(None, layout_path);
        err_tx.send(result).unwrap();
    });

    let start = Instant::now();
    let wait_time = Duration::from_millis(300);
    while start.elapsed() < wait_time {
        if layout_path_c.exists() {
            return Ok(());
        }

        let result = err_rx.try_recv();
        if result.is_ok() {
            return Err(result
                .unwrap()
                .expect_err("non error value sent through channel"));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(eyre!("layout file hasn't been created after {wait_time:?}"))
}

#[derive(Embed)]
#[folder = "templates/tests/"]
#[include = "*.template.lua"]
struct TestFiles;

#[derive(Serialize)]
struct TestData {
    session_root: PathBuf,
}

#[test]
fn basic_switch() -> Result<()> {
    unsafe {
        env::set_var("SESH_TMUX_SOCKET_NAME", "__sesh_testing");
    }
    let dir_mgr = TestDirectoryManager::new()?;
    let mut handlebars = Handlebars::new();
    handlebars.register_embed_templates_with_extension::<TestFiles>(".template.lua")?;
    let test_data = TestData {
        session_root: env::temp_dir(),
    };
    let layout_str = handlebars.render("session_with_root", &test_data)?;
    common::new_layout("session_with_root", &layout_str, dir_mgr.as_ref())?;
    let session_manager = common::test_session_manager(Arc::clone(dir_mgr.inner()))?;
    session_manager.switch(SwitchTarget::Session("session_with_root".to_owned()))?;
    assert_eq!(
        Session::list_sessions()?.contains(&"session_with_root".to_owned()),
        true
    );
    Ok(())
}
