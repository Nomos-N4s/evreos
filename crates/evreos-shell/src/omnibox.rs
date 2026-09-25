//! The combined entry field (omnibox) for search, history, and bookmarks.
//!
//! Under FR-003, a single entry field combines search, history, and bookmarks.
//! As the member types, suggestions are drawn exclusively from local data via
//! [`crate::suggest::SuggestionIndex`] (history, bookmarks, and open tabs).
//! Keystrokes in this field transmit nothing to any server (FR-007a, SC-006).
//!
//! Submitting a search is the one point where typed terms leave the machine
//! (FR-003a, FR-007a). Submission routes through [`evreos_net`] under
//! [`evreos_net::HistoryBearing::SubmittedSearch`], carrying only the terms the
//! member submitted and no identifiers, history, bookmarks, or visited addresses.

#![forbid(unsafe_code)]

use crate::store::bookmarks::BookmarkStore;
use crate::store::history::HistoryStore;
use crate::suggest::{OpenTab, Suggestion, SuggestionIndex};

/// Combined entry field (omnibox) state.
///
/// Holds the member's current input text, cursor position, and currently active
/// suggestions drawn locally from [`SuggestionIndex`].
#[derive(Debug, Clone, Default)]
pub struct Omnibox {
    /// The current text in the input field.
    text: String,
    /// The cursor position in unicode characters.
    cursor_position: usize,
    /// The current suggestions, if any, queried from the local `SuggestionIndex`.
    suggestions: Vec<Suggestion>,
    /// Currently highlighted suggestion index in `suggestions`, if any.
    selected_index: Option<usize>,
}

impl Omnibox {
    /// Create a new, empty omnibox.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current input text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the input text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Length of the input text in unicode characters.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// The cursor position in unicode characters.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Set the input text, repositioning the cursor at the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_position = self.text.chars().count();
        self.selected_index = None;
    }

    /// Insert text at the current cursor position.
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let char_indices: Vec<(usize, char)> = self.text.char_indices().collect();
        let byte_pos = if self.cursor_position >= char_indices.len() {
            self.text.len()
        } else {
            char_indices[self.cursor_position].0
        };

        self.text.insert_str(byte_pos, s);
        self.cursor_position += s.chars().count();
        self.selected_index = None;
    }

    /// Delete the character immediately preceding the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_position == 0 || self.text.is_empty() {
            return;
        }

        let char_indices: Vec<(usize, char)> = self.text.char_indices().collect();
        let remove_idx = self.cursor_position - 1;
        let byte_pos = char_indices[remove_idx].0;

        self.text.remove(byte_pos);
        self.cursor_position -= 1;
        self.selected_index = None;
    }

    /// Clear the input text, cursor position, and suggestions.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
        self.suggestions.clear();
        self.selected_index = None;
    }

    /// The list of current suggestions.
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }

    /// Set the suggestions directly (e.g. from an asynchronous worker lookup).
    pub fn set_suggestions(&mut self, suggestions: Vec<Suggestion>) {
        self.suggestions = suggestions;
        self.selected_index = None;
    }

    /// Query suggestions synchronously from local sources using [`SuggestionIndex`].
    ///
    /// Queries only local sources: open tabs, bookmarks, and history.
    /// Nothing is transmitted.
    pub fn query_suggestions(
        &mut self,
        history: &HistoryStore,
        bookmarks: &BookmarkStore,
        open_tabs: &[OpenTab],
    ) {
        let q = self.text.trim();
        if q.is_empty() {
            self.suggestions.clear();
            self.selected_index = None;
        } else {
            self.suggestions = SuggestionIndex::query(history, bookmarks, open_tabs, q);
            self.selected_index = None;
        }
    }

    /// The index of the currently selected suggestion in [`suggestions`], if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// The currently selected suggestion, if any.
    pub fn selected_suggestion(&self) -> Option<&Suggestion> {
        self.selected_index
            .and_then(|idx| self.suggestions.get(idx))
    }

    /// Select the next suggestion in the list, wrapping or selecting the first.
    pub fn select_next(&mut self) {
        if self.suggestions.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = match self.selected_index {
            None => Some(0),
            Some(idx) if idx + 1 < self.suggestions.len() => Some(idx + 1),
            Some(_) => Some(0),
        };
    }

    /// Select the previous suggestion in the list.
    pub fn select_prev(&mut self) {
        if self.suggestions.is_empty() {
            self.selected_index = None;
            return;
        }
        self.selected_index = match self.selected_index {
            None => Some(self.suggestions.len() - 1),
            Some(0) => Some(self.suggestions.len() - 1),
            Some(idx) => Some(idx - 1),
        };
    }

    /// Clear the suggestion selection.
    pub fn clear_selection(&mut self) {
        self.selected_index = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suggest::SuggestionSource;

    #[test]
    fn test_text_manipulation() {
        let mut box_state = Omnibox::new();
        assert!(box_state.is_empty());
        assert_eq!(box_state.len(), 0);
        assert_eq!(box_state.cursor_position(), 0);

        box_state.set_text("hello");
        assert_eq!(box_state.text(), "hello");
        assert_eq!(box_state.len(), 5);
        assert_eq!(box_state.cursor_position(), 5);

        box_state.insert_str(" world");
        assert_eq!(box_state.text(), "hello world");
        assert_eq!(box_state.cursor_position(), 11);

        box_state.backspace();
        assert_eq!(box_state.text(), "hello worl");
        assert_eq!(box_state.cursor_position(), 10);

        box_state.clear();
        assert!(box_state.is_empty());
        assert_eq!(box_state.cursor_position(), 0);
    }

    #[test]
    fn test_selection_cycling() {
        let mut box_state = Omnibox::new();
        box_state.set_suggestions(vec![
            Suggestion {
                address: "https://a.invalid".into(),
                title: "A".into(),
                source: SuggestionSource::OpenTab,
                score: 100,
            },
            Suggestion {
                address: "https://b.invalid".into(),
                title: "B".into(),
                source: SuggestionSource::Bookmark,
                score: 90,
            },
        ]);

        assert_eq!(box_state.selected_index(), None);
        box_state.select_next();
        assert_eq!(box_state.selected_index(), Some(0));
        assert_eq!(
            box_state.selected_suggestion().map(|s| s.address.as_str()),
            Some("https://a.invalid")
        );

        box_state.select_next();
        assert_eq!(box_state.selected_index(), Some(1));

        box_state.select_next();
        assert_eq!(box_state.selected_index(), Some(0));

        box_state.select_prev();
        assert_eq!(box_state.selected_index(), Some(1));

        box_state.clear_selection();
        assert_eq!(box_state.selected_index(), None);
    }
}
