mod pane;
mod session;
mod window;

use color_eyre::{Report, eyre};
use mlua::Lua;
use std::path::Path;

use crate::layout::Layout;
struct Error(Report);

impl From<Report> for Error {
    fn from(value: Report) -> Self {
        Error(value)
    }
}

trait IntoInteropResExt<T> {
    fn into_interop(self) -> Result<T, Error>;
}

impl<T> IntoInteropResExt<T> for Result<T, Report> {
    fn into_interop(self) -> Result<T, Error> {
        self.map_err(Error::from)
    }
}

impl From<Error> for mlua::Error {
    fn from(err: Error) -> Self {
        mlua::Error::external(err.0)
    }
}

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
