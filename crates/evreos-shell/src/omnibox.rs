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

use crate::brand::{Brand, SearchRequest, planned_search_request};
use crate::store::bookmarks::BookmarkStore;
use crate::store::history::HistoryStore;
use crate::suggest::{OpenTab, Suggestion, SuggestionIndex, SuggestionSource};

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

    /// Submit the current omnibox input or selected suggestion.
    ///
    /// - If a suggestion is selected:
    ///   - `OpenTab` -> [`OmniboxAction::SwitchToTab`]
    ///   - `Bookmark` / `History` -> [`OmniboxAction::Navigate`]
    /// - If no suggestion is selected and text is entered:
    ///   - If text matches a URL pattern -> [`OmniboxAction::Navigate`]
    ///   - Otherwise -> routes through `evreos_net` as a submitted search:
    ///     [`OmniboxAction::Search`]
    /// - If input is empty -> returns `None`.
    ///
    /// Upon submission, suggestions and selection are cleared.
    pub fn submit(&mut self, brand: &Brand) -> Option<OmniboxAction> {
        let action = if let Some(suggestion) = self.selected_suggestion() {
            match suggestion.source {
                SuggestionSource::OpenTab => Some(OmniboxAction::SwitchToTab {
                    address: suggestion.address.clone(),
                }),
                SuggestionSource::Bookmark | SuggestionSource::History => {
                    Some(OmniboxAction::Navigate {
                        url: suggestion.address.clone(),
                    })
                }
            }
        } else {
            let input = self.text.trim();
            if input.is_empty() {
                None
            } else if is_url_input(input) {
                Some(OmniboxAction::Navigate {
                    url: normalize_url(input),
                })
            } else {
                let (planned, request) = planned_search_request(brand, input);
                Some(OmniboxAction::Search(SubmittedSearch {
                    planned,
                    request,
                    terms: input.to_string(),
                }))
            }
        };

        if action.is_some() {
            self.suggestions.clear();
            self.selected_index = None;
        }

        action
    }
}

/// The outcome of submitting from the omnibox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmniboxAction {
    /// Navigate to a direct URL or a selected bookmark/history address.
    Navigate { url: String },
    /// Switch to an already open tab matching the suggestion.
    SwitchToTab { address: String },
    /// Submit a search query to the default search provider.
    Search(SubmittedSearch),
}

/// A submitted search request routed through `evreos-net`.
///
/// Under FR-003a and FR-007a, submitting a search is the one point where typed
/// terms leave the machine. The request carries ONLY the terms the member submitted,
/// and carries NO navigated address, NO page content, NO history or bookmark data,
/// and NO identifier Evreos assigns or persists across searches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedSearch {
    /// The planned request routed through `evreos-net` with `SubmittedSearch` purpose.
    pub planned: evreos_net::PlannedRequest,
    /// The brand search request carrying the search endpoint and formatted query.
    pub request: SearchRequest,
    /// The raw terms the member submitted.
    pub terms: String,
}

impl SubmittedSearch {
    /// The raw search terms submitted by the member.
    pub fn terms(&self) -> &str {
        &self.terms
    }

    /// The planned request through `evreos-net`.
    pub fn planned(&self) -> &evreos_net::PlannedRequest {
        &self.planned
    }

    /// The brand search request.
    pub fn request(&self) -> &SearchRequest {
        &self.request
    }

    /// Verify that the query strictly carries only the encoded search terms
    /// (`q=<terms>`) and no extraneous tracking, identifier, or history fields.
    pub fn carries_only_terms(&self) -> bool {
        self.request.query.starts_with("q=") && !self.request.query.contains('&')
    }
}

/// Heuristically determine whether the given input string represents a direct URL or address.
pub fn is_url_input(input: &str) -> bool {
    let s = input.trim();
    if s.is_empty() || s.chars().any(char::is_whitespace) {
        return false;
    }
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("about:")
        || s.starts_with("file://")
        || s.starts_with("data:")
    {
        return true;
    }
    let host_part = s
        .split('/')
        .next()
        .unwrap_or(s)
        .split(':')
        .next()
        .unwrap_or(s);
    if host_part == "localhost" {
        return true;
    }
    if let Some(dot_idx) = host_part.find('.') {
        if dot_idx > 0 && dot_idx < host_part.len() - 1 {
            return host_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-');
        }
    }
    false
}

/// Normalize an address input into a full URL.
pub fn normalize_url(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("about:")
        || s.starts_with("file://")
        || s.starts_with("data:")
    {
        s.to_string()
    } else {
        format!("https://{s}")
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

    #[test]
    fn test_url_detection() {
        assert!(is_url_input("https://example.com"));
        assert!(is_url_input("http://insecure.org/path"));
        assert!(is_url_input("about:blank"));
        assert!(is_url_input("file:///tmp/doc.txt"));
        assert!(is_url_input("localhost"));
        assert!(is_url_input("localhost:8080"));
        assert!(is_url_input("crates.io"));
        assert!(is_url_input("example.com/test"));

        assert!(!is_url_input(""));
        assert!(!is_url_input("   "));
        assert!(!is_url_input("hello world"));
        assert!(!is_url_input("search term"));
        assert!(!is_url_input("what is rust"));
    }

    #[test]
    fn test_submit_search_routes_through_evreos_net() {
        let brand = crate::brand::brand();
        let mut box_state = Omnibox::new();
        box_state.set_text("rust programming language");

        let action = box_state.submit(brand).expect("submission produces action");
        match action {
            OmniboxAction::Search(sub) => {
                assert_eq!(sub.terms(), "rust programming language");
                assert_eq!(sub.request().query, "q=rust%20programming%20language");
                assert!(sub.carries_only_terms());
                assert_eq!(
                    sub.planned().purpose(),
                    &evreos_net::Purpose::HistoryBearing(
                        evreos_net::HistoryBearing::SubmittedSearch
                    )
                );
            }
            other => panic!("expected search action, got {other:?}"),
        }

        // Suggestions and selection are cleared after submission
        assert!(box_state.suggestions().is_empty());
        assert_eq!(box_state.selected_index(), None);
    }

    #[test]
    fn test_submit_direct_url_navigates() {
        let brand = crate::brand::brand();
        let mut box_state = Omnibox::new();
        box_state.set_text("example.com/path");

        let action = box_state.submit(brand).expect("action");
        assert_eq!(
            action,
            OmniboxAction::Navigate {
                url: "https://example.com/path".into(),
            }
        );
    }

    #[test]
    fn test_submit_selected_suggestion_routes_to_tab_or_nav() {
        let brand = crate::brand::brand();
        let mut box_state = Omnibox::new();
        box_state.set_text("unused query");
        box_state.set_suggestions(vec![
            Suggestion {
                address: "https://open-tab.invalid".into(),
                title: "Tab".into(),
                source: SuggestionSource::OpenTab,
                score: 100,
            },
            Suggestion {
                address: "https://bookmarked.invalid".into(),
                title: "Bookmark".into(),
                source: SuggestionSource::Bookmark,
                score: 90,
            },
        ]);

        box_state.select_next(); // selects OpenTab
        let action1 = box_state.submit(brand).expect("action");
        assert_eq!(
            action1,
            OmniboxAction::SwitchToTab {
                address: "https://open-tab.invalid".into(),
            }
        );

        box_state.set_suggestions(vec![Suggestion {
            address: "https://bookmarked.invalid".into(),
            title: "Bookmark".into(),
            source: SuggestionSource::Bookmark,
            score: 90,
        }]);
        box_state.select_next(); // selects Bookmark
        let action2 = box_state.submit(brand).expect("action");
        assert_eq!(
            action2,
            OmniboxAction::Navigate {
                url: "https://bookmarked.invalid".into(),
            }
        );
    }
}
