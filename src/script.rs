mod pane;
mod session;
mod window;

use crate::layout::Layout;
use crate::tmux;
use color_eyre::eyre::{Context, Report, Result};
use rhai::{Engine, EvalAltResult};
use std::fs;
use std::path::Path;
use std::sync::Arc;

type ScriptFuncResult<T> = Result<T, Box<EvalAltResult>>;

fn eyre_to_rhai_err(error: Report) -> Box<EvalAltResult> {
    error.to_string().into()
}

// TODO: errors are sometimes? not detected by rhai wtf?

// TODO: mabye being explicit with the creation and attaachment of the session would be better,
// consider that
pub fn run(layout: &Layout, layouts_dir: &Path) -> Result<()> {
    let layout_path = layout.storage_path(layouts_dir);
    let script = fs::read_to_string(&layout_path).wrap_err("session script not found")?;
    let mut engine = Engine::new();
    //register_types(&mut engine);

    let tmux_session = tmux::SessionBuilder::new(layout.tmux_name().to_owned()).build()?;

    session::register(&mut engine, Arc::clone(&tmux_session));
    window::register(&mut engine);
    pane::register(&mut engine);

    engine.run(&script)?;

    tmux_session.attach()?;
    Ok(())
}
