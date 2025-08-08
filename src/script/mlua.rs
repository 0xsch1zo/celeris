mod pane;
mod session;
mod window;

use session::Session;
use std::path::{Path, PathBuf};

use color_eyre::{
    Report,
    eyre::{self, Context},
};
use mlua::{Lua, Value};
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

pub fn run() -> eyre::Result<()> {
    let lua = Lua::new();
    let globals = lua.globals();
    let api = lua.create_table()?;
    globals.set("sesh", &api)?;
    let session = lua.create_table()?;
    let session_name = String::from("test");
    api.set("session", &session)?;
    session.set(
        "new",
        lua.create_function(move |ctx, (root, opts): (PathBuf, Value)| {
            Session::try_new(ctx, session_name.clone(), opts)
        })?,
    )?;

    lua.load(Path::new("test.lua")).exec()?;
    Ok(())
}
