use color_eyre::eyre::OptionExt;
use itertools::Itertools;
use nucleo::{Matcher, Nucleo, Utf32Str, Utf32String};
use ratatui::widgets::ListState;
use std::sync::Arc;

#[derive(PartialEq, Eq)]
pub enum HighlightState {
    Highlighted,
    NotHighlighted,
}

pub struct ItemHighlight {
    pub highlight_state: HighlightState,
    pub text: String,
}

pub type ItemHighlights = Vec<ItemHighlight>;

trait ItemHighlightsExt {
    fn from(item: &Utf32String, indicies: Vec<u32>) -> ItemHighlights;
}

impl ItemHighlightsExt for ItemHighlights {
    fn from(item: &Utf32String, indicies: Vec<u32>) -> ItemHighlights {
        match item {
            Utf32String::Ascii(element) => element
                .chars()
                .enumerate()
                .chunk_by(|(index, _)| highlight_state(&indicies, *index))
                .into_iter()
                .map(|(highlight_state, chunk)| ItemHighlight {
                    highlight_state,
                    text: chunk.into_iter().map(|(_, c)| c).collect::<String>(),
                })
                .collect(),
            Utf32String::Unicode(element) => element
                .iter()
                .enumerate()
                .chunk_by(|(index, _)| highlight_state(&indicies, *index))
                .into_iter()
                .map(|(highlight_state, chunk)| ItemHighlight {
                    highlight_state,
                    text: chunk.into_iter().map(|(_, c)| c).collect::<String>(),
                })
                .collect(),
        }
    }
}

fn highlight_state(indicies: &Vec<u32>, index: usize) -> HighlightState {
    if indicies.contains(&(index as u32)) {
        HighlightState::Highlighted
    } else {
        HighlightState::NotHighlighted
    }
}

pub struct FuzzyListModel<T: Send + Sync + 'static> {
    nucleo: Nucleo<T>,
    highlight_matcher: Matcher,
    state: ListState,
}

pub struct Item<T: Send + Sync + 'static> {
    pub data: T,
    pub haystack: Utf32String,
}

pub struct ItemView<'a, T: Send + Sync + 'static> {
    pub data: &'a T,
    pub haystack: Utf32Str<'a>,
}

impl<T: Send + Sync + 'static> FuzzyListModel<T> {
    pub fn new(items: Vec<Item<T>>) -> Self {
        let nucleo = Nucleo::<T>::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1);
        let injector = nucleo.injector();
        items.into_iter().for_each(|item| {
            injector.push(item.data, |_, dst| dst[0] = item.haystack);
        });

        Self {
            nucleo,
            state: ListState::default().with_selected(Some(0)),
            highlight_matcher: Matcher::new(nucleo::Config::DEFAULT),
        }
    }

    pub fn state(&self) -> ListState {
        self.state.clone()
    }

    pub fn select_first(&mut self) {
        self.state.select_first();
    }

    pub fn select_prev(&mut self) {
        self.state.select_previous();
    }

    pub fn select_next(&mut self) {
        self.state.select_next();
    }

    pub fn selected(&self) -> Option<ItemView<T>> {
        let item = self
            .nucleo
            .snapshot()
            .get_matched_item(self.state.selected()? as u32)?;
        Some(ItemView {
            haystack: item.matcher_columns[0].slice(..),
            data: item.data,
        })
    }

    pub fn tick(&mut self) {
        self.nucleo.tick(10);
    }

    pub fn snapshot(&self) -> &nucleo::Snapshot<T> {
        self.nucleo.snapshot()
    }

    pub fn update_pattern(&mut self, previous_input: &str, input: &str) {
        self.nucleo.pattern.reparse(
            0,
            input,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            input.starts_with(previous_input),
        );
    }

    pub fn items_highlights(&mut self) -> Vec<ItemHighlights> {
        let indicies: Vec<_> = self
            .nucleo
            .snapshot()
            .matched_items(..)
            .enumerate()
            .map(|(i, _)| i)
            .collect();
        indicies
            .iter()
            .map(|i| self.item_highlight(*i as u32))
            .collect()
    }

    fn item_highlight(&mut self, index: u32) -> ItemHighlights {
        let element = &self
            .nucleo
            .snapshot()
            .get_matched_item(index)
            .ok_or_eyre("Tried to get matched item at an index out of bounds of {index}")
            .unwrap()
            .matcher_columns[0];
        let mut indicies = Vec::new();
        let _ = self.nucleo.pattern.column_pattern(0).indices(
            element.slice(..),
            &mut self.highlight_matcher,
            &mut indicies,
        );
        <ItemHighlights as ItemHighlightsExt>::from(element, indicies)
    }
}
