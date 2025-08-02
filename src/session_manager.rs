use crate::config::Config;
use crate::directory_manager::DirectoryManager;
use crate::manifest;
use crate::manifest::Manifest;
use crate::script::ScriptManager;
use crate::tmux::Session;
use crate::utils;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::WrapErr;
use itertools::Itertools;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

// TODO: make project mamagner which encompasses manifest and script manager
#[derive(Clone)]
pub enum Name {
    Deduced,
    Custom(String),
}

#[derive(Clone)]
pub struct SessionProperties {
    pub name: Name,
    pub path: PathBuf,
}

impl SessionProperties {
    pub fn new(name: Name, path: PathBuf) -> Self {
        Self { name, path }
    }

    pub fn from(name: Option<String>, path: PathBuf) -> Self {
        let name = match name {
            Some(name) => Name::Custom(name),
            None => Name::Deduced,
        };
        Self { name, path }
    }

    fn name(&self, manifest: &Manifest) -> Result<String> {
        Ok(match &self.name {
            Name::Custom(name) => name.to_owned(),
            Name::Deduced => manifest.deduce_name(&self.path).wrap_err(format!(
                "failed to deduce name of session with path: {}",
                self.path.display()
            ))?,
        })
    }
}

struct LastSessionManger {
    dir_mgr: Rc<DirectoryManager>,
}

impl LastSessionManger {
    fn new(dir_mgr: Rc<DirectoryManager>) -> Self {
        Self { dir_mgr }
    }
    const LAST_SESSION_FILE: &'static str = "last_session";
    fn save(&self, name: &str) -> Result<()> {
        let last_session_path = self.dir_mgr.cache_dir()?.join(Self::LAST_SESSION_FILE);
        fs::write(last_session_path, name).wrap_err("failed to save the last session")?;
        Ok(())
    }

    fn get(&self) -> Result<Option<String>> {
        let last_session_path = self.dir_mgr.cache_dir()?.join(Self::LAST_SESSION_FILE);
        if !last_session_path.exists() {
            return Ok(None);
        }
        Ok(Some(
            fs::read_to_string(last_session_path)
                .wrap_err("failed to retrieve saved last session")?,
        ))
    }
}

pub enum SwitchTarget {
    LastSession,
    Session(String),
}

pub use list_sessions::Options as ListSessionsOptions;

pub struct SessionManager {
    manifest: Manifest,
    config: Rc<Config>,
    last_session_mgr: LastSessionManger,
    script_mgr: ScriptManager,
}

// TODO: the program can be in a weird state when it it errors out during an action that get's
// saved figure out if somethig can be done
impl SessionManager {
    pub fn new(config: Rc<Config>, dir_mgr: Rc<DirectoryManager>) -> Result<Self> {
        Ok(Self {
            manifest: Manifest::new(Rc::clone(&dir_mgr))?,
            config,
            last_session_mgr: LastSessionManger::new(Rc::clone(&dir_mgr)),
            script_mgr: ScriptManager::new(dir_mgr),
        })
    }

    fn entry(&self, name: &str) -> Result<&manifest::Entry> {
        Ok(self
            .manifest
            .entry(&name)
            .ok_or_eyre(format!("session not found: {}", name))?)
    }

    pub fn create(&mut self, mut props: SessionProperties) -> Result<()> {
        props.path = utils::expand_path(props.path)?;
        let name = props.name(&self.manifest)?;
        let entry = manifest::Entry::new(name.clone(), props.path);
        self.manifest
            .push(entry)
            .wrap_err("failed to add session")?;
        let entry = self.entry(&name)?; // only ref
        self.script_mgr.create(entry).wrap_err(format!(
            "failed to create script for session with name: {}",
            name
        ))?;
        self.script_mgr.edit(entry, &self.config)?;
        Ok(())
    }

    pub fn edit(&self, name: &str) -> Result<()> {
        let entry = self.entry(name)?;
        self.script_mgr.edit(entry, &self.config)?;
        Ok(())
    }

    pub fn switch(&self, target: SwitchTarget) -> Result<()> {
        match target {
            SwitchTarget::LastSession => self.switch_last()?,
            SwitchTarget::Session(name) => self.switch_core(&name)?,
        }
        Ok(())
    }

    fn switch_last(&self) -> Result<()> {
        let last = self
            .last_session_mgr
            .get()?
            .ok_or_eyre("no last session saved")?;
        self.switch_core(&last)?;
        Ok(())
    }

    fn switch_core(&self, name: &str) -> Result<()> {
        let name = name.to_owned();
        let active_session = Session::active_name().wrap_err("failed to get active sesion")?;
        if Some(&name) == active_session.as_ref() {
            println!("info: session with that name is already attached. Aborting switch");
            return Ok(());
        }

        let running_sessions =
            Session::list_sessions().wrap_err("failed to get running sessions")?;
        let running_sessions = running_sessions
            .into_iter()
            .filter(|s| Some(s) != active_session.as_ref())
            .collect_vec();
        self.last_session_mgr
            .save(&name)
            .wrap_err("failed to save session name for later use")?;
        if running_sessions.contains(&name) {
            let session = Session::from(&name)?;
            session.attach()?;
        } else {
            self.run(&name)?;
        }
        Ok(())
    }

    fn run(&self, name: &str) -> Result<()> {
        let entry = self.entry(name)?;
        self.script_mgr.run(entry).wrap_err("script error")?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.manifest.contains(name)
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        let entry = self.entry(name)?;
        self.script_mgr.remove(entry)?;
        self.manifest
            .remove(name)
            .wrap_err("failed to remove session")?;
        Ok(())
    }

    pub fn list(&self, options: ListSessionsOptions) -> Result<()> {
        list_sessions::run(&self.manifest, options)?;
        Ok(())
    }
}

mod list_sessions {
    use std::io::{self, Write};

    use crate::manifest::Manifest;
    use crate::tmux::Session;
    use color_eyre::Result;
    use color_eyre::eyre::Context;
    use itertools::Itertools;

    pub struct Options {
        pub tmux_format: bool,
        pub include_active: bool,
        pub exclude_running: bool,
        pub only_running: bool,
    }

    struct ExcludeInfo {
        running_sessions: Vec<String>,
        active_session: Option<String>,
    }

    impl ExcludeInfo {
        fn new(running_sessions: Vec<String>, active_session: Option<String>) -> Self {
            Self {
                running_sessions,
                active_session,
            }
        }
    }

    // TODO: make a good interface for the functionality
    pub fn run(manifest: &Manifest, opts: Options) -> Result<()> {
        let manifest_sessions = manifest.list().into_iter().map(ToOwned::to_owned);
        let running_sessions = Session::list_sessions()?;
        let sessions = manifest_sessions.chain(running_sessions.clone().into_iter());
        let active_session = Session::active_name()?;

        let exclude_info = ExcludeInfo::new(running_sessions, active_session.clone());
        let sessions = sessions
            .filter(|name| exclude(name, &exclude_info, &opts))
            .map(|session| match session {
                active if active_session.as_ref() == Some(&session) => format!("{active}*"),
                _ => session,
            })
            .collect_vec();
        let sessions = sessions
            .into_iter()
            .sorted()
            .dedup()
            .join(match opts.tmux_format {
                true => " ",
                false => "\n",
            });
        io::stdout()
            .write_all(sessions.as_bytes())
            .wrap_err("failed to write sessions to stdout")?;
        Ok(())
    }

    fn exclude(session_name: &str, info: &ExcludeInfo, opts: &Options) -> bool {
        if opts.only_running {
            return info.running_sessions.contains(&session_name.to_owned());
        }

        if !opts.include_active
            && info.active_session.is_some()
            && session_name == info.active_session.as_ref().unwrap()
        {
            return false;
        }

        if opts.exclude_running && info.running_sessions.contains(&session_name.to_owned()) {
            return false;
        }

        true
    }
}
