#[allow(dead_code)]
mod common;

use celeris::config::Config;
use celeris::session_manager::{CreateSessionOptions, SwitchTarget};
use celeris::session_manager::{ListSessionsOptions, SessionManager};
use celeris::tmux::Session;
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

use crate::common::test_session_manager;

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
        only_running: false,
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
fn only_running() -> Result<()> {
    unsafe {
        env::set_var("CELERIS_TMUX_SOCKET_NAME", "__celeris_testing");
    }
    let dir_mgr = TestDirectoryManager::new()?;
    let session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;
    let session_names = ["__celeris_testing_1", "__celeris_testing_2"];
    let _sessions = session_names
        .iter()
        .map(|name| Session::builder((*name).to_owned()))
        .map(|mut builder| builder.build())
        .collect::<Result<Vec<_>>>()?;

    let opts = ListSessionsOptions {
        tmux_format: false,
        include_active: false,
        exclude_running: false,
        only_running: true,
    };
    session_manager
        .list(opts)?
        .lines()
        .sorted()
        .zip(session_names.into_iter().sorted())
        .for_each(|(given, got)| assert!(given.contains(got)));
    Ok(())
}

#[test]
fn list_sessions_active() -> Result<()> {
    unsafe {
        env::set_var("CELERIS_TMUX_SOCKET_NAME", "__celeris_testing");
    }

    let dir_mgr = TestDirectoryManager::new()?;
    let dummy_layouts = ["test1", "test2", "test3"];
    common::create_dummy_layouts(&dummy_layouts, dir_mgr.as_ref())?;
    let mut session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;

    let active_layouts = ["active_test", "active_test2", "active_test3"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();

    let generic_layout = TestFiles::get("generic_layout.lua").unwrap().data;
    active_layouts.iter().try_for_each(|layout| -> Result<()> {
        session_manager.create(CreateSessionOptions {
            name: Some(layout.to_owned()),
            path: env::temp_dir(),
            disable_editor: true,
            machine_readable: false,
        })?;
        Ok(())
    })?;

    let layouts_dir = dir_mgr.layouts_dir();
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
        only_running: false,
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
        .layouts_dir()
        .join("test")
        .with_extension("lua");

    println!(
        "{}",
        session_manager.list(ListSessionsOptions {
            tmux_format: false,
            include_active: false,
            exclude_running: true,
            only_running: false,
        })?
    );
    assert!(layout_path.exists());
    session_manager.remove("test")?;
    assert!(!layout_path.exists());

    Ok(())
}

#[test]
fn create_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let session_manager = Mutex::new(common::test_session_manager(Arc::clone(dir_mgr.inner()))?);
    let layout_path = dir_mgr.layouts_dir().join("test");
    File::create_new(&layout_path).wrap_err("failed to create layout's target")?;
    let layout_path_c = layout_path.clone();
    let (err_tx, err_rx) = mpsc::channel();

    thread::spawn(move || {
        let opts = CreateSessionOptions {
            disable_editor: false,
            name: None,
            path: layout_path,
            machine_readable: false,
        };
        let result = session_manager.lock().unwrap().create(opts);
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
fn create_session_default_template() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
        disable_template: true,
        ..Config::default()
    });
    let layout_data = TestData {
        session_root: env::temp_dir(),
    };

    let opts = CreateSessionOptions {
        disable_editor: true,
        path: layout_data.session_root.clone(),
        name: Some("test".to_owned()),
        machine_readable: false,
    };
    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(opts.clone())?;

    let layout_path = dir_mgr.layouts_dir().join("test").with_extension("lua");
    let template = fs::read_to_string(&layout_path)?;
    assert!(template.is_empty());
    session_manager.remove("test")?;

    let mut handlebars = Handlebars::new();
    handlebars.register_embed_templates_with_extension::<DefaultTemplate>(".lua")?;
    let mut session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;
    session_manager.create(opts)?;
    let template = fs::read_to_string(&layout_path)?;
    assert_eq!(template, handlebars.render("default", &layout_data)?);
    Ok(())
}

#[test]
fn create_session_custom_template() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config = Arc::new(Config {
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

    let opts = CreateSessionOptions {
        path: layout_data.session_root,
        name: Some("test".to_owned()),
        disable_editor: true,
        machine_readable: false,
    };

    let mut session_manager = SessionManager::new(config, Arc::clone(dir_mgr.inner()))?;
    session_manager.create(opts.clone())?;

    let layout_path = dir_mgr.layouts_dir().join("test").with_extension("lua");
    let template_got = fs::read_to_string(&layout_path)?;
    assert!(template_got.is_empty());
    session_manager.remove("test")?;

    let mut session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;
    session_manager.create(opts)?;
    let template_got = fs::read_to_string(&layout_path)?;
    assert_eq!(template_got, template_given);
    Ok(())
}

#[test]
fn create_all() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let mut session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;
    let paths = vec![
        dir_mgr.layouts_dir().join("test1"),
        dir_mgr.layouts_dir().join("test2"),
        dir_mgr.layouts_dir().join("test3"),
    ];

    paths
        .iter()
        .try_for_each(|path| -> Result<()> { Ok(fs::create_dir(path)?) })?;
    session_manager.create_all(paths.clone())?;
    paths
        .into_iter()
        .map(|path| path.with_extension("lua"))
        .for_each(|path| assert!(path.exists()));
    let paths = vec![
        dir_mgr.layouts_dir().join("test4"),
        dir_mgr.layouts_dir().join("dummy").join("test4"),
    ];

    paths
        .iter()
        .try_for_each(|path| -> Result<()> { Ok(fs::create_dir_all(path)?) })?;

    paths
        .iter()
        .map(|path| path.with_extension("lua"))
        .for_each(|path| assert!(!path.exists()));

    let _ = session_manager
        .create_all(paths.clone())
        .expect_err("create-all should fail with duplicate file names");
    paths
        .into_iter()
        .map(|path| path.with_extension("lua"))
        .for_each(|path| assert!(!path.exists()));
    Ok(())
}

// shitty test
#[test]
fn last_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let mut session_manager = test_session_manager(Arc::clone(dir_mgr.inner()))?;

    let template = TestFiles::get("generic_layout.lua").unwrap().data;
    fs::write(&dir_mgr.custom_template_path()?, template)?;

    let opts = CreateSessionOptions {
        disable_editor: true,
        path: env::temp_dir(),
        name: Some("test".to_owned()),
        machine_readable: false,
    };

    session_manager.create(opts)?;
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
