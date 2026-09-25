//! Suggestion index built strictly from local browser state.
//!
//! # Architecture and Invariants
//!
//! - **Local-Only Sources (FR-003, FR-007a)**:
//!   Suggestions are produced exclusively from data already on the local machine:
//!   the member's browsing history, bookmarks, and open tabs. FR-007a closes the list.
//! - **Zero Network Components (FR-007a, Principle VI)**:
//!   There is no remote suggestion service, no safe-browsing/reputation lookup,
//!   and no telemetry. The module has no dependency path to `evreos-net`.
//! - **Live Store Reconstruction (FR-004)**:
//!   The index is a pure function of current live stores. Deletions in history
//!   or bookmarks propagate immediately: deleted entries and deleted time ranges
//!   never reappear as suggestions.
//! - **Off-UI Execution (SC-006)**:
//!   To protect the 16 ms p99 keystroke latency cap, suggestion lookups run
//!   off the UI thread on the shell worker pool ([`crate::work::WorkerPool`]).

#![forbid(unsafe_code)]

use std::fmt;

/// The provenance or source of an omnibox suggestion.
///
/// Under FR-007a, suggestions are drawn exclusively from these three local sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SuggestionSource {
    /// An actively open tab.
    OpenTab,
    /// A saved member bookmark.
    Bookmark,
    /// A previously visited address in browsing history.
    History,
}

impl SuggestionSource {
    /// The display or identifier name of the source.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::OpenTab => "open_tab",
            Self::Bookmark => "bookmark",
            Self::History => "history",
        }
    }
}

impl fmt::Display for SuggestionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Representation of an open browser tab supplying live suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTab {
    /// The URL or address currently loaded in the tab.
    pub address: String,
    /// The page title of the tab.
    pub title: String,
}

impl OpenTab {
    /// Construct a new open tab representation.
    pub fn new(address: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
        }
    }
}

/// A matched suggestion ready for omnibox display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The target address.
    pub address: String,
    /// The title of the page or bookmark.
    pub title: String,
    /// Which local source yielded this suggestion.
    pub source: SuggestionSource,
    /// The relevance score (higher is more relevant).
    pub score: u32,
}

impl Suggestion {
    /// Construct a new suggestion.
    pub fn new(
        address: impl Into<String>,
        title: impl Into<String>,
        source: SuggestionSource,
        score: u32,
    ) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
            source,
            score,
        }
    }

    /// The target URL or address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The page or bookmark title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The origin source of the suggestion.
    pub fn source(&self) -> SuggestionSource {
        self.source
    }

    /// Computed match ranking score.
    pub fn score(&self) -> u32 {
        self.score
    }
}

/// The suggestion index.
#[derive(Debug, Default, Clone, Copy)]
pub struct SuggestionIndex;
