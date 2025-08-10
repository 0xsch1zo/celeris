use crate::tmux;
use crate::{script::IntoInteropResExt, tmux::BuilderTransform};
use color_eyre::eyre::WrapErr;
use mlua::{FromLua, Lua, LuaSerdeExt, Result, Table, UserData, UserDataMethods, Value};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Deserialize, Serialize)]
struct SessionOptions {
    root: Option<PathBuf>,
}

impl SessionOptions {
    fn try_into_builder(self, session_name: String) -> Result<tmux::SessionBuilder> {
        Ok(tmux::SessionBuilder::new(session_name)
            .try_builder_transform(self.root, tmux::SessionBuilder::root)
            .into_interop()?)
    }
}

impl UserData for SessionOptions {}

impl FromLua for SessionOptions {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        lua.from_value::<Self>(value)
    }
}
// wrapper around tmux::Session
#[derive(Clone, Debug, FromLua)]
pub struct Session {
    inner: Arc<tmux::Session>,
}

impl Session {
    fn try_new(ctx: &Lua, opts: SessionOptions) -> Result<Session> {
        let session_name: String = ctx
            .named_registry_value("SESH_SESSION_NAME")
            .wrap_err("failed to get session name from the lua registry")
            .into_interop()?;

        Ok(Self {
            inner: opts
                .try_into_builder(session_name)?
                .build()
                .into_interop()?,
        })
    }

    pub fn inner(self) -> Arc<tmux::Session> {
        self.inner
    }

    fn attach(_: &Lua, this: &mut Self, _: ()) -> Result<()> {
        this.inner.attach().into_interop()?;
        Ok(())
    }
}

impl UserData for Session {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("new", Session::try_new);
        methods.add_method_mut("attach", Session::attach);
    }
}

pub fn register(ctx: &Lua, api: &mut Table) -> Result<()> {
    api.set("Session", ctx.create_proxy::<Session>()?)?;
    Ok(())
}
