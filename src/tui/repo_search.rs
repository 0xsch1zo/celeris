use crate::config::Config;
use crate::repos::search::search;
use crate::tui::{RunningState, SearchModel, SearchResults};
use color_eyre::Result;
use crossterm::event::{self, Event};
use nucleo::{Nucleo, Utf32String};
use ratatui::prelude::*;
use ratatui::widgets::{List, ListDirection, ListState};
use std::error::Error;
use std::fmt::Display;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{thread, time};
use throbber_widgets_tui as throbber;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

pub struct RepoModel {
    search_bar: Input,
    running_state: RunningState,
    search_state: SearchState,
    config: Arc<Config>,
}

impl RepoModel {
    const TICK_RATE: time::Duration = time::Duration::from_millis(85);

    pub fn new(config: Config) -> Self {
        Self {
            search_bar: Input::new(String::new()),
            running_state: RunningState::Running,
            search_state: SearchState::NotStarted,
            config: Arc::new(config),
        }
    }
}

enum SearchState {
    NotStarted,
    Running(throbber::ThrobberState, Rc<Receiver<SearchResults>>),
    Done(ListModel),
}

enum Message {
    Input(event::Event),
    Tick,
    StartSearch,
    SearchEnded(SearchResults),
    UpdateThrobber,
    SelectNext,
    SelectPrev,
    Quit,
}

#[derive(Debug)]
struct StateError(String);

impl Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "program is in a bad state {0}", self)
    }
}

impl Error for StateError {}

impl SearchModel for RepoModel {
    fn main_loop<T: Backend>(mut self, term: &mut Terminal<T>) -> Result<()> {
        if let SearchState::NotStarted = self.search_state {
            update(&mut self, Message::StartSearch)?;
        }

        while self.running_state != RunningState::Done {
            term.draw(|frame| {
                view(&mut self, frame); // because of stateful widgets
            })?;

            if let Some(msg) = handle_events()? {
                update(&mut self, msg)?;
            }

            if let SearchState::Running(_, ref rx) = self.search_state {
                match rx.try_recv() {
                    Ok(r) => update(&mut self, Message::SearchEnded(r))?,
                    Err(_) => {
                        update(&mut self, Message::UpdateThrobber)?;
                    }
                }
            }

            if let SearchState::Done(_) = self.search_state {
                update(&mut self, Message::Tick)?
            }
        }
        Ok(())
    }

    fn search_bar(&self) -> &Input {
        &self.search_bar
    }
}

struct ListModel {
    items: Vec<String>,
    nucleo: Nucleo<String>,
    state: ListState,
}

impl ListModel {
    fn new(results: Vec<String>) -> Self {
        let nucleo = Nucleo::<String>::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1);
        let injector = nucleo.injector();
        results.iter().for_each(|result| {
            injector.push(result.to_string(), |s, dst| {
                dst[0] = Utf32String::from(s as &str)
            });
        });

        Self {
            items: results,
            nucleo,
            state: ListState::default(),
        }
    }

    fn items(&self) -> &[String] {
        &self.items
    }

    fn select_first(&mut self) {
        self.state.select_first();
    }

    fn select_prev(&mut self) {
        self.state.select_previous();
    }

    fn select_next(&mut self) {
        self.state.select_next();
    }

    fn tick(&mut self) {
        self.nucleo.tick(10);
    }

    fn snapshot(&self) -> &nucleo::Snapshot<String> {
        self.nucleo.snapshot()
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

    if key.code.is_up() {
        return Some(Message::SelectNext);
    } else if key.code.is_down() {
        return Some(Message::SelectPrev);
    }

    match key.code {
        event::KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            Some(Message::Quit)
        }
        _ => Some(Message::Input(Event::Key(key))),
    }
}

fn update(model: &mut RepoModel, msg: Message) -> Result<()> {
    match msg {
        Message::Quit => model.running_state = RunningState::Done,
        Message::Input(evt) => {
            let _ = model.search_bar.handle_event(&evt);
        }
        Message::StartSearch => start_search(model),
        Message::SearchEnded(results) => {
            let mut list_model = ListModel::new(results?);
            list_model.select_first();
            model.search_state = SearchState::Done(list_model);
        }
        Message::UpdateThrobber => {
            if let SearchState::Running(ref mut throbber_state, _) = model.search_state {
                throbber_state.calc_next();
            } else {
                return Err(StateError(String::from(
                    "got message to update throbber when the search is not running",
                ))
                .into());
            }
        }
        Message::Tick => {
            if let SearchState::Done(ref mut list_model) = model.search_state {
                list_model.tick();
            } else {
                return Err(StateError(String::from("tick called when search is not done")).into());
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
    };
    Ok(())
}

fn view(model: &mut RepoModel, frame: &mut Frame) {
    let layout = layout().split(frame.area());

    match model.search_state {
        SearchState::Running(ref mut state, _) => render_throbber(frame, layout[1], state),
        SearchState::Done(ref mut list_model) => {
            render_list(frame, layout[0], list_model);
            render_item_counter(frame, layout[1], list_model)
        }
        _ => {}
    };
    super::render_input(model, frame, layout[2]);
}

fn render_list(frame: &mut Frame, area: Rect, list_model: &mut ListModel) {
    let elements = list_model
        .items()
        .iter()
        .map(|s| super::padding_str() + s)
        .collect::<Vec<_>>();
    let list = List::new(elements)
        .highlight_symbol("▌")
        .highlight_style(Style::default().bg(Color::Black).fg(Color::Yellow))
        .direction(ListDirection::BottomToTop);
    frame.render_stateful_widget(list, area, &mut list_model.state);
}

fn render_item_counter(frame: &mut Frame, area: Rect, list_model: &ListModel) {
    let snap = list_model.snapshot();
    let item_counter = Line::from(format!(
        "{}{}/{}",
        super::padding_str(),
        snap.matched_item_count(),
        snap.item_count(),
    ));
    frame.render_widget(item_counter, area);
}

fn render_throbber(frame: &mut Frame, area: Rect, state: &mut throbber::ThrobberState) {
    let throbber = throbber::Throbber::default()
        .label("Searching...")
        .style(ratatui::style::Style::default().add_modifier(Modifier::BOLD))
        .throbber_style(Style::default().fg(Color::Green))
        .throbber_set(throbber::BRAILLE_SIX)
        .use_type(throbber::WhichUse::Spin);
    frame.render_stateful_widget(throbber, area, state);
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
            let sessions: Vec<_> = repos.into_iter().map(|repo| repo.name).collect();
            tx.send(Ok(sessions))
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
    model.search_state = SearchState::Running(throbber::ThrobberState::default(), Rc::new(rx));
}
