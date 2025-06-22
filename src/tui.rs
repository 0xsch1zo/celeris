pub mod repo_search;
pub mod session_search;
use color_eyre::Result;
use ratatui::{
    layout::Rect,
    prelude::*,
    text::{Line, Span},
    widgets::Paragraph,
};
use tui_input::Input;

type SearchResults = Result<Vec<String>>;

// Abstraction over RepoModel and SessionModel
trait SearchModel {
    fn main_loop<T: Backend>(self, term: &mut Terminal<T>) -> Result<()>;
    fn search_bar(&self) -> &Input;
}

#[derive(PartialEq, Eq)]
enum RunningState {
    Running,
    Done,
}

pub fn picker<T: SearchModel>(model: T) -> Result<()> {
    let mut term = ratatui::init();
    let result = model.main_loop(&mut term);
    ratatui::restore();
    result
}

const PROMPT: &'static str = "> ";
const PADDING: usize = PROMPT.len();

fn padding_str() -> String {
    std::iter::repeat_n(" ", PADDING).collect::<String>()
}

fn render_input<T: SearchModel>(model: &T, frame: &mut Frame, area: Rect) {
    let width = area.width.max(1 + PADDING as u16) - 1 - PADDING as u16; // acount for cursor
    let scroll = model.search_bar().visual_scroll(width.into());
    let input = Paragraph::new(Line::from(vec![
        Span::styled(PROMPT.to_string(), Style::default().fg(Color::Blue)),
        Span::from(model.search_bar().value()),
    ]))
    .scroll((0, scroll as u16));

    frame.render_widget(input, area);
    let x = model.search_bar().visual_cursor().max(scroll) - scroll + PADDING;
    frame.set_cursor_position((area.x + x as u16, area.y + 1))
}
