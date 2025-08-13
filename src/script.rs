mod pane;
mod session;
mod window;

use color_eyre::eyre;
use mlua::Lua;
use std::path::Path;

use crate::layout::Layout;

pub fn run(layout: &Layout, layouts_dir: &Path) -> eyre::Result<()> {
    let lua = Lua::new();
    lua.set_named_registry_value("SESH_SESSION_NAME", layout.tmux_name())?;

    let mut api = lua.create_table()?;
    lua.register_module("sesh", &api)?;

    session::register(&lua, &mut api)?;
    window::register(&lua, &mut api)?;
    pane::register(&lua, &mut api)?;

    let layout_path = layout.storage_path(layouts_dir);
    lua.load(layout_path).exec()?;
    Ok(())
}
