use crate::tmux::BuilderTransform;
use crate::tmux::{self, Target};
use color_eyre::eyre::WrapErr;
use mlua::{
    ExternalResult, FromLua, Lua, LuaSerdeExt, Result, Table, UserData, UserDataMethods, Value,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

#[derive(Deserialize, Serialize, Debug)]
struct SessionOptions {
    root: Option<PathBuf>,
}

impl SessionOptions {
    fn try_into_builder(self, session_name: String) -> Result<tmux::SessionBuilder> {
        Ok(tmux::SessionBuilder::new(session_name)
            .try_builder_transform(self.root, tmux::SessionBuilder::root)
            .into_lua_err()?)
    }
}

impl UserData for SessionOptions {}

impl FromLua for SessionOptions {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        lua.from_value(value)
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
            .named_registry_value("CELERIS_SESSION_NAME")
            .wrap_err("failed to get session name from the lua registry")
            .into_lua_err()?;

        Ok(Self {
            inner: opts
                .try_into_builder(session_name)?
                .build()
                .into_lua_err()?,
        })
    }

    pub fn inner(self) -> Arc<tmux::Session> {
        self.inner
    }

    fn attach(_: &Lua, this: &mut Self, _: ()) -> Result<()> {
        this.inner.attach().into_lua_err()?;
        Ok(())
    }

    fn target(_: &Lua, this: &Self, _: ()) -> Result<String> {
        Ok(this.inner.target().get().to_owned())
    }
}

impl UserData for Session {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("new", Session::try_new);
        methods.add_method_mut("attach", Session::attach);
        methods.add_method("target", Session::target);
    }
}

pub fn register(ctx: &Lua, api: &mut Table) -> Result<()> {
    api.set("Session", ctx.create_proxy::<Session>()?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use handlebars::Handlebars;
    use mlua::{ExternalResult, Lua, LuaSerdeExt, Result};
    use serde::Serialize;

    use crate::{script::session::SessionOptions, tmux};

    #[derive(Serialize)]
    struct OptData {
        root: PathBuf,
    }

    #[test]
    fn session_options() -> Result<()> {
        let lua = Lua::new();
        let handlebars = Handlebars::new();
        let opt_data = OptData {
            root: env::temp_dir(),
        };
        let given_opts: Vec<_> = ["{ root = \"{{root}}\" }", "{}"]
            .into_iter()
            .map(|opt| handlebars.render_template(opt, &opt_data).into_lua_err())
            .map(|opt| lua.from_value::<SessionOptions>(lua.load(opt?).eval()?))
            .collect::<Result<Vec<_>>>()?;

        let got_builders = given_opts
            .into_iter()
            .map(|opt| opt.try_into_builder("test".to_owned()))
            .collect::<Result<Vec<_>>>()?;

        let expected_builders = vec![
            tmux::SessionBuilder::new("test".to_owned())
                .root(opt_data.root)
                .into_lua_err()?,
            tmux::SessionBuilder::new("test".to_owned()),
        ];

        assert_eq!(expected_builders, got_builders);
        Ok(())
    }
}
