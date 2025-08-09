use crate::script::mlua::{
    IntoInteropResExt,
    pane::{Direction, Pane},
    session::Session,
};
use crate::tmux::{self, BuilderTransform};
use mlua::{FromLua, Lua, LuaSerdeExt, Result, Table, UserData, Value};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct WindowOptions {
    name: Option<String>,
    root: Option<PathBuf>,
    // FIXME
    raw_command: Option<String>,
}

impl WindowOptions {
    fn into_builder(self, session: Arc<tmux::Session>) -> tmux::WindowBuilder {
        tmux::WindowBuilder::new(session)
            .builder_transform(self.name, tmux::WindowBuilder::name)
            .builder_transform(self.root, tmux::WindowBuilder::root)
            .builder_transform(self.raw_command, tmux::WindowBuilder::raw_command)
    }
}

impl UserData for WindowOptions {}

impl FromLua for WindowOptions {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        lua.from_value::<Self>(value)
    }
}

#[derive(Clone, Debug)]
pub struct Window {
    inner: Arc<tmux::Window>,
}

impl Window {
    fn try_new(_: &Lua, (session, opts): (Session, WindowOptions)) -> Result<Window> {
        let builder = opts.into_builder(session.inner());
        Ok(Self {
            inner: Arc::new(builder.build().into_interop()?),
        })
    }

    fn default_pane(_: &Lua, this: &Self, _: ()) -> Result<Pane> {
        Ok(Pane::new(Arc::clone(&this.inner.default_pane())))
    }

    fn even_out(_: &Lua, this: &Self, direction: Direction) -> Result<()> {
        this.inner.event_out(direction.into()).into_interop()?;
        Ok(())
    }

    fn select(_: &Lua, this: &Self, _: ()) -> Result<()> {
        this.inner.select().into_interop()?;
        Ok(())
    }
}

impl UserData for Window {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("new", Window::try_new);
        methods.add_method("default_pane", Window::default_pane);
        methods.add_method("even_out", Window::even_out);
        methods.add_method("select", Window::select);
    }
}
pub fn register(ctx: &Lua, api: &mut Table) -> Result<()> {
    api.set("Window", ctx.create_proxy::<Window>()?)?;
    Ok(())
}
