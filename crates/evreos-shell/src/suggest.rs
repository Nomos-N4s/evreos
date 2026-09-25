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

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt;

use crate::store::bookmarks::BookmarkStore;
use crate::store::history::HistoryStore;
use crate::work::{JobId, PoolError, WorkerPool};

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

    /// Base rank priority for deduplication across sources.
    fn priority(&self) -> u8 {
        match self {
            Self::OpenTab => 3,
            Self::Bookmark => 2,
            Self::History => 1,
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
///
/// Built strictly as a live projection over the three allowed sources:
/// history, bookmarks, and open tabs (FR-007a). Holds no independent cache
/// or stale persistent records, ensuring immediate propagation of deletions (FR-004).
#[derive(Debug, Default, Clone, Copy)]
pub struct SuggestionIndex;

impl SuggestionIndex {
    /// Default maximum number of suggestions returned.
    pub const DEFAULT_LIMIT: usize = 8;

    /// Create a new suggestion index instance.
    pub fn new() -> Self {
        Self
    }

    /// Query suggestions matching `query` from the live stores and open tabs.
    ///
    /// Reconstructed dynamically from the live stores so that deletions in history
    /// or bookmarks immediately take effect in the same operation (FR-004).
    pub fn query(
        history: &HistoryStore,
        bookmarks: &BookmarkStore,
        open_tabs: &[OpenTab],
        query: &str,
    ) -> Vec<Suggestion> {
        Self::query_with_limit(history, bookmarks, open_tabs, query, Self::DEFAULT_LIMIT)
    }

    /// Query suggestions matching `query` with a custom maximum limit.
    pub fn query_with_limit(
        history: &HistoryStore,
        bookmarks: &BookmarkStore,
        open_tabs: &[OpenTab],
        query: &str,
        limit: usize,
    ) -> Vec<Suggestion> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut candidates: HashMap<String, Suggestion> = HashMap::new();

        // 1. Open tabs
        for tab in open_tabs {
            if let Some(score) =
                score_match(&tab.address, &tab.title, &q, SuggestionSource::OpenTab)
            {
                let norm = normalize_address(&tab.address);
                insert_or_replace_better(
                    &mut candidates,
                    norm,
                    Suggestion::new(&tab.address, &tab.title, SuggestionSource::OpenTab, score),
                );
            }
        }

        // 2. Bookmarks
        for bm in bookmarks.bookmarks() {
            if let Some(score) =
                score_match(bm.address(), bm.title(), &q, SuggestionSource::Bookmark)
            {
                let norm = normalize_address(bm.address());
                insert_or_replace_better(
                    &mut candidates,
                    norm,
                    Suggestion::new(bm.address(), bm.title(), SuggestionSource::Bookmark, score),
                );
            }
        }

        // 3. History
        for h in history.entries() {
            if let Some(score) = score_match(&h.address, &h.title, &q, SuggestionSource::History) {
                let norm = normalize_address(&h.address);
                insert_or_replace_better(
                    &mut candidates,
                    norm,
                    Suggestion::new(&h.address, &h.title, SuggestionSource::History, score),
                );
            }
        }

        let mut results: Vec<Suggestion> = candidates.into_values().collect();
        results.sort_by_key(|s| (Reverse(s.score), s.address.clone()));
        results.truncate(limit);
        results
    }

    /// Submit a suggestion lookup job to execute off the UI thread on the shell worker pool.
    ///
    /// SC-006 caps keystroke response at 16 ms at the 99th percentile. Running lookups
    /// on worker threads ensures large history or bookmark stores never block UI rendering.
    pub fn query_async(
        pool: &WorkerPool<Vec<Suggestion>>,
        history: HistoryStore,
        bookmarks: BookmarkStore,
        open_tabs: Vec<OpenTab>,
        query: impl Into<String>,
    ) -> Result<JobId, PoolError> {
        Self::query_async_with_limit(
            pool,
            history,
            bookmarks,
            open_tabs,
            query,
            Self::DEFAULT_LIMIT,
        )
    }

    /// Submit a suggestion lookup job with an explicit result limit.
    pub fn query_async_with_limit(
        pool: &WorkerPool<Vec<Suggestion>>,
        history: HistoryStore,
        bookmarks: BookmarkStore,
        open_tabs: Vec<OpenTab>,
        query: impl Into<String>,
        limit: usize,
    ) -> Result<JobId, PoolError> {
        let q = query.into();
        pool.submit(move || Self::query_with_limit(&history, &bookmarks, &open_tabs, &q, limit))
    }
}

fn score_match(address: &str, title: &str, q: &str, source: SuggestionSource) -> Option<u32> {
    let addr_lower = address.to_lowercase();
    let title_lower = title.to_lowercase();

    let addr_matches = addr_lower.contains(q);
    let title_matches = title_lower.contains(q);

    if !addr_matches && !title_matches {
        return None;
    }

    // Base score by source
    let base_score: u32 = match source {
        SuggestionSource::OpenTab => 3000,
        SuggestionSource::Bookmark => 2000,
        SuggestionSource::History => 1000,
    };

    let mut bonus: u32 = 0;

    // Exact matches
    if addr_lower == q {
        bonus += 1000;
    } else if title_lower == q {
        bonus += 800;
    }

    // Prefix matches
    if addr_lower.starts_with(q) {
        bonus += 500;
    } else if let Some(stripped) = addr_lower.strip_prefix("https://") {
        if stripped.starts_with(q) {
            bonus += 400;
        }
    } else if let Some(stripped) = addr_lower.strip_prefix("http://") {
        if stripped.starts_with(q) {
            bonus += 400;
        }
    }

    if title_lower.starts_with(q) {
        bonus += 300;
    }

    // Substring matches
    if addr_matches {
        bonus += 200;
    }
    if title_matches {
        bonus += 100;
    }

    Some(base_score + bonus)
}

fn normalize_address(address: &str) -> String {
    let lower = address.trim().to_lowercase();
    let without_slash = lower.trim_end_matches('/');
    without_slash.to_string()
}

fn insert_or_replace_better(
    map: &mut HashMap<String, Suggestion>,
    key: String,
    new_suggestion: Suggestion,
) {
    match map.get(&key) {
        Some(existing) => {
            if new_suggestion.source.priority() > existing.source.priority()
                || (new_suggestion.source.priority() == existing.source.priority()
                    && new_suggestion.score > existing.score)
            {
                map.insert(key, new_suggestion);
            }
        }
        None => {
            map.insert(key, new_suggestion);
        }
    }
}
