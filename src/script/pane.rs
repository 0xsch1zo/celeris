use crate::tmux::{self, BuilderTransform, Target};
use color_eyre::eyre::{self, Context};
use mlua::{ExternalResult, FromLua, Lua, LuaSerdeExt, Result, Table, UserData};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl From<Direction> for tmux::Direction {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Horizontal => tmux::Direction::Horizontal,
            Direction::Vertical => tmux::Direction::Vertical,
        }
    }
}

impl UserData for Direction {}

impl FromLua for Direction {
    fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
        lua.from_value(value)
    }
}

/*#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
enum SplitSize {
    Absolute { value: u32 },
    Percentage { value: u8 },
}

impl FromLua for SplitSize {
    fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
        lua.from_value(value)
    }
}

impl From<SplitSize> for tmux::SplitSize {
    fn from(value: SplitSize) -> Self {
        match value {
            SplitSize::Absolute { value } => tmux::SplitSize::Absolute(value),
            SplitSize::Percentage { value } => tmux::SplitSize::Percentage(value),
        }
    }
}
*/

#[derive(Serialize, Deserialize, Debug)]
pub struct SplitOptions {
    root: Option<PathBuf>,
    size: Option<String>,
}

impl SplitOptions {
    fn try_into_builder(
        self,
        sibling_pane: Arc<tmux::Pane>,
        direction: Direction,
    ) -> Result<tmux::SplitBuilder> {
        let size = self
            .size
            .map(|s| -> eyre::Result<tmux::SplitSize> {
                let size = s.trim();
                if size.ends_with("%") {
                    Ok(tmux::SplitSize::Percentage(
                        size.strip_suffix("%")
                            .expect(
                                "split size which ends with % should be strippable from the % sign",
                            )
                            .parse::<u8>()
                            .wrap_err_with(|| format!("failed to parse percentage size: {size}"))?,
                    ))
                } else {
                    Ok(tmux::SplitSize::Absolute(
                        size.parse::<u32>()
                            .wrap_err_with(|| format!("failed to parse percentage size: {size}"))?,
                    ))
                }
            })
            .transpose()
            .into_lua_err()?;

        Ok(sibling_pane
            .split(direction.into())
            .try_builder_transform(self.root, tmux::SplitBuilder::root)
            .into_lua_err()?
            .builder_transform(size, tmux::SplitBuilder::size))
    }
}

impl UserData for SplitOptions {}

impl FromLua for SplitOptions {
    fn from_lua(value: mlua::Value, lua: &Lua) -> Result<Self> {
        lua.from_value(value)
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    inner: Arc<tmux::Pane>,
}

impl Pane {
    pub fn new(inner: Arc<tmux::Pane>) -> Pane {
        Self { inner }
    }

    fn split(_: &Lua, this: &Self, (direction, opts): (Direction, SplitOptions)) -> Result<Pane> {
        let inner = opts
            .try_into_builder(Arc::clone(&this.inner), direction)?
            .build()
            .into_lua_err()?;
        Ok(Pane::new(Arc::new(inner)))
    }

    fn select(_: &Lua, this: &Self, _: ()) -> Result<()> {
        this.inner.select().into_lua_err()?;
        Ok(())
    }

    fn run_command(_: &Lua, this: &Self, command: String) -> Result<()> {
        this.inner.run_command(&command).into_lua_err()?;
        Ok(())
    }

    fn target(_: &Lua, this: &Self, _: ()) -> Result<String> {
        Ok(this.inner.target().get().to_owned())
    }
}

impl UserData for Pane {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("split", Pane::split);
        methods.add_method("select", Pane::select);
        methods.add_method("run_command", Pane::run_command);
        methods.add_method("target", Pane::target);
    }
}

pub fn register(ctx: &Lua, api: &mut Table) -> Result<()> {
    api.set("Pane", ctx.create_proxy::<Pane>()?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;

    use handlebars::Handlebars;
    use mlua::{ExternalResult, Lua, LuaSerdeExt, Result};
    use serde::Serialize;

    use crate::script::pane::Direction;
    use crate::script::pane::SplitOptions;
    use crate::tmux::SessionBuilder as TmuxSessionBuilder;
    use crate::tmux::SplitSize as TmuxSplitSize;
    use crate::tmux::WindowBuilder as TmuxWindowBuilder;
    use std::sync::Arc;

    #[derive(Serialize)]
    struct OptData {
        root: PathBuf,
        absolute_size: u32,
        percentage_size: u8,
        direction: Direction,
    }

    #[test]
    fn split_options() -> Result<()> {
        let lua = Lua::new();
        let handlebars = Handlebars::new();
        let session = TmuxSessionBuilder::new("__celeris_testing_lua".to_owned())
            .build()
            .into_lua_err()?;
        let window = TmuxWindowBuilder::new(session).build().into_lua_err()?;
        let default_pane = window.default_pane();

        let opt_data = OptData {
            root: env::temp_dir(),
            absolute_size: 420,
            percentage_size: 69,
            direction: Direction::Vertical,
        };

        let opts_given = [
            r#"{ root = "{{root}}", size = "{{absolute_size}}" }"#,
            r#"{ root = "{{root}}", size = "{{percentage_size}}%" }"#,
            r#"{ size = "{{absolute_size}}" }"#,
            r#"{ size = "{{percentage_size}}%" }"#,
            r#"{ root = "{{root}}" }"#,
        ]
        .into_iter()
        .map(|opt| handlebars.render_template(opt, &opt_data).into_lua_err())
        .map(|opt| lua.from_value::<SplitOptions>(lua.load(opt?).eval()?))
        .collect::<Result<Vec<_>>>()?;

        let builders_got = opts_given
            .into_iter()
            .map(|opt| opt.try_into_builder(Arc::clone(&default_pane), opt_data.direction.clone()))
            .collect::<Result<Vec<_>>>()?;

        let buliders_expected: Vec<_> = vec![
            default_pane
                .split(opt_data.direction.clone().into())
                .size(TmuxSplitSize::Absolute(opt_data.absolute_size.clone()))
                .root(opt_data.root.clone())
                .into_lua_err()?,
            default_pane
                .split(opt_data.direction.clone().into())
                .size(TmuxSplitSize::Percentage(opt_data.percentage_size.clone()))
                .root(opt_data.root.clone())
                .into_lua_err()?,
            default_pane
                .split(opt_data.direction.clone().into())
                .size(TmuxSplitSize::Absolute(opt_data.absolute_size.clone())),
            default_pane
                .split(opt_data.direction.clone().into())
                .size(TmuxSplitSize::Percentage(opt_data.percentage_size.clone())),
            default_pane
                .split(opt_data.direction.clone().into())
                .root(opt_data.root.clone())
                .into_lua_err()?,
        ];

        assert_eq!(buliders_expected, builders_got);

        let opts_given = [
            r#"{ size = "&{{absolute_size}}" }"#,
            r#"{ size = " {{percentage_size}} %" }"#,
            r#"{ size = "-{{percentage_size}}-% " }"#,
        ]
        .into_iter()
        .map(|opt| handlebars.render_template(opt, &opt_data).into_lua_err())
        .map(|opt| lua.from_value::<SplitOptions>(lua.load(opt?).eval()?))
        .collect::<Result<Vec<_>>>()?;

        opts_given
            .into_iter()
            .map(|opt| opt.try_into_builder(Arc::clone(&default_pane), opt_data.direction.clone()))
            .for_each(|result| {
                let _ = result.expect_err("should fail under eroneous value");
            });
        Ok(())
    }
}
