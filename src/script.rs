mod pane;
mod session;
mod window;

use crate::layout::Layout;
use color_eyre::eyre::{Context, Report, Result};
use rhai::{Engine, EvalAltResult};
use std::fs;
use std::path::Path;

type ScriptFuncResult<T> = Result<T, Box<EvalAltResult>>;

fn eyre_to_rhai_err(error: Report) -> Box<EvalAltResult> {
    error.to_string().into()
}

pub fn run(layout: &Layout, layouts_dir: &Path) -> Result<()> {
    let layout_path = layout.storage_path(layouts_dir);
    let script = fs::read_to_string(&layout_path).wrap_err("session script not found")?;
    let mut engine = Engine::new();

    session::register(&mut engine, layout.tmux_name().to_owned());
    window::register(&mut engine);
    pane::register(&mut engine);

    engine.run(&script)?;

    Ok(())
}
