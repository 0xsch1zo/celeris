mod pane;
mod session;
mod window;

use crate::config::Config;
use crate::manifest;
use crate::tmux;
use crate::utils;
use color_eyre::eyre::{self, Context};
use rhai::{Engine, EvalAltResult};
use std::env;
use std::fs;
use std::process;
use std::sync::Arc;

type ScriptFuncResult<T> = Result<T, Box<EvalAltResult>>;

fn eyre_to_rhai_err(error: eyre::Report) -> Box<EvalAltResult> {
    error.to_string().into()
}

// TODO: mabye being explicit with the creation and attaachment of the session would be better,
// consider that
pub fn run(entry: &manifest::Entry) -> eyre::Result<()> {
    let script = fs::read_to_string(entry.script_path()).wrap_err("session script not found")?;
    let mut engine = Engine::new();
    //register_types(&mut engine);

    let tmux_session = tmux::Session::new(
        entry.name(),
        tmux::SessionRoot::Custom(entry.session_path()),
    )?;

    session::register(&mut engine, Arc::clone(&tmux_session));
    window::register(&mut engine);
    pane::register(&mut engine);

    engine.run(&script)?;

    tmux_session.attach()?;
    Ok(())
}

pub fn edit(entry: &manifest::Entry, config: &Config) -> eyre::Result<()> {
    let editor = match &config.editor {
        Some(editor) => editor,
        None => &env::var("EDITOR").wrap_err("$EDITOR is not set nor set in the config")?,
    };

    process::Command::new(editor)
        .arg(utils::path_to_string(entry.script_path())?)
        .status()?;
    Ok(())
}
