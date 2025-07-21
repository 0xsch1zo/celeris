use crate::script::ScriptFuncResult;
use crate::tmux;
use rhai::{CustomType, Engine, FuncRegistration, Module, TypeBuilder};
use std::sync::Arc;

// wrapper around tmux::Session
#[derive(Clone, Debug)]
pub struct Session {
    inner: Arc<tmux::Session>,
}

// TODO: figure out what would be an idiomatic constructor
impl Session {
    pub fn new(tmux_session: Arc<tmux::Session>) -> ScriptFuncResult<Session> {
        Ok(Session {
            inner: tmux_session,
        })
    }

    pub fn inner(&self) -> Arc<tmux::Session> {
        Arc::clone(&self.inner)
    }
}

impl CustomType for Session {
    fn build(mut builder: TypeBuilder<Self>) {
        builder.with_name("Session");
    }
}

pub fn register(engine: &mut Engine, session: Arc<tmux::Session>) {
    engine.build_type::<Session>();
    let mut session_module = Module::new();
    FuncRegistration::new("build")
        .in_internal_namespace()
        .set_into_module(&mut session_module, move || {
            Session::new(Arc::clone(&session))
        });

    engine.register_static_module("Session", session_module.into());
}
