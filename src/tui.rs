use color_eyre::Result;
use crossterm::event::{self, Event};
use ratatui::{
    layout::Rect,
    prelude::*,
    text::{Line, Span},
    widgets::Paragraph,
};
use std::{io::Stdout, time::Duration};
use tui_input::{Input, backend::crossterm::EventHandler};

struct Model {
    search_bar: Input,
    prompt: String,
    running_state: RunningState,
}

impl Model {
    fn new() -> Self {
        Self {
            search_bar: Input::new(String::new()),
            prompt: "> ".into(),
            running_state: RunningState::Running,
        }
    }
}

#[derive(PartialEq, Eq)]
enum RunningState {
    Running,
    Done,
}

enum Message {
    Input(event::Event),
    Quit,
}

pub fn picker() -> Result<()> {
    let mut term = ratatui::init();
    let result = main_loop(&mut term);
    ratatui::restore();
    result
}

fn main_loop(term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut model = Model::new();
    while model.running_state != RunningState::Done {
        term.draw(|frame| {
            view(&model, frame);
        })?;

        if let Some(msg) = handle_events()? {
            update(&mut model, msg);
        }
    }
    Ok(())
}

fn handle_events() -> Result<Option<Message>> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(None);
    }

    if let Event::Key(key) = event::read()? {
        Ok(handle_key(key))
    } else {
        Ok(None)
    }
}

fn handle_key(key: event::KeyEvent) -> Option<Message> {
    match key.code {
        event::KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            Some(Message::Quit)
        }
        _ => Some(Message::Input(Event::Key(key))),
    }
}

//                 ↓ -- thanks tui-input
fn update(model: &mut Model, msg: Message) {
    match msg {
        Message::Quit => model.running_state = RunningState::Done,
        Message::Input(evt) => {
            let _ = model.search_bar.handle_event(&evt);
        }
    };
}

fn view(model: &Model, frame: &mut Frame) {
    let layout = picker_layout().split(frame.area());
    render_input(model, frame, layout[1]);
}

fn picker_layout() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Min(1), Constraint::Length(1)])
}

fn render_input(model: &Model, frame: &mut Frame, area: Rect) {
    let width = area.width.max(1 + model.prompt.len() as u16) - 1 - model.prompt.len() as u16; // acount for cursor
    let scroll = model.search_bar.visual_scroll(width.into());
    let input = Paragraph::new(Line::from(vec![
        Span::styled(&model.prompt, Style::default().fg(Color::Blue)),
        Span::from(model.search_bar.value()),
    ]))
    .scroll((0, scroll as u16));

    frame.render_widget(input, area);
    let x = model.search_bar.visual_cursor().max(scroll) - scroll + model.prompt.len();
    frame.set_cursor_position((area.x + x as u16, area.y + 1))
}
