use crate::config::Config;
use crate::manifest::Manifest;
use crate::repos::{Repo, search::search};
use crate::script_manager;
use crate::tui::{
    SearchModel,
    fuzzy_list::{FuzzyListModel, Item},
};
use color_eyre::Result;
use color_eyre::eyre::Context;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use nucleo::Utf32String;
use ratatui::prelude::*;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::Display;
use std::io::stdout;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{thread, time};
use throbber_widgets_tui as throbber;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

type SearchResults = Result<Vec<Repo>>;

pub struct RepoModel {
    manifest: Manifest,
    search_bar: Input,
    running_state: RunningState,
    search_state: SearchState,
    config: Arc<Config>,
}

impl RepoModel {
    const TICK_RATE: time::Duration = time::Duration::from_millis(85);

    pub fn new(config: Config) -> Self {
        Self {
            manifest: Manifest::new().unwrap(),
            search_bar: Input::new(String::new()),
            running_state: RunningState::Running,
            search_state: SearchState::NotStarted,
            config: Arc::new(config),
        }
    }
}

//#[derive(PartialEq, Eq)]
enum RunningState {
    Running,
    Editor(RefCell<Repo>),
    Done,
}

enum SearchState {
    NotStarted,
    Running(
        Rc<RefCell<throbber::ThrobberState>>,
        Rc<Receiver<SearchResults>>,
    ),
    Done(FuzzyListModel<Repo>),
}

enum Message {
    Input(event::Event),
    NucleoTick,
    StartSearch,
    SearchEnded(SearchResults),
    UpdateThrobber(Rc<RefCell<throbber::ThrobberState>>),
    SelectNext,
    SelectPrev,
    Selected,
    Quit,
}

#[derive(Debug)]
struct StateError;

impl Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "program is in a bad state: {0}", self)
    }
}

impl Error for StateError {}

impl SearchModel for RepoModel {
    fn main_loop<T: Backend>(mut self, term: &mut Terminal<T>) -> Result<()> {
        if let SearchState::NotStarted = self.search_state {
            update(&mut self, Message::StartSearch)?;
        }

        while let RunningState::Running = self.running_state {
            term.draw(|frame| {
                view(&mut self, frame); // because of stateful widgets
            })?;

            if let Some(msg) = handle_events()? {
                update(&mut self, msg)?;
            }

            if let SearchState::Running(ref throbber_state, ref rx) = self.search_state {
                match rx.try_recv() {
                    Ok(r) => update(&mut self, Message::SearchEnded(r))?,
                    Err(_) => {
                        let throbber_state = Rc::clone(throbber_state);
                        update(&mut self, Message::UpdateThrobber(throbber_state))?;
                    }
                }
            }

            if let SearchState::Done(_) = self.search_state {
                update(&mut self, Message::NucleoTick)?
            }

            if let RunningState::Editor(ref repo) = self.running_state {
                let repo = repo.borrow().clone();
                editor_mode(&mut self, repo, term)?;
                update(&mut self, Message::Quit)?;
            }
        }
        Ok(())
    }

    fn search_bar(&self) -> &Input {
        &self.search_bar
    }
}

fn handle_events() -> Result<Option<Message>> {
    if !event::poll(RepoModel::TICK_RATE)? {
        return Ok(None);
    }

    if let Event::Key(key) = event::read()? {
        Ok(handle_key(key))
    } else {
        Ok(None)
    }
}

fn handle_key(key: event::KeyEvent) -> Option<Message> {
    if !key.is_press() {
        return None;
    }

    match key.code {
        event::KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            Some(Message::Quit)
        }
        event::KeyCode::Up => Some(Message::SelectNext),
        event::KeyCode::Down => Some(Message::SelectPrev),
        event::KeyCode::Enter => Some(Message::Selected),
        _ => Some(Message::Input(Event::Key(key))),
    }
}

fn update(model: &mut RepoModel, msg: Message) -> Result<()> {
    match msg {
        Message::Quit => model.running_state = RunningState::Done,
        Message::Input(evt) => {
            let prev = model.search_bar.value().to_string();
            let changed = model.search_bar.handle_event(&evt);
            if changed.is_some_and(|c| c.value) {
                if let SearchState::Done(ref mut list_model) = model.search_state {
                    list_model.update_pattern(&prev, model.search_bar.value());
                }
            }
        }
        Message::StartSearch => start_search(model),
        Message::SearchEnded(results) => {
            let items = results?
                .into_iter()
                .map(|result| Item::<Repo> {
                    haystack: Utf32String::from(result.name.clone()),
                    data: result,
                })
                .collect();
            model.search_state = SearchState::Done(FuzzyListModel::new(items));
        }
        Message::UpdateThrobber(throbber_state) => {
            throbber_state.borrow_mut().calc_next();
        }
        Message::NucleoTick => {
            if let SearchState::Done(ref mut list_model) = model.search_state {
                list_model.tick();
            } else {
                return Err(StateError).wrap_err("tick called when search is not done");
            }
        }
        Message::SelectPrev => {
            if let SearchState::Done(ref mut list_model) = model.search_state {
                list_model.select_prev();
            }
        }
        Message::SelectNext => {
            if let SearchState::Done(ref mut list_model) = model.search_state {
                list_model.select_next();
            }
        }
        Message::Selected => {
            if let SearchState::Done(ref list_model) = model.search_state {
                match list_model.selected() {
                    Some(item) => {
                        model.running_state = RunningState::Editor(RefCell::new(item.data.clone()));
                    }
                    _ => {}
                }
            }
        }
    };
    Ok(())
}

fn editor_mode<T: Backend>(
    model: &mut RepoModel,
    item: Repo,
    term: &mut Terminal<T>,
) -> Result<()> {
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    script_manager::edit_script(&mut model.manifest, item.into())?;
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    term.clear()?;
    Ok(())
}

fn view(model: &mut RepoModel, frame: &mut Frame) {
    let layout = layout().split(frame.area());

    match model.search_state {
        SearchState::Running(ref state, _) => {
            super::render_throbber(frame, layout[1], &mut state.borrow_mut())
        }
        SearchState::Done(ref mut list_model) => {
            super::render_list(frame, layout[0], list_model);
            super::render_item_counter(frame, layout[1], list_model)
        }
        _ => {}
    };
    super::render_input(model, frame, layout[2]);
}

fn layout() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
}

fn fetch_results(tx: Sender<SearchResults>, config: &Config) {
    let _ = search(&config)
        .and_then(|repos| {
            tx.send(Ok(repos))
                .unwrap_or_else(|e| panic!("failed to send search results: {e}"));
            Ok(())
        })
        .or_else(|e| -> Result<(), ()> {
            let e_str = e.to_string();
            tx.send(Err(e)).unwrap_or_else(|send_error| {
                panic!("failed to send search error: {e_str}, because: {send_error}")
            });
            Ok(())
        });
}

fn start_search(model: &mut RepoModel) {
    let (tx, rx) = mpsc::channel::<SearchResults>();
    let config = model.config.clone();
    thread::spawn(move || fetch_results(tx, &config));
    model.search_state = SearchState::Running(
        Rc::new(RefCell::new(throbber::ThrobberState::default())),
        Rc::new(rx),
    );
}
