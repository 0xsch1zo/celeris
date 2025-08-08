/*use crate::script::mlua::IntoInteropResExt;
use crate::script::pane::Pane;
use crate::script::session::Session;
use crate::tmux::{self, Direction};
use mlua::Result;
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

    fn root(&mut self, path: &str) -> Result<Self> {
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

    fn build(&mut self) -> Result<Window> {
        Ok(Window {
            inner: self.inner.lock().unwrap().build().into_interop()?,
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

    fn even_out(&mut self, direction: Direction) -> Result<()> {
        self.inner.event_out(direction).into_interop()?;
        Ok(())
    }

    fn select(&mut self) -> Result<()> {
        self.inner.select().into_interop()?;
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
}*/
