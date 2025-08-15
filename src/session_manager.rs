use crate::config::Config;
use crate::directory_manager::DirectoryManager;
use crate::layout::Layout;
use crate::layout::LayoutManager;
use crate::layout::LayoutName;
use crate::script;
use crate::tmux::Session;
use crate::utils;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::WrapErr;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn layout_from_options(
    name: Option<String>,
    path: PathBuf,
    layout_mgr: &LayoutManager,
) -> Result<Layout> {
    let name = match name {
        Some(name) => LayoutName::try_new(name)?,
        None => LayoutName::try_from_path(&path, layout_mgr)?,
    };
    Ok(Layout::new(name))
}

// FIXME: will shit it self, when it tries to switch to a purely runtime session not a layout
struct LastSessionManager;

impl LastSessionManager {
    const LAST_SESSION_FILE: &'static str = "last_session";
    fn save(dir_mgr: &DirectoryManager, name: &str) -> Result<()> {
        let last_session_path = dir_mgr.cache_dir()?.join(Self::LAST_SESSION_FILE);
        fs::write(last_session_path, name).wrap_err("failed to save the last session")?;
        Ok(())
    }

    fn get(dir_mgr: &DirectoryManager) -> Result<Option<String>> {
        let last_session_path = dir_mgr.cache_dir()?.join(Self::LAST_SESSION_FILE);
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
    layout_mgr: LayoutManager,
    config: Arc<Config>,
    dir_mgr: Arc<DirectoryManager>,
}

impl SessionManager {
    pub fn new(config: Arc<Config>, dir_mgr: Arc<DirectoryManager>) -> Result<Self> {
        Ok(Self {
            config,
            layout_mgr: LayoutManager::new(dir_mgr.layouts_dir()?)?,
            dir_mgr,
        })
    }

    fn layout(&self, name: &str) -> Result<&Layout> {
        Ok(self
            .layout_mgr
            .layout(&name)
            .ok_or_eyre(format!("session not found: {}", name))?)
    }

    pub fn create(&mut self, name: Option<String>, path: PathBuf) -> Result<String> {
        let path = utils::expand_path(path)?;
        let layout = layout_from_options(name, path.clone(), &self.layout_mgr)?;
        let name = layout.tmux_name().to_owned();
        self.layout_mgr
            .create(layout, &path, &self.config, &self.dir_mgr.config_dir()?)
            .wrap_err("failed to create layout file")?;
        Ok(name) // TODO: maybe return a message
    }

    pub fn edit(&self, tmux_name: &str) -> Result<()> {
        self.layout_mgr.edit(tmux_name, &self.config)?;
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
        let last = LastSessionManager::get(&self.dir_mgr)?.ok_or_eyre("no last session saved")?;
        self.switch_core(&last)?;
        Ok(())
    }

    fn switch_core(&self, tmux_name: &str) -> Result<()> {
        let tmux_name = tmux_name.to_owned();
        let active_session = Session::active_name().wrap_err("failed to get active sesion")?;
        if Some(&tmux_name) == active_session.as_ref() {
            println!("info: session with that name is already attached. Aborting switch");
            return Ok(());
        }

        let running_sessions = Self::running_sessions(active_session.as_ref())?;
        LastSessionManager::save(&self.dir_mgr, &tmux_name)
            .wrap_err("failed to save session name for later use")?;
        if running_sessions.contains(&tmux_name) {
            let session = Session::from(&tmux_name)?;
            session.attach()?;
        } else {
            self.run(&tmux_name)?;
        }
        Ok(())
    }

    fn running_sessions(active_session: Option<&String>) -> Result<Vec<String>> {
        let running_sessions =
            Session::list_sessions().wrap_err("failed to get running sessions")?;
        Ok(running_sessions
            .into_iter()
            .filter(|s| Some(s) != active_session)
            .collect())
    }

    fn run(&self, tmux_name: &str) -> Result<()> {
        let layout = self.layout(tmux_name)?;
        script::run(layout, &self.dir_mgr.layouts_dir()?).wrap_err(format!(
            "an error occured while exucting the layout file: {tmux_name}"
        ))?;
        Ok(())
    }

    pub fn remove(&mut self, tmux_name: &str) -> Result<()> {
        self.layout_mgr
            .remove(tmux_name)
            .wrap_err_with(|| format!("failed to remove layout with name: {tmux_name}"))?;
        Ok(())
    }

    pub fn list(&self, options: ListSessionsOptions) -> Result<String> {
        Ok(list_sessions::run(&self.layout_mgr, options)?)
    }
}

mod list_sessions {
    use crate::layout::LayoutManager;
    use crate::tmux::Session;
    use color_eyre::Result;
    use itertools::Itertools;

    pub struct Options {
        pub tmux_format: bool,
        pub include_active: bool,
        pub exclude_running: bool,
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
    pub fn run(layout_mgr: &LayoutManager, opts: Options) -> Result<String> {
        let layouts = layout_mgr.list().into_iter().map(ToOwned::to_owned);
        let running_sessions = Session::list_sessions()?;
        let sessions = layouts.chain(running_sessions.clone().into_iter());
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
        Ok(sessions)
    }

    fn exclude(session_name: &str, info: &ExcludeInfo, opts: &Options) -> bool {
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
