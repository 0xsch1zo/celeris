mod pane;
mod session;
mod window;

use crate::layout::Layout;
use crate::tmux::{TmuxExecuteExt, tmux};
use color_eyre::eyre::{self, Context};
use mlua::{ExternalResult, Lua, Result};
use std::path::Path;

fn raw_command(_: &Lua, args: Vec<String>) -> Result<String> {
    let output = tmux()
        .wrap_err("failed to assemble custom tmux command")
        .into_lua_err()?
        .args(&args)
        .execute()
        .wrap_err("failed to execute custom tmux command")
        .into_lua_err()?;
    Ok(output)
}

pub fn run(layout: &Layout, layouts_dir: &Path) -> eyre::Result<()> {
    let lua = Lua::new();
    lua.set_named_registry_value("CELERIS_SESSION_NAME", layout.tmux_name())?;

    let mut api = lua.create_table()?;
    lua.register_module("celeris", &api)?;

    session::register(&lua, &mut api)?;
    window::register(&lua, &mut api)?;
    pane::register(&lua, &mut api)?;
    api.set("rawCommand", lua.create_function(raw_command)?)?;

    let layout_path = layout.storage_path(layouts_dir);
    lua.load(layout_path).exec()?;
    Ok(())
}
