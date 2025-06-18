use crossterm::event::{self, Event};
//use ratatui::{DefaultTerminal, Frame};
use crate::sessions::Sessions;
use std::error::Error;
use std::io;

struct State {
    sessions: Sessions,
}

pub fn picker() -> Result<(), Box<dyn Error>> {
    let mut term = ratatui::init();
    loop {
        term.draw(|frame| {
            frame.render_widget("hello wr", frame.area());
        })?;
        if matches!(event::read()?, Event::Key(_)) {
            break;
        }
    }

    Ok(ratatui::restore())
}

