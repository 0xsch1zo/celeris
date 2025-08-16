use crate::script::{
    pane::{Direction, Pane},
    session::Session,
};
use crate::tmux::{self, BuilderTransform};
use mlua::{ExternalResult, FromLua, Lua, LuaSerdeExt, Result, Table, UserData, Value};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug)]
pub struct WindowOptions {
    name: Option<String>,
    root: Option<PathBuf>,
    raw_command: Option<String>,
}

impl WindowOptions {
    fn try_into_builder(self, session: Arc<tmux::Session>) -> Result<tmux::WindowBuilder> {
        Ok(tmux::WindowBuilder::new(session)
            .builder_transform(self.name, tmux::WindowBuilder::name)
            .try_builder_transform(self.root, tmux::WindowBuilder::root)
            .into_lua_err()?
            .builder_transform(self.raw_command, tmux::WindowBuilder::raw_command))
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
        let builder = opts.try_into_builder(session.inner())?;
        Ok(Self {
            inner: Arc::new(builder.build().into_lua_err()?),
        })
    }

    fn default_pane(_: &Lua, this: &Self, _: ()) -> Result<Pane> {
        Ok(Pane::new(Arc::clone(&this.inner.default_pane())))
    }

    fn even_out(_: &Lua, this: &Self, direction: Direction) -> Result<()> {
        this.inner.event_out(direction.into()).into_lua_err()?;
        Ok(())
    }

    fn select(_: &Lua, this: &Self, _: ()) -> Result<()> {
        this.inner.select().into_lua_err()?;
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;

    use handlebars::Handlebars;
    use mlua::{ExternalResult, Lua, LuaSerdeExt, Result};
    use serde::Serialize;

    use crate::script::window::WindowOptions;
    use crate::tmux::SessionBuilder as TmuxSessionBuilder;
    use crate::tmux::WindowBuilder as TmuxWindowBuilder;
    use std::sync::Arc;

    #[derive(Serialize)]
    struct OptData {
        name: String,
        root: PathBuf,
        raw_command: String,
    }

    #[test]
    fn window_options() -> Result<()> {
        let lua = Lua::new();
        let handlebars = Handlebars::new();
        let session = TmuxSessionBuilder::new("__celeris_testing_lua".to_owned())
            .build()
            .into_lua_err()?;

        let opt_data = OptData {
            name: "test".to_owned(),
            root: env::temp_dir(),
            raw_command: "test".to_owned(),
        };

        let opts_given = [
            "{ name = \"{{name}}\", root = \"{{root}}\", raw_command = \"{{raw_command}}\" }",
            "{ name = \"{{name}}\", root = \"{{root}}\"}",
            "{ name = \"{{name}}\", raw_command = \"{{raw_command}}\" }",
            "{ name = \"{{name}}\" }",
            "{ root = \"{{root}}\" }",
            "{ raw_command = \"{{raw_command}}\" }",
        ]
        .into_iter()
        .map(|opt| handlebars.render_template(opt, &opt_data).into_lua_err())
        .map(|opt| lua.from_value::<WindowOptions>(lua.load(opt?).eval()?))
        .collect::<Result<Vec<_>>>()?;

        let builders_got = opts_given
            .into_iter()
            .map(|opt| opt.try_into_builder(Arc::clone(&session)))
            .collect::<Result<Vec<_>>>()?;

        let buliders_expected: Vec<_> = vec![
            TmuxWindowBuilder::new(Arc::clone(&session))
                .name(opt_data.name.clone())
                .root(opt_data.root.clone())
                .into_lua_err()?
                .raw_command(opt_data.raw_command.clone()),
            TmuxWindowBuilder::new(Arc::clone(&session))
                .name(opt_data.name.clone())
                .root(opt_data.root.clone())
                .into_lua_err()?,
            TmuxWindowBuilder::new(Arc::clone(&session))
                .name(opt_data.name.clone())
                .raw_command(opt_data.raw_command.clone()),
            TmuxWindowBuilder::new(Arc::clone(&session)).name(opt_data.name.clone()),
            TmuxWindowBuilder::new(Arc::clone(&session))
                .root(opt_data.root.clone())
                .into_lua_err()?,
            TmuxWindowBuilder::new(Arc::clone(&session)).raw_command(opt_data.raw_command.clone()),
        ];

        assert_eq!(buliders_expected, builders_got);
        Ok(())
    }
}
