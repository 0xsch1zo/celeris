use crate::config::Config;
use crate::manifest;
use crate::tmux::{self, Direction};
use crate::utils;
use color_eyre::eyre::{self, Context};
use rhai::{
    CustomType, Engine, EvalAltResult, FuncRegistration, Module, TypeBuilder, export_module,
    exported_module,
};
use std::env;
use std::fs;
use std::process;
use std::sync::Arc;

macro_rules! create_enum_module {
    ($module:ident : $typ:ty => $($variant:ident),+) => {
        #[export_module]
        pub mod $module {
            $(
                #[allow(non_upper_case_globals)]
                pub const $variant: $typ = <$typ>::$variant;
            )*
        }
    };
}

create_enum_module! { direction_enum_mod: Direction => Vertical, Horizontal }

fn eyre_to_rhai_err(error: eyre::Report) -> Box<EvalAltResult> {
    error.to_string().into()
}

// wrapper around tmux::Session
#[derive(Clone, Debug)]
struct Session {
    tmux_session: Arc<tmux::Session>,
}

impl Session {
    fn new(tmux_session: &Arc<tmux::Session>) -> Result<Session, Box<EvalAltResult>> {
        Ok(Session {
            tmux_session: Arc::clone(tmux_session),
        })
    }

    fn new_window(&mut self) -> Result<Window, Box<EvalAltResult>> {
        Ok(Window {
            tmux_window: self
                .tmux_session
                .new_window(None, None)
                .map_err(|e| eyre_to_rhai_err(e))?,
        })
    }

    fn new_window_named(&mut self, name: &str) -> Result<Window, Box<EvalAltResult>> {
        Ok(Window {
            tmux_window: self
                .tmux_session
                .new_window(Some(name), None)
                .map_err(|e| eyre_to_rhai_err(e))?,
        })
    }
}

impl CustomType for Session {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Session")
            .with_fn("new_window", Session::new_window)
            .with_fn("new_window", Session::new_window_named);
    }
}

#[derive(Clone, Debug)]
struct Window {
    tmux_window: Arc<tmux::Window>,
}

impl Window {
    fn default_pane(&mut self) -> Pane {
        Pane {
            tmux_pane: Arc::clone(&self.tmux_window.default_pane()),
        }
    }

    fn even_out(&mut self, direction: Direction) -> Result<(), Box<EvalAltResult>> {
        self.tmux_window
            .event_out(direction)
            .map_err(|e| eyre_to_rhai_err(e))?;
        Ok(())
    }

    fn select(&mut self) -> Result<(), Box<EvalAltResult>> {
        self.tmux_window.select().map_err(|e| eyre_to_rhai_err(e))?;
        Ok(())
    }
}

impl CustomType for Window {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Window")
            .with_fn("default_pane", Window::default_pane)
            .with_fn("even_out", Window::even_out)
            .with_fn("select", Window::select);
    }
}

#[derive(Clone, Debug)]
struct Pane {
    tmux_pane: Arc<tmux::Pane>,
}

impl Pane {
    fn split(&mut self, direction: Direction) -> Result<Self, Box<EvalAltResult>> {
        Ok(Pane {
            tmux_pane: Arc::new(
                self.tmux_pane
                    .split(direction)
                    .map_err(|e| eyre_to_rhai_err(e))?,
            ),
        })
    }

    // TODO: Maybe use an enum here
    fn split_with_percentage(
        &mut self,
        direction: Direction,
        percentage: u8,
    ) -> Result<Self, Box<EvalAltResult>> {
        Ok(Pane {
            tmux_pane: Arc::new(
                self.tmux_pane
                    .split_with_size(direction, tmux::SplitSize::Percentage(percentage))
                    .map_err(|e| eyre_to_rhai_err(e))?,
            ),
        })
    }

    fn split_with_size(
        &mut self,
        direction: Direction,
        size: u32,
    ) -> Result<Self, Box<EvalAltResult>> {
        Ok(Pane {
            tmux_pane: Arc::new(
                self.tmux_pane
                    .split_with_size(direction, tmux::SplitSize::Fixed(size))
                    .map_err(|e| eyre_to_rhai_err(e))?,
            ),
        })
    }

    fn select(&mut self) -> Result<(), Box<EvalAltResult>> {
        self.tmux_pane.select().map_err(|e| eyre_to_rhai_err(e))?;
        Ok(())
    }

    fn run_command(&mut self, command: &str) -> Result<(), Box<EvalAltResult>> {
        self.tmux_pane
            .run_command(command)
            .map_err(|e| eyre_to_rhai_err(e))?;
        Ok(())
    }
}

impl CustomType for Pane {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Pane")
            .with_fn("split", Pane::split)
            .with_fn("split", Pane::split_with_percentage)
            .with_fn("split_fixed_size", Pane::split_with_size)
            .with_fn("select", Pane::select)
            .with_fn("run_command", Pane::run_command);
    }
}

pub fn run(entry: &manifest::Entry) -> eyre::Result<()> {
    let script = fs::read_to_string(entry.session_path()).wrap_err("session script not found")?;
    let mut engine = Engine::new();
    engine.build_type::<Session>();
    engine.build_type::<Window>();
    engine.build_type::<Pane>();

    let tmux_session = tmux::Session::new(entry.name()).map_err(|e| eyre_to_rhai_err(e))?;
    let script_session = Arc::clone(&tmux_session); // a copy just for build
    let mut session_module = Module::new();
    FuncRegistration::new("build")
        .in_internal_namespace()
        .set_into_module(&mut session_module, move || Session::new(&script_session));

    engine.register_static_module("Session", session_module.into());

    let direction_module = exported_module!(direction_enum_mod);
    engine.register_static_module("Direction", direction_module.into());

    engine.run(&script)?;
    tmux_session.attach()?;
    Ok(())
}

pub fn edit(entry: &manifest::Entry, config: &Config) -> eyre::Result<()> {
    let editor = match &config.editor {
        Some(editor) => editor,
        None => &env::var("EDITOR").wrap_err("$EDITOR is not set nor set in the config")?,
    };

    process::Command::new(editor)
        .arg(utils::path_to_string(entry.session_path())?)
        .status()?;
    Ok(())
}
