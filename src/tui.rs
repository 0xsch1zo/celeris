mod fuzzy_list;
pub mod repo_search;
pub mod session_search;
use color_eyre::Result;
use fuzzy_list::{FuzzyListModel, HighlightState};
use ratatui::{
    layout::Rect,
    prelude::*,
    text::{Line, Span},
    widgets::Paragraph,
    widgets::{List, ListDirection},
};
use throbber_widgets_tui as throbber;
use tui_input::Input;

// TODO: don't use panics in here

// Abstraction over RepoModel and SessionModel
pub trait SearchModel {
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

fn render_list<T: Send + Sync + 'static>(
    frame: &mut Frame,
    area: Rect,
    list_model: &mut FuzzyListModel<T>,
) {
    let mut state = list_model.state().clone();
    let elements =
        list_model
            .items_highlights()
            .into_iter()
            .enumerate()
            .map(|(index, highlights)| {
                highlights
                    .into_iter()
                    .map(|highlight| match highlight.highlight_state {
                        HighlightState::Highlighted => Span::styled(
                            highlight.text,
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        HighlightState::NotHighlighted
                            if state.selected().is_some() && index == state.selected().unwrap() =>
                        {
                            Span::styled(
                                highlight.text,
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            )
                        }
                        HighlightState::NotHighlighted => Span::from(highlight.text),
                    })
                    .collect::<Line>()
            });
    let symbol = format!(
        "▌{}",
        std::iter::repeat_n(" ", std::cmp::max(PADDING, 1) - 1).collect::<String>()
    );
    let list = List::new(elements)
        .highlight_symbol(&symbol)
        .highlight_style(Style::default().bg(Color::Black))
        .repeat_highlight_symbol(true)
        .direction(ListDirection::BottomToTop);
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_item_counter<T: Send + Sync + 'static>(
    frame: &mut Frame,
    area: Rect,
    list_model: &FuzzyListModel<T>,
) {
    let snap = list_model.snapshot();
    let item_counter = Line::from(format!(
        "{}{}/{}",
        padding_str(),
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
