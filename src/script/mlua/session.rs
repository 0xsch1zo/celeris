use crate::script::mlua::IntoInteropResExt;
use crate::tmux;
use color_eyre::eyre::WrapErr;
use mlua::{AnyUserData, Lua, LuaSerdeExt, Result, UserData, UserDataMethods, Value};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Deserialize, Serialize)]
struct SessionOptions {
    root: Option<PathBuf>,
}

impl SessionOptions {
    fn try_into_builder(self, session_name: String) -> Result<tmux::SessionBuilder> {
        let mut builder = tmux::SessionBuilder::new(session_name);
        if let Some(root) = self.root {
            builder.root(root).into_interop()?;
        }
        Ok(builder)
    }

    /*fn try_new(ctx: &Lua, _: ()) -> Result<Self> {
        let session_name: String = ctx
            .named_registry_value("SESH_SESSION_NAME")
            .wrap_err("failed to get session name from the lua registry")
            .into_interop()?;

        let builder = tmux::SessionBuilder::new(session_name);
        Ok(Self {
            inner: Arc::new(Mutex::new(builder)),
        })
    }*/

    /*fn root(_: &Lua, this: &mut Self, root: String) -> Result<Self> {
        let root = PathBuf::from(root);
        this.inner.lock().unwrap().root(root).into_interop()?;
        Ok(this.clone())
    }

    fn build(_: &Lua, this: &mut Self, _: ()) -> Result<Session> {
        let tmux_session = this.inner.lock().unwrap().build().into_interop()?;
        Ok(Session::new(tmux_session))
    }*/
}

impl UserData for SessionOptions {}

// wrapper around tmux::Session
#[derive(Clone, Debug)]
pub struct Session {
    inner: Arc<tmux::Session>,
}

impl Session {
    pub fn try_new(ctx: &Lua, session_name: String, opts: Value) -> Result<Session> {
        /*let session_name: String = ctx
        .named_registry_value("SESH_SESSION_NAME")
        .wrap_err("failed to get session name from the lua registry")
        .into_interop()?;*/

        let opts = ctx.from_value::<SessionOptions>(opts)?;
        Ok(Self {
            inner: opts
                .try_into_builder(session_name)?
                .build()
                .into_interop()?,
        })
    }

    fn attach(_: &Lua, this: &mut Self, _: ()) -> Result<()> {
        this.inner.attach().into_interop()?;
        Ok(())
    }
}

impl UserData for Session {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("attach", Session::attach);
    }
}
