/*
use crate::script::{ScriptFuncResult, eyre_to_rhai_err};
use crate::tmux;
use rhai::{CustomType, Engine, TypeBuilder};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
struct SessionBuilder {
    inner: Arc<Mutex<tmux::SessionBuilder>>,
    session_name: String,
}

impl SessionBuilder {
    fn new(session_name: &str) -> Self {
        let builder = tmux::SessionBuilder::new(session_name.to_owned());
        Self {
            inner: Arc::new(Mutex::new(builder)),
            session_name: session_name.to_owned(),
        }
    }

    fn root(&mut self, root: &str) -> ScriptFuncResult<Self> {
        let root = PathBuf::from(root);
        self.inner
            .lock()
            .unwrap()
            .root(root)
            .map_err(|e| eyre_to_rhai_err(e))?;
        Ok(self.clone())
    }

    fn build(&mut self) -> ScriptFuncResult<Session> {
        let tmux_session = self
            .inner
            .lock()
            .unwrap()
            .build()
            .map_err(|e| eyre_to_rhai_err(e))?;
        Ok(Session::new(tmux_session))
    }
}

// wrapper around tmux::Session
#[derive(Clone, Debug)]
pub struct Session {
    inner: Arc<tmux::Session>,
}

impl Session {
    fn new(tmux_session: Arc<tmux::Session>) -> Session {
        Session {
            inner: tmux_session,
        }
    }

    fn attach(&mut self) -> ScriptFuncResult<()> {
        self.inner.attach().map_err(|e| eyre_to_rhai_err(e))?;
        Ok(())
    }

    pub fn inner(&self) -> Arc<tmux::Session> {
        Arc::clone(&self.inner)
    }
}

impl CustomType for Session {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Session")
            .with_fn("attach", Session::attach);
    }
}

pub fn register(engine: &mut Engine, session_name: String) {
    engine.build_type::<Session>();
    //engine.build_type::<SessionBuilder>();
    engine.register_fn("Session", move || SessionBuilder::new(&session_name));
}*/
