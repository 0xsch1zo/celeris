use crate::script::{self, ScriptFuncResult};
use crate::tmux::{self, Direction, SplitSize};
use rhai::{CustomType, Engine, Module, TypeBuilder, export_module, exported_module};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

macro_rules! pure_enum_module {
    ($module:ident : $typ:ty => $($variant:ident),+) => {
        use rhai::Dynamic;
        use rhai::plugin::*;

        #[export_module]
        pub mod $module {
            $(
                #[allow(non_upper_case_globals)]
                pub const $variant: $typ = <$typ>::$variant;
            )*

            #[rhai_fn(global, get = "type", pure)]
            pub fn get_type(this: &mut $typ) -> String {
                match this {
                    $( <$typ>::$variant => format!("{}", stringify!($variant)), )+
                }
            }

            #[rhai_fn(global, name = "to_string", name = "to_debug", pure)]
            pub fn to_string(this: &mut $typ) -> String {
                format!("{this:?}")
            }

            #[rhai_fn(global, name = "==", pure)]
            pub fn eq(this: &mut $typ, other: $typ) -> bool {
                *this == other
            }

            #[rhai_fn(global, name = "!=", pure)]
            pub fn neq(this: &mut $typ, other: $typ) -> bool {
                *this != other
            }
        }
    };
}

pure_enum_module! { direction_enum_mod: Direction => Vertical, Horizontal }

#[derive(Clone)]
pub struct SplitBuilder {
    inner: Arc<Mutex<tmux::SplitBuilder>>,
}

impl SplitBuilder {
    fn new(inner: tmux::SplitBuilder) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn root(&mut self, path: &str) -> ScriptFuncResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("{path:?} does not exist").into());
        }
        self.inner.lock().unwrap().root(path);
        Ok(self.clone())
    }

    fn absolute_size(&mut self, size: i64) -> Self {
        self.inner
            .lock()
            .unwrap()
            .size(SplitSize::Absolute(size as u32));
        self.clone()
    }

    fn percentage_size(&mut self, size: i64) -> Self {
        self.inner
            .lock()
            .unwrap()
            .size(SplitSize::Percentage(size as u8));
        self.clone()
    }

    fn build(&mut self) -> ScriptFuncResult<Pane> {
        Ok(Pane {
            inner: Arc::new(
                self.inner
                    .lock()
                    .unwrap()
                    .build()
                    .map_err(|e| script::eyre_to_rhai_err(e))?,
            ),
        })
    }
}

impl CustomType for SplitBuilder {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("SplitBuilder")
            .with_fn("root", SplitBuilder::root)
            .with_fn("absolute_size", SplitBuilder::absolute_size)
            .with_fn("percentage_size", SplitBuilder::percentage_size)
            .with_fn("build", SplitBuilder::build);
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    inner: Arc<tmux::Pane>,
}

impl Pane {
    // TODO: move arc when taking ownership, do thie EVERYWHERE
    pub fn new(inner: Arc<tmux::Pane>) -> Pane {
        Self { inner }
    }

    fn split_builder(&mut self, direction: Direction) -> SplitBuilder {
        SplitBuilder::new(self.inner.split_builder(direction))
    }

    fn select(&mut self) -> ScriptFuncResult<()> {
        self.inner
            .select()
            .map_err(|e| script::eyre_to_rhai_err(e))?;
        Ok(())
    }

    fn run_command(&mut self, command: &str) -> ScriptFuncResult<()> {
        self.inner
            .run_command(command)
            .map_err(|e| script::eyre_to_rhai_err(e))?;
        Ok(())
    }
}

impl CustomType for Pane {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Pane")
            .with_fn("select", Pane::select)
            .with_fn("split_builder", Pane::split_builder)
            .with_fn("run_command", Pane::run_command);
    }
}

pub fn register(engine: &mut Engine) {
    engine.build_type::<SplitBuilder>();
    engine.build_type::<Pane>();

    let direction_module = exported_module!(direction_enum_mod);
    engine.register_static_module("Direction", direction_module.into());
}
