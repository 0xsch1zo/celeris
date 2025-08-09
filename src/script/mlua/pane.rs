use crate::script::mlua::IntoInteropResExt;
use crate::tmux::{self, BuilderTransform};
use mlua::{FromLua, Lua, LuaSerdeExt, Result, Table, UserData};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct SplitOptions {
    root: Option<PathBuf>,
    size: Option<SplitSize>,
}

impl SplitOptions {
    fn into_builder(
        self,
        sibling_pane: Arc<tmux::Pane>,
        direction: Direction,
    ) -> tmux::SplitBuilder {
        sibling_pane
            .split(direction.into())
            .builder_transform(self.root, tmux::SplitBuilder::root)
            .builder_transform(self.size.map(|s| s.into()), tmux::SplitBuilder::size)
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
            .into_builder(Arc::clone(&this.inner), direction)
            .build()
            .into_interop()?;
        Ok(Pane::new(Arc::new(inner)))
    }

    fn select(_: &Lua, this: &Self, _: ()) -> Result<()> {
        this.inner.select().into_interop()?;
        Ok(())
    }

    fn run_command(_: &Lua, this: &Self, command: String) -> Result<()> {
        this.inner.run_command(&command).into_interop()?;
        Ok(())
    }
}

impl UserData for Pane {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // TODO: constider making static, something like sesh.Pane.split()
        methods.add_method("split", Pane::split);
        methods.add_method("select", Pane::select);
        methods.add_method("run_command", Pane::run_command);
    }
}

pub fn register(ctx: &Lua, api: &mut Table) -> Result<()> {
    api.set("Pane", ctx.create_proxy::<Pane>()?)?;
    Ok(())
}
