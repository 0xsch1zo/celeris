use crate::script::pane::Pane;
use crate::script::session::Session;
use crate::script::{self, ScriptFuncResult};
use crate::tmux::{self, Direction};
use rhai::{CustomType, Engine, TypeBuilder};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct WindowBuilder {
    inner: Arc<Mutex<tmux::WindowBuilder>>,
}

impl WindowBuilder {
    fn new(session: Session) -> Self {
        WindowBuilder {
            inner: Arc::new(Mutex::new(tmux::WindowBuilder::new(session.inner()))),
        }
    }

    fn name(&mut self, name: &str) -> Self {
        self.inner.lock().unwrap().name(name.to_owned());
        self.clone()
    }

    fn root(&mut self, path: &str) -> ScriptFuncResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("{path:?} does not exist").into());
        }
        self.inner.lock().unwrap().root(path);
        Ok(self.clone())
    }

    fn command(&mut self, command: &str) -> Self {
        self.inner.lock().unwrap().shell_command(command.to_owned());
        self.clone()
    }

    fn build(&mut self) -> ScriptFuncResult<Window> {
        Ok(Window {
            inner: self
                .inner
                .lock()
                .unwrap()
                .build()
                .map_err(|e| script::eyre_to_rhai_err(e))?,
        })
    }
}

impl CustomType for WindowBuilder {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("WindowBuilder")
            .with_fn("Window", WindowBuilder::new)
            .with_fn("name", WindowBuilder::name)
            .with_fn("root", WindowBuilder::root)
            .with_fn("raw_command", WindowBuilder::command)
            .with_fn("build", WindowBuilder::build);
    }
}

#[derive(Clone, Debug)]
pub struct Window {
    inner: Arc<tmux::Window>,
}

impl Window {
    fn default_pane(&mut self) -> Pane {
        Pane::new(Arc::clone(&self.inner.default_pane()))
    }

    fn even_out(&mut self, direction: Direction) -> ScriptFuncResult<()> {
        self.inner
            .event_out(direction)
            .map_err(|e| script::eyre_to_rhai_err(e))?;
        Ok(())
    }

    fn select(&mut self) -> ScriptFuncResult<()> {
        self.inner
            .select()
            .map_err(|e| script::eyre_to_rhai_err(e))?;
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

pub fn register(engine: &mut Engine) {
    engine.build_type::<WindowBuilder>();
    engine.build_type::<Window>();
}
