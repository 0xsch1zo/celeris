mod pane;
mod session;
mod window;

use crate::config::Config;
use crate::directory_manager::DirectoryManager;
use crate::manifest;
use crate::tmux;
use crate::utils;
use color_eyre::eyre::{Context, Report, Result};
use rhai::{Engine, EvalAltResult};
use std::env;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;
use std::sync::Arc;

type ScriptFuncResult<T> = Result<T, Box<EvalAltResult>>;

fn eyre_to_rhai_err(error: Report) -> Box<EvalAltResult> {
    error.to_string().into()
}

pub struct ScriptManager {
    dir_mgr: Rc<DirectoryManager>,
}

// TODO: errors are sometimes? not detected by rhai wtf?

impl ScriptManager {
    pub fn new(dir_mgr: Rc<DirectoryManager>) -> Self {
        Self { dir_mgr }
    }

    fn script_path(&self, entry: &manifest::Entry) -> Result<PathBuf> {
        Ok(self
            .dir_mgr
            .layouts_dir()?
            .join(entry.script_name())
            .with_extension("rhai"))
    }

    pub fn create(&self, entry: &manifest::Entry) -> Result<()> {
        File::create_new(&self.script_path(entry)?).wrap_err(format!(
            "failed to create a script file for session with name: {}",
            entry.name(),
        ))?;
        Ok(())
    }

    // TODO: mabye being explicit with the creation and attaachment of the session would be better,
    // consider that
    pub fn run(&self, entry: &manifest::Entry) -> Result<()> {
        let script =
            fs::read_to_string(self.script_path(entry)?).wrap_err("session script not found")?;
        let mut engine = Engine::new();
        //register_types(&mut engine);

        let tmux_session = tmux::Session::new(
            entry.name(),
            tmux::Root::Custom(entry.session_path().to_owned()),
        )?;

        session::register(&mut engine, Arc::clone(&tmux_session));
        window::register(&mut engine);
        pane::register(&mut engine);

        engine.run(&script)?;

        tmux_session.attach()?;
        Ok(())
    }

    pub fn edit(&self, entry: &manifest::Entry, config: &Config) -> Result<()> {
        let editor = match &config.editor {
            Some(editor) => editor,
            None => &env::var("EDITOR").wrap_err("$EDITOR is not set nor set in the config")?,
        };

        process::Command::new(editor)
            .arg(utils::path_to_string(&self.script_path(entry)?)?)
            .status()?;
        Ok(())
    }

    pub fn remove(&self, entry: &manifest::Entry) -> Result<()> {
        fs::remove_file(self.script_path(entry)?).wrap_err("failed to remove script")?;
        Ok(())
    }
}
