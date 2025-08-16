#[allow(dead_code)]
mod common;

use celeris::config::Config;
use celeris::session_manager::SwitchTarget;
use celeris::session_manager::{ListSessionsOptions, SessionManager};
use color_eyre::eyre::eyre;
use color_eyre::{Result, eyre::Context};
use common::TestDirectoryManager;
use handlebars::Handlebars;
use itertools::Itertools;
use rust_embed::Embed;
use serde::Serialize;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::{env, fs};

#[derive(Embed)]
#[folder = "templates/tests/"]
#[include = "*.lua"]
struct TestFiles;

#[derive(Embed)]
#[folder = "templates/tests/"]
#[include = "*.template.lua"]
struct TemplateFiles;

#[derive(Embed)]
#[folder = "templates/"]
#[include = "*.lua"]
struct DefaultTemplate;

#[derive(Serialize)]
struct TestData {
    session_root: PathBuf,
}

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
fn list_sessions_active() -> Result<()> {
    unsafe {
        env::set_var("CELERIS_TMUX_SOCKET_NAME", "__celeris_testing");
    }

    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        ..Config::default()
    });

    let dummy_layouts = ["test1", "test2", "test3"];
    common::create_dummy_layouts(&dummy_layouts, dir_mgr.as_ref())?;
    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;

    let active_layouts = ["active_test", "active_test2", "active_test3"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();

    let generic_layout = TestFiles::get("generic_layout.lua").unwrap().data;
    active_layouts.iter().try_for_each(|layout| -> Result<()> {
        session_manager.create(Some(layout.to_owned()), env::temp_dir())?;
        Ok(())
    })?;

    let layouts_dir = dir_mgr.layouts_dir()?;
    active_layouts.iter().try_for_each(|layout| {
        fs::write(
            layouts_dir.join(layout).with_extension("lua"),
            &generic_layout,
        )
    })?;

    active_layouts
        .iter()
        .try_for_each(|layout| session_manager.switch(SwitchTarget::Session(layout.to_owned())))?;

    let opts = ListSessionsOptions {
        tmux_format: false,
        include_active: true,
        exclude_running: false,
    };

    let output = session_manager.list(opts)?;
    output
        .lines()
        .map(str::trim)
        .sorted()
        .zip(
            dummy_layouts
                .into_iter()
                .chain(active_layouts.iter().map(String::as_str))
                .sorted(),
        )
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

#[test]
fn new_session_default_template() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        disable_template: true,
        ..Config::default()
    });
    let layout_data = TestData {
        session_root: env::temp_dir(),
    };
    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(Some("test".to_owned()), layout_data.session_root.clone())?;

    let layout_path = dir_mgr.layouts_dir()?.join("test").with_extension("lua");
    let template = fs::read_to_string(&layout_path)?;
    assert!(template.is_empty());
    session_manager.remove("test")?;

    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        ..Config::default()
    });

    let mut handlebars = Handlebars::new();
    handlebars.register_embed_templates_with_extension::<DefaultTemplate>(".lua")?;
    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(Some("test".to_owned()), layout_data.session_root.clone())?;
    let template = fs::read_to_string(&layout_path)?;
    assert_eq!(template, handlebars.render("default", &layout_data)?);
    Ok(())
}

#[test]
fn new_session_custom_template() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        disable_template: true,
        ..Config::default()
    });
    let layout_data = TestData {
        session_root: env::temp_dir(),
    };

    let mut handlebars = Handlebars::new();

    handlebars.register_embed_templates_with_extension::<TestFiles>(".lua")?;
    let template_given = handlebars.render("generic_layout", &layout_data)?;
    fs::write(dir_mgr.custom_template_path()?, &template_given)?;

    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(Some("test".to_owned()), layout_data.session_root.clone())?;

    let layout_path = dir_mgr.layouts_dir()?.join("test").with_extension("lua");
    let template_got = fs::read_to_string(&layout_path)?;
    assert!(template_got.is_empty());
    session_manager.remove("test")?;

    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        ..Config::default()
    });
    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(Some("test".to_owned()), layout_data.session_root.clone())?;
    let template_got = fs::read_to_string(&layout_path)?;
    assert_eq!(template_got, template_given);
    Ok(())
}

// shitty test
#[test]
fn last_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
        disable_editor_on_creation: true,
        ..Config::default()
    });

    let template = TestFiles::get("generic_layout.lua").unwrap().data;
    fs::write(&dir_mgr.custom_template_path()?, template)?;

    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(Some("test".to_owned()), env::temp_dir())?;
    let _ = session_manager
        .switch(SwitchTarget::LastSession)
        .expect_err("switch should error out when there is no last session");

    session_manager.switch(SwitchTarget::Session("test".to_owned()))?;
    session_manager.switch(SwitchTarget::LastSession)?;
    Ok(())
}

#[test]
fn comp_test() -> Result<()> {
    unsafe {
        env::set_var("CELERIS_TMUX_SOCKET_NAME", "__celeris_testing");
    }
    let dir_mgr = TestDirectoryManager::new()?;
    let mut handlebars = Handlebars::new();
    handlebars.register_embed_templates_with_extension::<TemplateFiles>(".template.lua")?;
    let test_data = TestData {
        session_root: env::temp_dir(),
    };
    let layout_str = handlebars.render("comptest", &test_data)?;
    common::new_layout("comptest", &layout_str, dir_mgr.as_ref())?;
    let session_manager = common::test_session_manager(Arc::clone(dir_mgr.inner()))?;
    session_manager.switch(SwitchTarget::Session("comptest".to_owned()))?;
    Ok(())
}
